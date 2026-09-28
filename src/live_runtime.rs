// Runtime for subjects, live streams and actors in compiled Futuruna programs.
//
// The generated program includes this file inside a module. It implements the
// contract in docs/reference/streams.md, the same rules the interpreter
// implements in src/live_streams.rs:
// - every handle to a subject, derived stream or actor shares one cell;
// - `send` delivers synchronously: every subscription and derived stream has
//   handled the value before `send` returns, in the order they were created;
// - a value sent while the stream is delivering is delivered right after the
//   current one, so every observer sees the stream's values in order;
// - a stream ends once, by completion or by an error; new observers receive
//   the retained values and then the end;
// - an actor handles one message at a time; `send` returns after the message
//   (and any message sent while it was handled) has been handled.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

pub enum __FutEvent<T> {
    Value(T),
    Complete,
    Error(String),
}

impl<T: Clone> Clone for __FutEvent<T> {
    fn clone(&self) -> Self {
        match self {
            __FutEvent::Value(value) => __FutEvent::Value(value.clone()),
            __FutEvent::Complete => __FutEvent::Complete,
            __FutEvent::Error(message) => __FutEvent::Error(message.clone()),
        }
    }
}

#[derive(Clone)]
enum __FutEnd {
    Complete,
    Error(String),
}

impl __FutEnd {
    fn event<T>(&self) -> __FutEvent<T> {
        match self {
            __FutEnd::Complete => __FutEvent::Complete,
            __FutEnd::Error(message) => __FutEvent::Error(message.clone()),
        }
    }
}

type __FutObserver<T> = Arc<Mutex<dyn FnMut(&__FutEvent<T>) + Send>>;

struct __FutSubscriber<T> {
    id: u64,
    first_event: u64,
    observer: __FutObserver<T>,
}

struct __FutStreamCell<T> {
    retained: VecDeque<T>,
    limit: Option<usize>,
    emitted: i64,
    latest: Option<T>,
    end: Option<__FutEnd>,
    subscribers: Vec<__FutSubscriber<T>>,
    next_subscriber: u64,
    next_event: u64,
    pending: VecDeque<(u64, __FutEvent<T>)>,
    delivering: bool,
}

pub struct __FutStream<T> {
    cell: Arc<Mutex<__FutStreamCell<T>>>,
    writable: bool,
}

impl<T> Clone for __FutStream<T> {
    fn clone(&self) -> Self {
        __FutStream {
            cell: self.cell.clone(),
            writable: self.writable,
        }
    }
}

/// A value an operator can read as a stream: a live stream or the values of
/// a finite stream.
pub trait __FutSource<T> {
    fn into_live(self) -> __FutStream<T>;
}

impl<T: Clone + Send + 'static> __FutSource<T> for __FutStream<T> {
    fn into_live(self) -> __FutStream<T> {
        self
    }
}

impl<T: Clone + Send + 'static> __FutSource<T> for Vec<T> {
    fn into_live(self) -> __FutStream<T> {
        __FutStream::finished(self)
    }
}

fn __fut_combined_limit(left: Option<usize>, right: Option<usize>) -> Option<usize> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left.max(right)),
        _ => None,
    }
}

