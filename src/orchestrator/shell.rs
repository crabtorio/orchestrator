use std::{
    collections::VecDeque,
    io::{self, Write},
    sync::{Arc, Mutex},
};

use common_game::utils::ID;

use crate::orchestrator::Command::{self, Exit, StartPlanet, StartPlanets};

pub struct Shell {
    user_queue: Arc<Mutex<VecDeque<Command>>>,
}

impl Shell {
    pub fn new(user_queue: Arc<Mutex<VecDeque<Command>>>) -> Self {
        Self { user_queue }
    }
    pub fn run(&self) {
        loop {
            print!("> ");
            io::stdout().flush().unwrap();
            let mut input = String::new();
            if let Ok(_) = io::stdin().read_line(&mut input) {
                match parse_command(input.trim().to_string()) {
                    Ok(command) => {
                        if let Exit = command {
                            self.user_queue.lock().unwrap().push_back(command);
                            break;
                        }
                        self.user_queue.lock().unwrap().push_back(command);
                    }
                    Err(error) => {
                        println!("Error: {}", error);
                    }
                }
            }
        }
    }
}
fn parse_command(mut input: String) -> Result<Command, String> {
    input.make_ascii_lowercase();
    let parts: Vec<&str> = input.split_whitespace().collect();
    let command = parts[0];

    let arguments = &parts[1..];

    match command {
        "startplanets" => Ok(StartPlanets),
        "exit" => Ok(Exit),
        "startplanet" => {
            print!("dsaas: {:?}", arguments[0].parse::<ID>());
            Ok(StartPlanet(
                arguments[0].parse::<ID>().map_err(|err| err.to_string())?,
            ))
        }
        "quit" => Ok(Exit),
        _ => Err(format!("Command '{}' is invalid", command)),
    }
}

#[cfg(test)]
mod test {
    use super::parse_command;
    use crate::orchestrator::Command;

    #[test]
    fn test_parse_command() {
        let test1 = parse_command("startplanets".to_string());
        let test2 = parse_command("StartPlanet 102".to_string());

        assert!(matches!(test1, Ok(Command::StartPlanets)));
        assert!(matches!(test2, Ok(Command::StartPlanet(102))));
    }
}
