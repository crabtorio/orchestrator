use std::{
    collections::VecDeque,
    io::{self, Write},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering::Relaxed},
    },
};

use common_game::utils::ID;

use crate::orchestrator::Command::{self, Exit};

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
                match parse_command(&input.trim()) {
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
fn parse_command(input: &str) -> Result<Command, String> {
    let mut parts = input.rsplit(" ");
    let command = parts.next().unwrap_or("").to_lowercase();
    let arguments: Vec<&str> = parts.collect();

    match command.as_str() {
        "exit" => Ok(Exit),
        "startplanet" => Ok(start_planet(arguments)?),
        _ => Err(format!("Command '{}' doesn't exist", command)),
    }
}
fn start_planet(arguments: Vec<&str>) -> Result<Command, String> {
    if let Some(id) = arguments.get(0) {
        Ok(Command::StartPlanet(match id.parse::<ID>() {
            Ok(id) => id,
            Err(error) => return Err(error.to_string()),
        }))
    } else {
        Err(String::from("No argument found"))
    }
}