impl<T: Clone + Send + 'static> __FutStream<T> {
    fn with_limit(limit: Option<usize>, writable: bool) -> Self {
        __FutStream {
            cell: Arc::new(Mutex::new(__FutStreamCell {
                retained: VecDeque::new(),
                limit,
                emitted: 0,
                latest: None,
                end: None,
                subscribers: Vec::new(),
                next_subscriber: 0,
                next_event: 0,
                pending: VecDeque::new(),
                delivering: false,
            })),
            writable,
        }
    }

    /// `subject()`, `subject(initial)` and `subject(initial, keep)`.
    pub fn subject(initial: Option<T>, keep: Option<i64>) -> Self {
        let limit = match keep {
            None => None,
            Some(keep) if keep >= 0 => Some(keep as usize),
            Some(keep) => panic!(
                "subject history size must be an Int of at least 0, found {}",
                keep
            ),
        };
        let stream = Self::with_limit(limit, true);
        if let Some(initial) = initial {
            stream.emit(__FutEvent::Value(initial));
        }
        stream
    }

    fn finished(values: Vec<T>) -> Self {
        let stream = Self::with_limit(None, false);
        {
            let mut cell = stream.cell.lock().unwrap();
            cell.emitted = values.len() as i64;
            cell.latest = values.last().cloned();
            cell.retained = values.into();
            cell.end = Some(__FutEnd::Complete);
        }
        stream
    }

    fn derived(&self) -> Self {
        Self::with_limit(self.limit(), false)
    }

    fn limit(&self) -> Option<usize> {
        self.cell.lock().unwrap().limit
    }

    /// `as_stream(subject)`: the same stream without write access.
    pub fn read_only(&self) -> Self {
        __FutStream {
            cell: self.cell.clone(),
            writable: false,
        }
    }

    fn require_writable_open(&self, operation: &str) {
        if !self.writable {
            panic!("{} needs a subject; this stream is read-only", operation);
        }
        match &self.cell.lock().unwrap().end {
            None => {}
            Some(__FutEnd::Complete) => panic!("{}: the subject is already completed", operation),
            Some(__FutEnd::Error(message)) => panic!(
                "{}: the subject already ended with error: {}",
                operation, message
            ),
        }
    }

    /// `subject <- value`.
    pub fn send(&self, value: T) {
        self.require_writable_open("`<-`");
        self.emit(__FutEvent::Value(value));
    }

    /// `complete(subject)`.
    pub fn complete(&self) {
        self.require_writable_open("complete");
        self.emit(__FutEvent::Complete);
    }

    /// `error(subject, message)`.
    pub fn error(&self, message: String) {
        self.require_writable_open("error");
        self.emit(__FutEvent::Error(message));
    }

    /// `.count`: values the stream has emitted.
    pub fn count(&self) -> i64 {
        self.cell.lock().unwrap().emitted
    }

    /// `.latest`: the most recent value.
    pub fn latest(&self) -> T {
        let latest = self.cell.lock().unwrap().latest.clone();
        latest.unwrap_or_else(|| panic!("`latest` of a stream that has no values"))
    }

    /// Retained values for a terminal read; an error ending fails the read.
    pub fn snapshot(&self) -> Vec<T> {
        let cell = self.cell.lock().unwrap();
        if let Some(__FutEnd::Error(message)) = &cell.end {
            let message = message.clone();
            drop(cell);
            panic!("stream ended with error: {}", message);
        }
        cell.retained.iter().cloned().collect()
    }

    fn emit(&self, event: __FutEvent<T>) {
        {
            let mut cell = self.cell.lock().unwrap();
            if cell.end.is_some() {
                return;
            }
            let sequence = cell.next_event;
            cell.next_event += 1;
            match &event {
                __FutEvent::Value(value) => {
                    cell.emitted += 1;
                    cell.latest = Some(value.clone());
                    cell.retained.push_back(value.clone());
                    if let Some(limit) = cell.limit {
                        while cell.retained.len() > limit {
                            cell.retained.pop_front();
                        }
                    }
                }
                __FutEvent::Complete => cell.end = Some(__FutEnd::Complete),
                __FutEvent::Error(message) => cell.end = Some(__FutEnd::Error(message.clone())),
            }
            cell.pending.push_back((sequence, event));
            if cell.delivering {
                return;
            }
            cell.delivering = true;
        }
        self.drain();
    }

    fn drain(&self) {
        loop {
            let (event, targets) = {
                let mut cell = self.cell.lock().unwrap();
                let Some((sequence, event)) = cell.pending.pop_front() else {
                    cell.delivering = false;
                    return;
                };
                let targets = cell
                    .subscribers
                    .iter()
                    .filter(|subscriber| subscriber.first_event <= sequence)
                    .map(|subscriber| (subscriber.id, subscriber.observer.clone()))
                    .collect::<Vec<_>>();
                (event, targets)
            };
            for (id, observer) in targets {
                let still_subscribed = self
                    .cell
                    .lock()
                    .unwrap()
                    .subscribers
                    .iter()
                    .any(|subscriber| subscriber.id == id);
                if still_subscribed {
                    (&mut *observer.lock().unwrap())(&event);
                }
            }
            if !matches!(event, __FutEvent::Value(_)) {
                self.cell.lock().unwrap().subscribers.clear();
            }
        }
    }

    fn attach(
        &self,
        replay: bool,
        observer: impl FnMut(&__FutEvent<T>) + Send + 'static,
    ) -> Option<u64> {
        let observer: __FutObserver<T> = Arc::new(Mutex::new(observer));
        let (retained, end, was_delivering, id) = {
            let mut cell = self.cell.lock().unwrap();
            let retained = if replay {
                cell.retained.iter().cloned().collect::<Vec<_>>()
            } else {
                Vec::new()
            };
            let end = cell.end.clone();
            let was_delivering = std::mem::replace(&mut cell.delivering, true);
            let id = if end.is_none() {
                let id = cell.next_subscriber;
                cell.next_subscriber += 1;
                let first_event = cell.next_event;
                cell.subscribers.push(__FutSubscriber {
                    id,
                    first_event,
                    observer: observer.clone(),
                });
                Some(id)
            } else {
                None
            };
            (retained, end, was_delivering, id)
        };
        {
            let mut observe = observer.lock().unwrap();
            for value in retained {
                (&mut *observe)(&__FutEvent::Value(value));
            }
            if let Some(end) = &end {
                (&mut *observe)(&end.event());
            }
        }
        if !was_delivering {
            self.drain();
        }
        if let (Some(id), Some(scope)) = (id, __fut_current_scope()) {
            let stream = self.clone();
            __fut_register_scope_cleanup(scope, Box::new(move || stream.unsubscribe(id)));
        }
        id
    }

    fn unsubscribe(&self, id: u64) {
        self.cell
            .lock()
            .unwrap()
            .subscribers
            .retain(|subscriber| subscriber.id != id);
    }

    /// `~ stream | arms` and `for x in stream { ... }`.
    pub fn subscribe(&self, observer: impl FnMut(&__FutEvent<T>) + Send + 'static) {
        self.attach(true, observer);
    }

    fn forward_end<U: Clone + Send + 'static>(out: &__FutStream<U>, event: &__FutEvent<T>) {
        match event {
            __FutEvent::Value(_) => {}
            __FutEvent::Complete => out.emit(__FutEvent::Complete),
            __FutEvent::Error(message) => out.emit(__FutEvent::Error(message.clone())),
        }
    }

    pub fn map<U: Clone + Send + 'static>(
        &self,
        mut f: impl FnMut(T) -> U + Send + 'static,
    ) -> __FutStream<U> {
        let out = __FutStream::<U>::with_limit(self.limit(), false);
        let target = out.clone();
        self.attach(true, move |event| match event {
            __FutEvent::Value(value) => target.emit(__FutEvent::Value(f(value.clone()))),
            _ => Self::forward_end(&target, event),
        });
        out
    }

    pub fn filter(&self, mut keep: impl FnMut(T) -> bool + Send + 'static) -> __FutStream<T> {
        let out = self.derived();
        let target = out.clone();
        self.attach(true, move |event| match event {
            __FutEvent::Value(value) => {
                if keep(value.clone()) {
                    target.emit(event.clone());
                }
            }
            _ => Self::forward_end(&target, event),
        });
        out
    }

    pub fn scan<A: Clone + Send + 'static>(
        &self,
        init: A,
        mut step: impl FnMut(A, T) -> A + Send + 'static,
    ) -> __FutStream<A> {
        let out = __FutStream::<A>::with_limit(self.limit(), false);
        let target = out.clone();
        let mut acc = init;
        self.attach(true, move |event| match event {
            __FutEvent::Value(value) => {
                acc = step(acc.clone(), value.clone());
                target.emit(__FutEvent::Value(acc.clone()));
            }
            _ => Self::forward_end(&target, event),
        });
        out
    }

    pub fn take(&self, count: i64) -> __FutStream<T> {
        let out = self.derived();
        let mut remaining = count.max(0);
        if remaining == 0 {
            out.emit(__FutEvent::Complete);
            return out;
        }
        let target = out.clone();
        self.attach(true, move |event| match event {
            __FutEvent::Value(_) => {
                if remaining > 0 {
                    remaining -= 1;
                    target.emit(event.clone());
                    if remaining == 0 {
                        target.emit(__FutEvent::Complete);
                    }
                }
            }
            _ => Self::forward_end(&target, event),
        });
        out
    }

    pub fn skip(&self, count: i64) -> __FutStream<T> {
        let out = self.derived();
        let target = out.clone();
        let mut remaining = count.max(0);
        self.attach(true, move |event| match event {
            __FutEvent::Value(_) => {
                if remaining > 0 {
                    remaining -= 1;
                } else {
                    target.emit(event.clone());
                }
            }
            _ => Self::forward_end(&target, event),
        });
        out
    }

    pub fn tap(&self, mut observe: impl FnMut(T) + Send + 'static) -> __FutStream<T> {
        let out = self.derived();
        let target = out.clone();
        self.attach(true, move |event| {
            if let __FutEvent::Value(value) = event {
                observe(value.clone());
            }
            target.emit(event.clone());
        });
        out
    }

    pub fn start_with(&self, first: T) -> __FutStream<T> {
        let out = self.derived();
        out.emit(__FutEvent::Value(first));
        let target = out.clone();
        self.attach(true, move |event| target.emit(event.clone()));
        out
    }

    pub fn catch<S: __FutSource<T>>(
        &self,
        mut recover: impl FnMut(String) -> S + Send + 'static,
    ) -> __FutStream<T> {
        let out = self.derived();
        let target = out.clone();
        self.attach(true, move |event| match event {
            __FutEvent::Error(message) => {
                let recovery = recover(message.clone()).into_live();
                let forward = target.clone();
                recovery.attach(true, move |event| forward.emit(event.clone()));
            }
            _ => target.emit(event.clone()),
        });
        out
    }

    pub fn merge(&self, other: impl __FutSource<T>) -> __FutStream<T> {
        let other = other.into_live();
        let out = Self::with_limit(__fut_combined_limit(self.limit(), other.limit()), false);
        // Values both sources already retain alternate, as for finite
        // streams; later values follow emission order.
        let left = self.cell.lock().unwrap().retained.iter().cloned().collect::<Vec<_>>();
        let right = other.cell.lock().unwrap().retained.iter().cloned().collect::<Vec<_>>();
        for index in 0..left.len().max(right.len()) {
            for side in [&left, &right] {
                if let Some(value) = side.get(index) {
                    out.emit(__FutEvent::Value(value.clone()));
                }
            }
        }
        let open_sides = Arc::new(Mutex::new(2u8));
        for side in [self, &other] {
            let target = out.clone();
            let open_sides = open_sides.clone();
            side.attach(false, move |event| match event {
                __FutEvent::Complete => {
                    let remaining = {
                        let mut open = open_sides.lock().unwrap();
                        *open = open.saturating_sub(1);
                        *open
                    };
                    if remaining == 0 {
                        target.emit(__FutEvent::Complete);
                    }
                }
                _ => target.emit(event.clone()),
            });
        }
        out
    }

    pub fn concat(&self, second: impl __FutSource<T>) -> __FutStream<T> {
        struct SecondState<T> {
            first_done: bool,
            buffered: Vec<T>,
            end: Option<__FutEvent<T>>,
        }
        let second = second.into_live();
        let out = Self::with_limit(__fut_combined_limit(self.limit(), second.limit()), false);
        let state = Arc::new(Mutex::new(SecondState {
            first_done: false,
            buffered: Vec::new(),
            end: None,
        }));
        {
            let target = out.clone();
            let state = state.clone();
            second.attach(true, move |event| {
                let forward = {
                    let mut state = state.lock().unwrap();
                    if state.first_done {
                        true
                    } else {
                        match event {
                            __FutEvent::Value(value) => state.buffered.push(value.clone()),
                            _ => state.end = Some(event.clone()),
                        }
                        false
                    }
                };
                if forward {
                    target.emit(event.clone());
                }
            });
        }
        let target = out.clone();
        self.attach(true, move |event| match event {
            __FutEvent::Complete => {
                let (buffered, end) = {
                    let mut state = state.lock().unwrap();
                    state.first_done = true;
                    (std::mem::take(&mut state.buffered), state.end.take())
                };
                for value in buffered {
                    target.emit(__FutEvent::Value(value));
                }
                if let Some(end) = end {
                    target.emit(end);
                }
            }
            _ => target.emit(event.clone()),
        });
        out
    }
}

