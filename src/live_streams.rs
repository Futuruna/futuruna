//! Subjects, live streams and actors are shared runtime cells.
//!
//! Every handle to a subject, a derived live stream or an actor refers to the
//! same cell. Sending delivers synchronously and in order to every
//! subscription and derived stream; a stream ends once, by completion or by an
//! error; an actor handles one message at a time. The written contract is
//! `docs/reference/streams.md`; compiled programs implement the same rules in
//! `src/live_runtime.rs`.

use super::*;
use std::collections::VecDeque;

/// Operators that build a live stream when their stream argument is live.
pub const LIVE_STREAM_OPERATORS: &[&str] = &[
    "map",
    "filter",
    "scan",
    "take",
    "skip",
    "tap",
    "merge",
    "start_with",
    "concat",
    "catch",
];

/// Operators that read the values a live stream currently retains.
pub const LIVE_STREAM_SNAPSHOT_OPERATORS: &[&str] = &[
    "collect", "count", "sum", "first", "last", "reduce", "any", "all",
];

/// Stream operators that are defined only for finite streams.
pub const FINITE_ONLY_STREAM_OPERATORS: &[&str] = &[
    "flat_map",
    "enumerate",
    "distinct",
    "zip",
    "window",
    "combine_latest",
    "pairwise",
    "debounce",
    "throttle",
    "delay",
    "buffer",
    "timeout",
    "switch_map",
    "sample",
    "take_until",
];

/// Builtins that receive a live stream as a value instead of reading it.
const LIVE_STREAM_PASS_THROUGH: &[&str] = &[
    "print",
    "show",
    "rust_debug",
    "complete",
    "error",
    "as_stream",
    "spawn",
    "pair",
    "triple",
];

pub fn finite_only_stream_operator_message(operator: &str) -> String {
    format!(
        "`{operator}` is defined only for finite streams; read the live stream with `collect(...)` first"
    )
}

#[derive(Clone)]
pub enum StreamEnd {
    Complete,
    Error(String),
}

#[derive(Clone)]
enum StreamEvent {
    Value(Value),
    End(StreamEnd),
}

/// A handle to a live stream. Subjects are writable handles; derived streams
/// and `as_stream(...)` views are read-only handles to a cell.
#[derive(Clone)]
pub struct LiveStream {
    cell: Rc<RefCell<LiveStreamCell>>,
    writable: bool,
}

struct LiveStreamCell {
    retained: VecDeque<Value>,
    /// `None` keeps every value; `Some(k)` keeps the last `k`.
    limit: Option<usize>,
    emitted: i64,
    latest: Option<Value>,
    end: Option<StreamEnd>,
    subscribers: Vec<LiveSubscriber>,
    next_subscriber: u64,
    next_event: u64,
    pending: VecDeque<(u64, StreamEvent)>,
    delivering: bool,
}

struct LiveSubscriber {
    id: u64,
    /// First event sequence number this subscriber receives live; earlier
    /// values reached it through the replay of retained values.
    first_event: u64,
    observer: Rc<RefCell<Observer>>,
}

enum Observer {
    Subscription {
        arms: Rc<Vec<MatchArm>>,
        env: Env,
    },
    ForLoop {
        var: String,
        body: Rc<Vec<Stmt>>,
        env: Env,
    },
    Map {
        callback: Value,
        env: Env,
        out: LiveStream,
    },
    Filter {
        callback: Value,
        env: Env,
        out: LiveStream,
    },
    Scan {
        callback: Value,
        env: Env,
        acc: Value,
        out: LiveStream,
    },
    Take {
        remaining: i64,
        out: LiveStream,
    },
    Skip {
        remaining: i64,
        out: LiveStream,
    },
    Tap {
        callback: Value,
        env: Env,
        out: LiveStream,
    },
    Forward {
        out: LiveStream,
    },
    MergeSide {
        out: LiveStream,
        open_sides: Rc<Cell<u8>>,
    },
    ConcatFirst {
        out: LiveStream,
        second: Rc<RefCell<ConcatSecondState>>,
    },
    ConcatSecond {
        out: LiveStream,
        state: Rc<RefCell<ConcatSecondState>>,
    },
    Catch {
        callback: Value,
        env: Env,
        out: LiveStream,
    },
}

