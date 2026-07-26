// Downstream-shaped fixture: text command parsing and reporting.

#[derive(Clone, Debug)]
enum Command {
    Enable(String),
    Disable(String),
    Threshold(i64),
    Ignore,
}

fn parse_line(line: &str) -> Command {
    if line == "enable:search" {
        Command::Enable("search".to_string())
    } else if line == "disable:ads" {
        Command::Disable("ads".to_string())
    } else if line == "threshold:10" {
        Command::Threshold(10)
    } else {
        Command::Ignore
    }
}

fn add_line(commands: Vec<Command>, line: &str) -> Vec<Command> {
    let mut out = commands.clone();
    out.push(parse_line(line));
    out
}

fn command_label(command: &Command) -> String {
    match command {
        Command::Enable(name) => format!("enable {}", name),
        Command::Disable(name) => format!("disable {}", name),
        Command::Threshold(value) => format!("threshold {}", value),
        Command::Ignore => "ignore".to_string(),
    }
}

fn enabled_value(command: &Command) -> i64 {
    let label = command_label(command);
    if label.starts_with("enable") { 1 } else { 0 }
}

fn threshold_value(command: &Command) -> i64 {
    let label = command_label(command);
    if label == "threshold 10" { 10 } else { 0 }
}

fn enabled_count(commands: &Vec<Command>) -> i64 {
    enabled_value(&commands[0])
        + enabled_value(&commands[1])
        + enabled_value(&commands[2])
        + enabled_value(&commands[3])
}

fn threshold_sum(commands: &Vec<Command>) -> i64 {
    threshold_value(&commands[0])
        + threshold_value(&commands[1])
        + threshold_value(&commands[2])
        + threshold_value(&commands[3])
}

fn main() {
    let mut commands = Vec::new();
    commands = add_line(commands, "enable:search");
    commands = add_line(commands, "threshold:10");
    commands = add_line(commands, "disable:ads");
    commands = add_line(commands, "comment");

    println!("commands={}", commands.len());
    println!("enabled={}", enabled_count(&commands));
    println!("thresholds={}", threshold_sum(&commands));
    println!("first={}", command_label(&commands[0]));
    println!("last={}", command_label(&commands[3]));
}
