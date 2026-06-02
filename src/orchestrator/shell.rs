use crate::orchestrator::Command;
struct Shell;

impl Shell {
    fn run(&self) {}
}
fn parse_command(input: &str) -> Result<Command, String> {
    let mut parts = input.rsplit(" ");
    let command = parts.next().unwrap_or("").to_lowercase();
    let arguments: Vec<&str> = parts.collect();

    match command.as_str() {
        _ => Err(format!("Command '{}' doesn't exist", command)),
    }
}