#[derive(Default)]
struct ConcatSecondState {
    first_done: bool,
    buffered: Vec<Value>,
    end: Option<StreamEnd>,
}

impl LiveStream {
    fn new(limit: Option<usize>, writable: bool) -> Self {
        LiveStream {
            cell: Rc::new(RefCell::new(LiveStreamCell {
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

    /// A completed stream holding `values`, used when a finite stream meets
    /// a live one.
    fn finished(values: Vec<Value>) -> Self {
        let stream = LiveStream::new(None, false);
        {
            let mut cell = stream.cell.borrow_mut();
            cell.emitted = values.len() as i64;
            cell.latest = values.last().cloned();
            cell.retained = values.into();
            cell.end = Some(StreamEnd::Complete);
        }
        stream
    }

    pub fn is_writable(&self) -> bool {
        self.writable
    }

    pub fn read_only(&self) -> Self {
        LiveStream {
            cell: self.cell.clone(),
            writable: false,
        }
    }

    pub fn same_stream(&self, other: &LiveStream) -> bool {
        Rc::ptr_eq(&self.cell, &other.cell)
    }

    /// The values the stream currently retains, oldest first.
    pub fn retained_values(&self) -> Vec<Value> {
        self.cell.borrow().retained.iter().cloned().collect()
    }

    fn limit(&self) -> Option<usize> {
        self.cell.borrow().limit
    }

    fn end(&self) -> Option<StreamEnd> {
        self.cell.borrow().end.clone()
    }

    fn unsubscribe(&self, id: u64) {
        self.cell
            .borrow_mut()
            .subscribers
            .retain(|subscriber| subscriber.id != id);
    }
}

impl fmt::Debug for LiveStream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let cell = self.cell.borrow();
        f.debug_struct("LiveStream")
            .field("writable", &self.writable)
            .field("retained", &cell.retained)
            .field("emitted", &cell.emitted)
            .finish()
    }
}

impl fmt::Display for LiveStream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "~[")?;
        for (index, value) in self.cell.borrow().retained.iter().enumerate() {
            if index > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{}", value)?;
        }
        write!(f, "]")
    }
}

fn combined_limit(left: Option<usize>, right: Option<usize>) -> Option<usize> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left.max(right)),
        _ => None,
    }
}

/// A handle to an actor. Every copy of the handle shares the actor's state
/// and mailbox.
#[derive(Clone)]
pub struct ActorRef(Rc<ActorCell>);

struct ActorCell {
    actor_name: String,
    state_param: String,
    handlers: Vec<Handler>,
    env: Env,
    state: RefCell<Value>,
    mailbox: RefCell<VecDeque<Value>>,
    busy: Cell<bool>,
}

impl ActorRef {
    pub fn actor_name(&self) -> &str {
        &self.0.actor_name
    }

    pub fn state_param(&self) -> &str {
        &self.0.state_param
    }

    pub fn state(&self) -> Value {
        self.0.state.borrow().clone()
    }

    pub fn env(&self) -> &Env {
        &self.0.env
    }

    pub fn same_actor(&self, other: &ActorRef) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl fmt::Debug for ActorRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Actor")
            .field("name", &self.0.actor_name)
            .field("state", &*self.0.state.borrow())
            .finish()
    }
}

impl Interpreter {
    // ── Subjects ────────────────────────────────────────────────────────

    /// `subject()`, `subject(initial)` or `subject(initial, keep)`.
    pub(crate) fn live_subject(&mut self, args: Vec<Value>) -> Value {
        let mut args = args.into_iter();
        let initial = args.next();
        let limit = match args.next() {
            None => None,
            Some(Value::Int(keep)) if keep >= 0 => Some(keep as usize),
            Some(other) => {
                return self.panic_or_ground_fail(format!(
                    "subject history size must be an Int of at least 0, found {}",
                    other
                ))
            }
        };
        let stream = LiveStream::new(limit, true);
        if let Some(initial) = initial {
            self.live_emit(&stream, StreamEvent::Value(initial));
        }
        Value::LiveStream(stream)
    }