// ── Named scopes ─────────────────────────────────────────────────────────

static __FUT_SCOPE_STACK: Mutex<Vec<&'static str>> = Mutex::new(Vec::new());
static __FUT_SCOPE_CLEANUPS: Mutex<Vec<(&'static str, Box<dyn FnOnce() + Send>)>> =
    Mutex::new(Vec::new());

fn __fut_current_scope() -> Option<&'static str> {
    __FUT_SCOPE_STACK.lock().unwrap().last().copied()
}

fn __fut_register_scope_cleanup(scope: &'static str, cleanup: Box<dyn FnOnce() + Send>) {
    __FUT_SCOPE_CLEANUPS.lock().unwrap().push((scope, cleanup));
}

/// Subscriptions and derived streams created while a named scope's body runs
/// belong to that scope.
pub fn __fut_scope_enter(scope: &'static str) {
    __FUT_SCOPE_STACK.lock().unwrap().push(scope);
}

pub fn __fut_scope_exit() {
    __FUT_SCOPE_STACK.lock().unwrap().pop();
}

/// `@ teardown("Scope")`: the scope's subscriptions stop receiving values.
pub fn __fut_teardown(scope: &str) {
    let cleanups = {
        let mut all = __FUT_SCOPE_CLEANUPS.lock().unwrap();
        let (owned, rest): (Vec<_>, Vec<_>) = all.drain(..).partition(|(name, _)| *name == scope);
        *all = rest;
        owned
    };
    for (_, cleanup) in cleanups {
        cleanup();
    }
}

