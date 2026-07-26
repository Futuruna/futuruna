// Downstream-shaped fixture: enum/reference loop aggregation with conditional accumulator rebinding.

#[derive(Clone, Debug)]
enum Command {
    Enable(String),
    Disable(String),
    Threshold(i64),
    Ignore,
}

fn command_label(command: &Command) -> String {
    match command {
        Command::Enable(name) => format!("enable {}", name),
        Command::Disable(name) => format!("disable {}", name),
        Command::Threshold(value) => format!("threshold {}", value),
        Command::Ignore => "ignore".to_string(),
    }
}

fn enabled_count(commands: &Vec<Command>) -> i64 {
    let mut total = 0;
    for command in commands {
        let label = command_label(command);
        if label.starts_with("enable") {
            total += 1;
        }
    }
    total
}

fn threshold_sum(commands: &Vec<Command>) -> i64 {
    let mut total = 0;
    for command in commands {
        let label = command_label(command);
        if label == "threshold 10" {
            total = total + 10;
        }
    }
    total
}

fn main() {
    let mut commands = Vec::new();
    commands.push(Command::Enable("search".to_string()));
    commands.push(Command::Threshold(10));
    commands.push(Command::Disable("ads".to_string()));
    commands.push(Command::Ignore);

    println!("commands={}", commands.len());
    println!("enabled={}", enabled_count(&commands));
    println!("thresholds={}", threshold_sum(&commands));
    println!("first={}", command_label(&commands[0]));
    println!("last={}", command_label(&commands[3]));
}