    fn live_writable_subject(&mut self, target: Option<&Value>, operation: &str) -> LiveStream {
        match target {
            Some(Value::LiveStream(stream)) if stream.is_writable() => stream.clone(),
            Some(Value::LiveStream(_)) => {
                self.ordinary_runtime_fail(format!(
                    "{operation} needs a subject; this stream is read-only"
                ))
            }
            Some(other) => self.ordinary_runtime_fail(format!(
                "{operation} needs a subject, found {}",
                other
            )),
            None => self.ordinary_runtime_fail(format!("{operation} needs a subject")),
        }
    }

    fn live_require_open(&self, stream: &LiveStream, operation: &str) {
        match stream.end() {
            None => {}
            Some(StreamEnd::Complete) => self.ordinary_runtime_fail(format!(
                "{operation}: the subject is already completed"
            )),
            Some(StreamEnd::Error(message)) => self.ordinary_runtime_fail(format!(
                "{operation}: the subject already ended with error: {message}"
            )),
        }
    }

    /// `target <- message` for subjects and actors.
    pub(crate) fn live_send(&mut self, target: Value, message: Value) {
        match target {
            Value::Actor(actor) => self.actor_send(&actor, message),
            Value::LiveStream(stream) if stream.is_writable() => {
                self.live_require_open(&stream, "`<-`");
                self.live_emit(&stream, StreamEvent::Value(message));
            }
            Value::LiveStream(_) => self.ordinary_runtime_fail(
                "`<-` needs a subject or an actor; this stream is read-only".to_string(),
            ),
            other => self.ordinary_runtime_fail(format!(
                "`<-` needs a subject or an actor, found {}",
                other
            )),
        }
    }

    pub(crate) fn live_complete(&mut self, args: &[Value]) -> Value {
        let stream = self.live_writable_subject(args.first(), "complete");
        self.live_require_open(&stream, "complete");
        self.live_emit(&stream, StreamEvent::End(StreamEnd::Complete));
        Value::Unit
    }

    pub(crate) fn live_error(&mut self, args: &[Value]) -> Value {
        let stream = self.live_writable_subject(args.first(), "error");
        self.live_require_open(&stream, "error");
        let message = match args.get(1) {
            Some(Value::Str(message)) => message.clone(),
            Some(other) => format!("{}", other),
            None => self.ordinary_runtime_fail("error needs a subject and a message".to_string()),
        };
        self.live_emit(&stream, StreamEvent::End(StreamEnd::Error(message)));
        Value::Unit
    }

    pub(crate) fn live_as_stream(&mut self, args: Vec<Value>) -> Value {
        match args.into_iter().next() {
            Some(Value::LiveStream(stream)) => Value::LiveStream(stream.read_only()),
            Some(Value::Stream(items)) => Value::Stream(items),
            Some(other) => Value::Stream(list_to_vec(&other)),
            None => Value::Stream(Vec::new()),
        }
    }

    /// `.count` and `.latest` on a live stream.
    pub(crate) fn live_field(&self, stream: &LiveStream, field: &str) -> Option<Value> {
        let (emitted, latest) = {
            let cell = stream.cell.borrow();
            (cell.emitted, cell.latest.clone())
        };
        match field {
            "count" => Some(Value::Int(emitted)),
            "latest" => Some(latest.unwrap_or_else(|| {
                self.panic_or_ground_fail("`latest` of a stream that has no values")
            })),
            _ => None,
        }
    }

    /// Retained values for a terminal read; an error ending fails the read.
    fn live_snapshot(&mut self, stream: &LiveStream) -> Vec<Value> {
        if let Some(StreamEnd::Error(message)) = stream.end() {
            self.ordinary_runtime_fail(format!("stream ended with error: {message}"));
        }
        stream.retained_values()
    }

    // ── Delivery ────────────────────────────────────────────────────────

    /// Record an event and deliver it unless the stream is already
    /// delivering, in which case it is delivered after the current event.
    /// Events after the end are ignored; user-facing operations check
    /// `live_require_open` first.
    fn live_emit(&mut self, stream: &LiveStream, event: StreamEvent) {
        {
            let mut cell = stream.cell.borrow_mut();
            if cell.end.is_some() {
                return;
            }
            let sequence = cell.next_event;
            cell.next_event += 1;
            match &event {
                StreamEvent::Value(value) => {
                    cell.emitted += 1;
                    cell.latest = Some(value.clone());
                    cell.retained.push_back(value.clone());
                    if let Some(limit) = cell.limit {
                        while cell.retained.len() > limit {
                            cell.retained.pop_front();
                        }
                    }
                }
                StreamEvent::End(end) => cell.end = Some(end.clone()),
            }
            cell.pending.push_back((sequence, event));
            if cell.delivering {
                return;
            }
            cell.delivering = true;
        }
        self.live_drain(stream);
    }