// ── Actors ───────────────────────────────────────────────────────────────

struct __FutActorCell<S, M> {
    state: S,
    mailbox: VecDeque<M>,
    busy: bool,
}

pub struct __FutActor<S, M> {
    cell: Arc<Mutex<__FutActorCell<S, M>>>,
    name: &'static str,
    handler: fn(S, M) -> S,
}

impl<S, M> Clone for __FutActor<S, M> {
    fn clone(&self) -> Self {
        __FutActor {
            cell: self.cell.clone(),
            name: self.name,
            handler: self.handler,
        }
    }
}

impl<S: Clone, M> __FutActor<S, M> {
    pub fn spawn(name: &'static str, handler: fn(S, M) -> S, initial: S) -> Self {
        __FutActor {
            cell: Arc::new(Mutex::new(__FutActorCell {
                state: initial,
                mailbox: VecDeque::new(),
                busy: false,
            })),
            name,
            handler,
        }
    }

    fn handle(&self, message: M) -> S {
        let state = self.cell.lock().unwrap().state.clone();
        let next = (self.handler)(state, message);
        self.cell.lock().unwrap().state = next.clone();
        next
    }

    fn drain(&self) {
        loop {
            let next = {
                let mut cell = self.cell.lock().unwrap();
                match cell.mailbox.pop_front() {
                    Some(message) => message,
                    None => {
                        cell.busy = false;
                        return;
                    }
                }
            };
            self.handle(next);
        }
    }

    /// `actor <- message`.
    pub fn send(&self, message: M) {
        {
            let mut cell = self.cell.lock().unwrap();
            cell.mailbox.push_back(message);
            if cell.busy {
                return;
            }
            cell.busy = true;
        }
        self.drain();
    }

    /// `ask(actor, message)`: handle the message and return the new state.
    pub fn ask(&self, message: M) -> S {
        {
            let mut cell = self.cell.lock().unwrap();
            if cell.busy {
                drop(cell);
                panic!(
                    "ask to actor `{}` while it is handling a message would never return",
                    self.name
                );
            }
            cell.busy = true;
        }
        let state = self.handle(message);
        self.drain();
        state
    }
}
