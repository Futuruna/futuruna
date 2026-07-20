// T17: State machine — enum + transition function
#[derive(Debug, Clone)]
enum State {
    Idle,
    Running(i64),
    Done(String),
    Failed(String),
}

fn transition(state: State, event: &str) -> State {
    match state {
        State::Idle => {
            if event == "start" { State::Running(0) }
            else { State::Idle }
        }
        State::Running(p) => {
            if event == "progress" { State::Running(p + 25) }
            else if event == "complete" { State::Done("success".to_string()) }
            else if event == "error" { State::Failed("runtime error".to_string()) }
            else { State::Running(p) }
        }
        other => other,
    }
}

fn describe_state(state: &State) -> String {
    match state {
        State::Idle => "idle".to_string(),
        State::Running(p) => format!("running ({}%)", p),
        State::Done(msg) => format!("done: {}", msg),
        State::Failed(msg) => format!("failed: {}", msg),
    }
}

fn main() {
    // start → progress → progress → complete
    let s = transition(State::Idle, "start");
    let s = transition(s, "progress");
    let s = transition(s, "progress");
    let s = transition(s, "complete");
    println!("{}", describe_state(&s));

    // start → error
    let s = transition(State::Idle, "start");
    let s = transition(s, "error");
    println!("{}", describe_state(&s));

    // start → 4x progress → complete
    let s = transition(State::Idle, "start");
    let s = transition(s, "progress");
    let s = transition(s, "progress");
    let s = transition(s, "progress");
    let s = transition(s, "progress");
    let s = transition(s, "complete");
    println!("{}", describe_state(&s));
}