    fn live_drain(&mut self, stream: &LiveStream) {
        loop {
            let (sequence, event, targets) = {
                let mut cell = stream.cell.borrow_mut();
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
                (sequence, event, targets)
            };
            let _ = sequence;
            for (id, observer) in targets {
                let still_subscribed = stream
                    .cell
                    .borrow()
                    .subscribers
                    .iter()
                    .any(|subscriber| subscriber.id == id);
                if still_subscribed {
                    self.live_notify(&observer, &event);
                }
            }
            if matches!(event, StreamEvent::End(_)) {
                stream.cell.borrow_mut().subscribers.clear();
            }
        }
    }

    /// Attach an observer: it first receives the retained values (and the
    /// end, if the stream has ended), then every later event.
    fn live_attach(&mut self, stream: &LiveStream, observer: Observer, replay: bool) -> Option<u64> {
        let observer = Rc::new(RefCell::new(observer));
        let (retained, end, was_delivering, id) = {
            let mut cell = stream.cell.borrow_mut();
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
                cell.subscribers.push(LiveSubscriber {
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
        for value in retained {
            self.live_notify(&observer, &StreamEvent::Value(value));
        }
        if let Some(end) = end {
            self.live_notify(&observer, &StreamEvent::End(end));
        }
        if !was_delivering {
            self.live_drain(stream);
        }
        if let (Some(id), Some(scope)) = (id, self.live_scope_stack.last().cloned()) {
            self.live_scope_subscriptions
                .entry(scope)
                .or_default()
                .push((stream.clone(), id));
        }
        id
    }

    fn live_notify(&mut self, observer: &Rc<RefCell<Observer>>, event: &StreamEvent) {
        enum Step {
            Subscription(Rc<Vec<MatchArm>>, Env),
            ForLoop(String, Rc<Vec<Stmt>>, Env),
            Map(Value, Env, LiveStream),
            Filter(Value, Env, LiveStream),
            Scan(Value, Env, Value, LiveStream),
            Tap(Value, Env, LiveStream),
            Catch(Value, Env, LiveStream),
            Emit(LiveStream, StreamEvent),
            EmitMany(LiveStream, Vec<Value>, Option<StreamEnd>),
            Nothing,
        }
        let step = {
            let mut observer = observer.borrow_mut();
            match (&mut *observer, event) {
                (Observer::Subscription { arms, env }, _) => {
                    Step::Subscription(arms.clone(), env.clone())
                }
                (Observer::ForLoop { var, body, env }, _) => {
                    Step::ForLoop(var.clone(), body.clone(), env.clone())
                }
                (Observer::Map { callback, env, out }, StreamEvent::Value(_)) => {
                    Step::Map(callback.clone(), env.clone(), out.clone())
                }
                (Observer::Filter { callback, env, out }, StreamEvent::Value(_)) => {
                    Step::Filter(callback.clone(), env.clone(), out.clone())
                }
                (
                    Observer::Scan {
                        callback,
                        env,
                        acc,
                        out,
                    },
                    StreamEvent::Value(_),
                ) => Step::Scan(callback.clone(), env.clone(), acc.clone(), out.clone()),
                (Observer::Tap { callback, env, out }, StreamEvent::Value(_)) => {
                    Step::Tap(callback.clone(), env.clone(), out.clone())
                }
                (Observer::Take { remaining, out }, StreamEvent::Value(value)) => {
                    if *remaining <= 0 {
                        Step::Nothing
                    } else {
                        *remaining -= 1;
                        let end = (*remaining == 0).then_some(StreamEnd::Complete);
                        Step::EmitMany(out.clone(), vec![value.clone()], end)
                    }
                }
                (Observer::Skip { remaining, out }, StreamEvent::Value(value)) => {
                    if *remaining > 0 {
                        *remaining -= 1;
                        Step::Nothing
                    } else {
                        Step::Emit(out.clone(), event.clone())
                    }
                }
                (Observer::MergeSide { out, open_sides }, StreamEvent::End(StreamEnd::Complete)) => {
                    let open = open_sides.get().saturating_sub(1);
                    open_sides.set(open);
                    if open == 0 {
                        Step::Emit(out.clone(), event.clone())
                    } else {
                        Step::Nothing
                    }
                }
                (Observer::ConcatFirst { out, second }, StreamEvent::End(StreamEnd::Complete)) => {
                    let mut second = second.borrow_mut();
                    second.first_done = true;
                    let buffered = std::mem::take(&mut second.buffered);
                    Step::EmitMany(out.clone(), buffered, second.end.clone())
                }
                (Observer::ConcatSecond { out, state }, _) => {
                    let mut state = state.borrow_mut();
                    if state.first_done {
                        Step::Emit(out.clone(), event.clone())
                    } else {
                        match event {
                            StreamEvent::Value(value) => state.buffered.push(value.clone()),
                            StreamEvent::End(end) => state.end = Some(end.clone()),
                        }
                        Step::Nothing
                    }
                }
                (Observer::Catch { callback, env, out }, StreamEvent::End(StreamEnd::Error(_))) => {
                    Step::Catch(callback.clone(), env.clone(), out.clone())
                }
                (
                    Observer::Map { out, .. }
                    | Observer::Filter { out, .. }
                    | Observer::Scan { out, .. }
                    | Observer::Tap { out, .. }
                    | Observer::Take { out, .. }
                    | Observer::Skip { out, .. }
                    | Observer::Forward { out }
                    | Observer::MergeSide { out, .. }
                    | Observer::ConcatFirst { out, .. }
                    | Observer::Catch { out, .. },
                    _,
                ) => Step::Emit(out.clone(), event.clone()),
            }
        };
        match step {
            Step::Nothing => {}
            Step::Emit(out, event) => self.live_emit(&out, event),
            Step::EmitMany(out, values, end) => {
                for value in values {
                    self.live_emit(&out, StreamEvent::Value(value));
                }
                if let Some(end) = end {
                    self.live_emit(&out, StreamEvent::End(end));
                }
            }
            Step::Subscription(arms, env) => self.live_run_subscription_arms(&arms, &env, event),
            Step::ForLoop(var, body, env) => match event {
                StreamEvent::Value(value) => {
                    let mut loop_env = env.child();
                    loop_env.set(var, value.clone());
                    self.run_statement_block(&body, &mut loop_env);
                }
                StreamEvent::End(StreamEnd::Complete) => {}
                StreamEvent::End(StreamEnd::Error(message)) => self.ordinary_runtime_fail(
                    format!("unhandled stream error in `for` loop: {message}"),
                ),
            },
            Step::Map(callback, env, out) => {
                if let StreamEvent::Value(value) = event {
                    let mapped =
                        self.apply_checked_builtin_callback(1, callback, vec![value.clone()], &env);
                    self.live_emit(&out, StreamEvent::Value(mapped));
                }
            }
            Step::Filter(callback, env, out) => {
                if let StreamEvent::Value(value) = event {
                    match self.apply_checked_builtin_callback(
                        1,
                        callback,
                        vec![value.clone()],
                        &env,
                    ) {
                        Value::Bool(true) => self.live_emit(&out, event.clone()),
                        Value::Bool(false) => {}
                        other => self.ordinary_runtime_fail(format!(
                            "filter predicate must return Bool, found {}",
                            other
                        )),
                    }
                }
            }
            Step::Scan(callback, env, acc, out) => {
                if let StreamEvent::Value(value) = event {
                    let next =
                        self.apply_checked_builtin_callback(2, callback, vec![acc, value.clone()], &env);
                    if let Observer::Scan { acc, .. } = &mut *observer.borrow_mut() {
                        *acc = next.clone();
                    }
                    self.live_emit(&out, StreamEvent::Value(next));
                }
            }
            Step::Tap(callback, env, out) => {
                if let StreamEvent::Value(value) = event {
                    self.apply_checked_builtin_callback(1, callback, vec![value.clone()], &env);
                    self.live_emit(&out, event.clone());
                }
            }
            Step::Catch(callback, env, out) => {
                if let StreamEvent::End(StreamEnd::Error(message)) = event {
                    let recovery = self.apply_checked_builtin_callback(
                        1,
                        callback,
                        vec![Value::Str(message.clone())],
                        &env,
                    );
                    let recovery = self.live_source(recovery, "catch");
                    self.live_attach(&recovery, Observer::Forward { out }, true);
                }
            }
        }
    }

    fn live_run_subscription_arms(&mut self, arms: &[MatchArm], env: &Env, event: &StreamEvent) {
        let is_complete_arm = |arm: &MatchArm| {
            matches!(&arm.pat, Pat::Var(name) | Pat::Con(name, _) if name == "Complete")
        };
        let is_error_arm = |arm: &MatchArm| matches!(&arm.pat, Pat::Con(name, _) if name == "Err");
        match event {
            StreamEvent::Value(value) => {
                let mut value_arms = arms
                    .iter()
                    .filter(|arm| !is_complete_arm(arm) && !is_error_arm(arm))
                    .peekable();
                if value_arms.peek().is_none() {
                    return;
                }
                for arm in value_arms {
                    let mut arm_env = env.child();
                    if !self.match_pattern(&arm.pat, value, &mut arm_env) {
                        continue;
                    }
                    let guard_ok = match &arm.guard {
                        Some(guard) => match self.eval(guard, &arm_env) {
                            Value::Bool(result) => result,
                            other => self.ordinary_runtime_fail(format!(
                                "subscription guard must return Bool, found {}",
                                other
                            )),
                        },
                        None => true,
                    };
                    if guard_ok {
                        self.eval(&arm.body, &arm_env);
                        return;
                    }
                }
                self.ordinary_runtime_fail(format!(
                    "no subscription arm matches the value {}",
                    value
                ));
            }
            StreamEvent::End(StreamEnd::Error(message)) => {
                let Some(arm) = arms.iter().find(|arm| is_error_arm(arm)) else {
                    self.ordinary_runtime_fail(format!(
                        "unhandled stream error: {message}; add an `| Err(e) -> ...` arm to handle it"
                    ));
                };
                let mut arm_env = env.child();
                let error = Value::Constructor("Err".into(), vec![Value::Str(message.clone())].into());
                if self.match_pattern(&arm.pat, &error, &mut arm_env) {
                    self.eval(&arm.body, &arm_env);
                }
            }
            StreamEvent::End(StreamEnd::Complete) => {
                if let Some(arm) = arms.iter().find(|arm| is_complete_arm(arm)) {
                    let arm_env = env.child();
                    self.eval(&arm.body, &arm_env);
                }
            }
        }
    }

    /// A live source for an operator: live streams stay shared, finite
    /// streams and lists become completed streams.
    fn live_source(&mut self, value: Value, operator: &str) -> LiveStream {
        match value {
            Value::LiveStream(stream) => stream,
            Value::Stream(items) | Value::List(items) => LiveStream::finished(items),
            other @ Value::Constructor(..) => LiveStream::finished(list_to_vec(&other)),
            other => self.ordinary_runtime_fail(format!(
                "`{operator}` needs a stream, found {}",
                other
            )),
        }
    }

    // ── Subscriptions ───────────────────────────────────────────────────

    /// `~ stream | arms`. Finite streams deliver their values and then
    /// complete.
    pub(crate) fn live_subscribe_statement(&mut self, source: Value, arms: &[MatchArm], env: &Env) {
        let stream = self.live_source(source, "subscription");
        let observer = Observer::Subscription {
            arms: Rc::new(arms.to_vec()),
            env: env.clone(),
        };
        self.live_attach(&stream, observer, true);
    }

    /// `for x in stream { ... }` over a live stream.
    pub(crate) fn live_for_statement(&mut self, stream: &LiveStream, var: &str, body: &[Stmt], env: &Env) {
        let observer = Observer::ForLoop {
            var: var.to_string(),
            body: Rc::new(body.to_vec()),
            env: env.clone(),
        };
        self.live_attach(stream, observer, true);
    }

    // ── Scopes ──────────────────────────────────────────────────────────

    pub(crate) fn live_scope_enter(&mut self, scope: &str) {
        self.live_scope_stack.push(scope.to_string());
    }

    pub(crate) fn live_scope_exit(&mut self) {
        self.live_scope_stack.pop();
    }

    /// `@ teardown("Scope")`: every subscription and derived-stream link
    /// created inside the scope stops receiving values.
    pub(crate) fn live_teardown(&mut self, scope: &str) {
        for (stream, id) in self.live_scope_subscriptions.remove(scope).unwrap_or_default() {
            stream.unsubscribe(id);
        }
    }

    // ── Operators ───────────────────────────────────────────────────────

    /// Builtins called with a live stream argument. Returns `None` for
    /// builtins that treat the live stream as an ordinary value.
    pub(crate) fn eval_live_stream_builtin(
        &mut self,
        namespace: &RuntimeNamespace,
        name: &str,
        args: &[Value],
        env: &Env,
    ) -> Option<Value> {
        let canonical = builtin_canonical(name);
        if LIVE_STREAM_PASS_THROUGH.contains(&canonical) {
            return None;
        }
        if FINITE_ONLY_STREAM_OPERATORS.contains(&canonical) {
            self.ordinary_runtime_fail(finite_only_stream_operator_message(canonical));
        }
        let live_source = matches!(args.first(), Some(Value::LiveStream(_)))
            || (matches!(canonical, "merge" | "concat")
                && matches!(args.get(1), Some(Value::LiveStream(_))));
        if LIVE_STREAM_OPERATORS.contains(&canonical) && live_source {
            return Some(self.live_operator(canonical, args, env));
        }
        if LIVE_STREAM_SNAPSHOT_OPERATORS.contains(&canonical) {
            let args = args
                .iter()
                .map(|argument| match argument {
                    Value::LiveStream(stream) => Value::Stream(self.live_snapshot(stream)),
                    other => other.clone(),
                })
                .collect::<Vec<_>>();
            return Some(self.eval_builtin_in_namespace(namespace, name, args, env));
        }
        self.ordinary_runtime_fail(format!(
            "`{name}` does not accept a live stream; read its values with `collect(...)` first"
        ))
    }

    fn live_operator(&mut self, operator: &str, args: &[Value], env: &Env) -> Value {
        let argument = |index: usize| args.get(index).cloned().unwrap_or(Value::Unit);
        let count_argument = |this: &Self, index: usize| match args.get(index) {
            Some(Value::Int(count)) => (*count).max(0),
            Some(other) => this.ordinary_runtime_fail(format!(
                "`{operator}` needs an Int count, found {}",
                other
            )),
            None => this.ordinary_runtime_fail(format!("`{operator}` needs a count")),
        };
        let source = self.live_source(argument(0), operator);
        let out = LiveStream::new(source.limit(), false);
        match operator {
            "map" => {
                let observer = Observer::Map {
                    callback: argument(1),
                    env: env.clone(),
                    out: out.clone(),
                };
                self.live_attach(&source, observer, true);
            }
            "filter" => {
                let observer = Observer::Filter {
                    callback: argument(1),
                    env: env.clone(),
                    out: out.clone(),
                };
                self.live_attach(&source, observer, true);
            }
            "scan" => {
                let observer = Observer::Scan {
                    callback: argument(2),
                    env: env.clone(),
                    acc: argument(1),
                    out: out.clone(),
                };
                self.live_attach(&source, observer, true);
            }
            "take" => {
                let remaining = count_argument(self, 1);
                if remaining == 0 {
                    self.live_emit(&out, StreamEvent::End(StreamEnd::Complete));
                } else {
                    let observer = Observer::Take {
                        remaining,
                        out: out.clone(),
                    };
                    self.live_attach(&source, observer, true);
                }
            }
            "skip" => {
                let remaining = count_argument(self, 1);
                let observer = Observer::Skip {
                    remaining,
                    out: out.clone(),
                };
                self.live_attach(&source, observer, true);
            }
            "tap" => {
                let observer = Observer::Tap {
                    callback: argument(1),
                    env: env.clone(),
                    out: out.clone(),
                };
                self.live_attach(&source, observer, true);
            }
            "start_with" => {
                self.live_emit(&out, StreamEvent::Value(argument(1)));
                self.live_attach(&source, Observer::Forward { out: out.clone() }, true);
            }
            "catch" => {
                let observer = Observer::Catch {
                    callback: argument(1),
                    env: env.clone(),
                    out: out.clone(),
                };
                self.live_attach(&source, observer, true);
            }
            "merge" => {
                let other = self.live_source(argument(1), operator);
                let out = LiveStream::new(combined_limit(source.limit(), other.limit()), false);
                // Values both sources already retain alternate, as for
                // finite streams; later values follow emission order.
                let left = source.retained_values();
                let right = other.retained_values();
                for index in 0..left.len().max(right.len()) {
                    for side in [&left, &right] {
                        if let Some(value) = side.get(index) {
                            self.live_emit(&out, StreamEvent::Value(value.clone()));
                        }
                    }
                }
                let open_sides = Rc::new(Cell::new(2u8));
                for side in [&source, &other] {
                    let observer = Observer::MergeSide {
                        out: out.clone(),
                        open_sides: open_sides.clone(),
                    };
                    self.live_attach(side, observer, false);
                }
                return Value::LiveStream(out);
            }
            "concat" => {
                let second = self.live_source(argument(1), operator);
                let out = LiveStream::new(combined_limit(source.limit(), second.limit()), false);
                let state = Rc::new(RefCell::new(ConcatSecondState::default()));
                self.live_attach(
                    &second,
                    Observer::ConcatSecond {
                        out: out.clone(),
                        state: state.clone(),
                    },
                    true,
                );
                self.live_attach(
                    &source,
                    Observer::ConcatFirst {
                        out: out.clone(),
                        second: state,
                    },
                    true,
                );
                return Value::LiveStream(out);
            }
            _ => unreachable!("live operator table and dispatch disagree on `{operator}`"),
        }
        Value::LiveStream(out)
    }

    // ── Actors ──────────────────────────────────────────────────────────

    pub(crate) fn spawn_actor(
        &mut self,
        actor_name: String,
        state_param: String,
        handlers: Vec<Handler>,
        env: Env,
        initial_state: Value,
    ) -> Value {
        Value::Actor(ActorRef(Rc::new(ActorCell {
            actor_name,
            state_param,
            handlers,
            env,
            state: RefCell::new(initial_state),
            mailbox: RefCell::new(VecDeque::new()),
            busy: Cell::new(false),
        })))
    }

    fn actor_handle(&mut self, actor: &ActorRef, message: &Value) -> Value {
        let state = actor.state();
        let next = self.dispatch_actor_message(
            &actor.0.actor_name,
            &state,
            &actor.0.state_param,
            &actor.0.handlers,
            &actor.0.env,
            message,
        );
        *actor.0.state.borrow_mut() = next.clone();
        next
    }

    fn actor_drain(&mut self, actor: &ActorRef) {
        loop {
            let next = actor.0.mailbox.borrow_mut().pop_front();
            let Some(message) = next else {
                break;
            };
            self.actor_handle(actor, &message);
        }
        actor.0.busy.set(false);
    }

    /// `actor <- message`: the message is handled before the send returns.
    /// A message sent while the actor is handling another one is handled
    /// right after it, in send order.
    pub(crate) fn actor_send(&mut self, actor: &ActorRef, message: Value) {
        actor.0.mailbox.borrow_mut().push_back(message);
        if actor.0.busy.replace(true) {
            return;
        }
        self.actor_drain(actor);
    }

    /// `ask(actor, message)`: handle the message and return the new state.
    pub(crate) fn actor_ask(&mut self, args: &[Value]) -> Value {
        let (Some(Value::Actor(actor)), Some(message)) = (args.first(), args.get(1)) else {
            self.ordinary_runtime_fail(match args.first() {
                Some(other) => format!("ask needs an actor, found {}", other),
                None => "ask needs an actor and a message".to_string(),
            });
        };
        let actor = actor.clone();
        if actor.0.busy.replace(true) {
            self.ordinary_runtime_fail(format!(
                "ask to actor `{}` while it is handling a message would never return",
                actor.0.actor_name
            ));
        }
        let state = self.actor_handle(&actor, message);
        self.actor_drain(&actor);
        state
    }
}
