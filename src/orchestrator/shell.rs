use std::{
    collections::VecDeque,
    io::{self, Write},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering::Relaxed},
    },
};

use common_game::utils::ID;

use crate::orchestrator::Command::{self, Exit, StartAllPlanets, StartPlanet};

pub struct Shell {
    user_queue: Arc<Mutex<VecDeque<Command>>>,
    run_flag: Arc<AtomicBool>,
}

impl Shell {
    pub fn new(user_queue: Arc<Mutex<VecDeque<Command>>>, run_flag: Arc<AtomicBool>) -> Self {
        Self {
            user_queue,
            run_flag,
        }
    }
    pub fn run(&self) {
        loop {
            print!("> ");
            io::stdout().flush().unwrap();
            let mut input = String::new();
            if let Ok(_) = io::stdin().read_line(&mut input) {
                // May need to make the read_line non-blocking, so that the flag is checked periodically using a timer.
                // Though during testing the orchestrator is faster at modyfing the run_flag than the shell is at reading it
                match parse_command(input.trim().to_string()) {
                    Ok(command) => {
                        self.user_queue.lock().unwrap().push_back(command);
                    }
                    Err(error) => {
                        println!("Error: {}", error);
                    }
                }
            }

            if !self.run_flag.load(Relaxed) {
                break;
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
        "startallplanets" => Ok(StartAllPlanets),
        "exit" => Ok(Exit),
        "startplanet" => {
            print!("dsaas: {:?}", arguments[0].parse::<ID>());
            Ok(StartPlanet(
                arguments[0].parse::<ID>().map_err(|err| err.to_string())?,
            ))
        }
        _ => Err(format!("Command '{}' is invalid", command)),
    }
}

#[cfg(test)]
mod test {
    use super::parse_command;
    use crate::orchestrator::Command;

    #[test]
    fn test_parse_command() {
        let test1 = parse_command("startallplanets".to_string());
        let test2 = parse_command("StartPlanet 102".to_string());

        assert!(matches!(test1, Ok(Command::StartAllPlanets)));
        assert!(matches!(test2, Ok(Command::StartPlanet(102))));
    }
}
