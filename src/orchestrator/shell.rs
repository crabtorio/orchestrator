use std::{
    collections::VecDeque,
    io::{self, Write},
    sync::{Arc, Mutex},
};

use common_game::{
    components::resource::{
        BasicResourceType::*,
        ComplexResourceType::*,
        ResourceType::{self},
    },
    utils::ID,
};

use crate::orchestrator::{
    Command::{self, *},
    ai::AiType::{self, RichardRandom},
};

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
        "startexplorers" => Ok(StartExplorers),
        "stopexplorers" => Ok(StopExplorers),
        "killexplorers" => Ok(KillExplorers),
        "resetexplorers" => Ok(ResetExplorers),
        "startexplorer" => Ok(StartExplorer({
            if arguments[0] == "0" {
                super::ExplorerID::First
            } else {
                super::ExplorerID::Second
            }
        })),
        "stopexplorer" => Ok(StopExplorer({
            if arguments[0] == "0" {
                super::ExplorerID::First
            } else {
                super::ExplorerID::Second
            }
        })),
        "killexplorer" => Ok(KillExplorer({
            if arguments[0] == "0" {
                super::ExplorerID::First
            } else {
                super::ExplorerID::Second
            }
        })),
        "resetexplorer" => Ok(ResetExplorer({
            if arguments[0] == "0" {
                super::ExplorerID::First
            } else {
                super::ExplorerID::Second
            }
        })),
        "moveexplorer" => Ok(MoveExplorer {
            planet_id: arguments[0].parse::<ID>().map_err(|err| err.to_string())?,
            explorer: if arguments[1] == "0" {
                super::ExplorerID::First
            } else {
                super::ExplorerID::Second
            },
        }),
        "currentplanetrequest" => Ok(CurrentPlanetRequest({
            if arguments[0] == "0" {
                super::ExplorerID::First
            } else {
                super::ExplorerID::Second
            }
        })),
        "supportedresourcerequest" => Ok(SupportedResourceRequest({
            if arguments[0] == "0" {
                super::ExplorerID::First
            } else {
                super::ExplorerID::Second
            }
        })),
        "supportedcombinationrequest" => Ok(SupportedCombinationRequest({
            if arguments[0] == "0" {
                super::ExplorerID::First
            } else {
                super::ExplorerID::Second
            }
        })),
        "generateresourcerequest" => Ok(GenerateResourceRequest(
            if arguments[0] == "0" {
                super::ExplorerID::First
            } else {
                super::ExplorerID::Second
            },
            if let Ok(ResourceType::Basic(res)) = string_to_resource(arguments[1]) {
                res
            } else {
                return Err(format!(
                    "expected Basic resource, found complex resource: {}",
                    arguments[1]
                ));
            },
        )),
        "combineresourcerequest" => Ok(CombineResourceRequest(
            if arguments[0] == "0" {
                super::ExplorerID::First
            } else {
                super::ExplorerID::Second
            },
            if let Ok(ResourceType::Complex(res)) = string_to_resource(arguments[1]) {
                res
            } else {
                return Err(format!(
                    "expected Complex resource, found complex resource: {}",
                    arguments[1]
                ));
            },
        )),
        "bagcontentrequest" => Ok(BagContentRquest({
            if arguments[0] == "0" {
                super::ExplorerID::First
            } else {
                super::ExplorerID::Second
            }
        })),
        "startplanets" => Ok(StartPlanets),
        "stopplanets" => Ok(StopPlanets),
        "killplanets" => Ok(KillPlanets),
        "startplanet" => Ok(StartPlanet(
            arguments[0].parse::<ID>().map_err(|err| err.to_string())?,
        )),
        "stopplanet" => Ok(StopPlanet(
            arguments[0].parse::<ID>().map_err(|err| err.to_string())?,
        )),
        "killplanet" => Ok(KillPlanet(
            arguments[0].parse::<ID>().map_err(|err| err.to_string())?,
        )),
        "sendsunray" => Ok(SendSunray(
            arguments[0].parse::<ID>().map_err(|err| err.to_string())?,
        )),
        "sendasteroid" => Ok(SendAsteroid(
            arguments[0].parse::<ID>().map_err(|err| err.to_string())?,
        )),
        "internalstaterequest" => Ok(InternalStateRequest(
            arguments[0].parse::<ID>().map_err(|err| err.to_string())?,
        )),
        "spawnai" => Ok(SpawnAi(if let Ok(ai_type) = string_to_ai(arguments[0]) {
            ai_type
        } else {
            return Err(format!("{} is not a valid Ai format", arguments[0]));
        })),
        "killai" => Ok(KillAi(
            arguments[0].parse::<ID>().map_err(|err| err.to_string())?,
        )),
        "showrunningais" => Ok(ShowRunningAis),
        "exit" => Ok(Exit),
        _ => Err(format!("Command '{}' is invalid", command)),
    }
}
fn string_to_ai(str: &str) -> Result<AiType, ()> {
    match str.to_ascii_lowercase().as_str() {
        "richardrandom" => Ok(RichardRandom),
        _ => Err(()),
    }
}
fn string_to_resource(str: &str) -> Result<ResourceType, ()> {
    match str.to_ascii_lowercase().as_str() {
        "oxygen" => Ok(ResourceType::Basic(Oxygen)),
        "hydrogen" => Ok(ResourceType::Basic(Hydrogen)),
        "carbon" => Ok(ResourceType::Basic(Carbon)),
        "silicon" => Ok(ResourceType::Basic(Silicon)),
        "diamond" => Ok(ResourceType::Complex(Diamond)),
        "water" => Ok(ResourceType::Complex(Water)),
        "life" => Ok(ResourceType::Complex(Life)),
        "robot" => Ok(ResourceType::Complex(Robot)),
        "dolphin" => Ok(ResourceType::Complex(Dolphin)),
        "aIPartner" => Ok(ResourceType::Complex(AIPartner)),
        _ => Err(()),
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
