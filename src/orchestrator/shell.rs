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
    ExplorerVendor,
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
            match io::stdin().read_line(&mut input) {
                Ok(0) => break,
                Ok(_) => match parse_command(input.trim().to_string()) {
                    Ok(command) => match command {
                        Exit => {
                            self.user_queue.lock().unwrap().push_back(command);
                            break;
                        }
                        Nothing => {
                            print_commands();
                        }
                        _ => self.user_queue.lock().unwrap().push_back(command),
                    },
                    Err(error) => {
                        log::error!("Parsing error: {}", error);
                    }
                },
                Err(err) => log::error!("Shell error: {}", err),
            }
        }
    }
}
fn parse_command(mut input: String) -> Result<Command, String> {
    input.make_ascii_lowercase();
    let parts: Vec<&str> = input.split_whitespace().collect();
    if parts.is_empty() {
        return Err(String::from("Command is empty"));
    }
    let command = parts[0];

    let arguments = &parts[1..];

    match command {
        "resumeexplorers" => Ok(ResumeExplorers),
        "stopexplorers" => Ok(StopExplorers),
        "killexplorers" => Ok(KillExplorers),
        "spawnexplorer" => Ok(SpawnExplorer {
            explorer_id: if let Some(argument) = arguments.get(0) {
                if *argument == "0" {
                    super::ExplorerID::First
                } else {
                    super::ExplorerID::Second
                }
            } else {
                return Err(String::from(
                    "No arguments given, but needed by 'stopexplorer' command",
                ));
            },
            vendor: {
                if let Some(argument) = arguments.get(1) {
                    if let Ok(explorer_vendor) = string_to_vendor(*argument) {
                        explorer_vendor
                    } else {
                        return Err(String::from("Invalid explorer vendor"));
                    }
                } else {
                    return Err(String::from("Missing second argument: explorer_vendor"));
                }
            },
            destination_planet: {
                if let Some(argument) = arguments.get(2) {
                    argument.parse::<ID>().map_err(|err| err.to_string())?
                } else {
                    return Err(String::from(
                        "No arguments given, but needed by 'stopexplorer' command",
                    ));
                }
            },
        }),
        "stopexplorer" => Ok(StopExplorer({
            if let Some(argument) = arguments.get(0) {
                if *argument == "0" {
                    super::ExplorerID::First
                } else {
                    super::ExplorerID::Second
                }
            } else {
                return Err(String::from(
                    "No arguments given, but needed by 'stopexplorer' command",
                ));
            }
        })),
        "killexplorer" => Ok(KillExplorer({
            if let Some(argument) = arguments.get(0) {
                if *argument == "0" {
                    super::ExplorerID::First
                } else {
                    super::ExplorerID::Second
                }
            } else {
                return Err(String::from(
                    "No arguments given, but needed by 'killexplorer' command",
                ));
            }
        })),
        "resetexplorer" => Ok({
            if let Some(explorer_id) = arguments.get(0) {
                let explorer_id = if *explorer_id == "0" {
                    super::ExplorerID::First
                } else {
                    super::ExplorerID::Second
                };
                Command::ResetExplorer(explorer_id)
            } else {
                return Err(format!(
                    "Not enough arguments given, expected <explorer_id>"
                ));
            }
        }),
        "startexplorer" => Ok(StartExplorer({
            if let Some(argument) = arguments.get(0) {
                if *argument == "0" {
                    super::ExplorerID::First
                } else {
                    super::ExplorerID::Second
                }
            } else {
                return Err(format!(
                    "No arguments given, but needed by 'resumeexplorer' command"
                ));
            }
        })),
        "moveexplorer" => Ok(MoveExplorer {
            planet_id: if let Some(argument) = arguments.get(0) {
                argument.parse::<ID>().map_err(|err| err.to_string())?
            } else {
                return Err(String::from(
                    "No argument given, needed by 'moveexplorer' command",
                ));
            },
            explorer: if let Some(argument) = arguments.get(1) {
                if *argument == "0" {
                    super::ExplorerID::First
                } else {
                    super::ExplorerID::Second
                }
            } else {
                return Err(String::from(
                    "Missing second argument, needed by 'moveexplorer' command",
                ));
            },
        }),
        "currentplanetrequest" => Ok(CurrentPlanetRequest({
            if let Some(argument) = arguments.get(0) {
                if *argument == "0" {
                    super::ExplorerID::First
                } else {
                    super::ExplorerID::Second
                }
            } else {
                return Err(String::from(
                    "No arguments given, but needed by 'currentplanetrequest' command",
                ));
            }
        })),
        "supportedresourcerequest" => Ok(SupportedResourceRequest({
            if let Some(argument) = arguments.get(0) {
                if *argument == "0" {
                    super::ExplorerID::First
                } else {
                    super::ExplorerID::Second
                }
            } else {
                return Err(String::from(
                    "No arguments given, but needed by 'supportedresourcerequest' command",
                ));
            }
        })),
        "supportedcombinationrequest" => Ok(SupportedCombinationRequest({
            if let Some(argument) = arguments.get(0) {
                if *argument == "0" {
                    super::ExplorerID::First
                } else {
                    super::ExplorerID::Second
                }
            } else {
                return Err(String::from(
                    "No arguments given, but needed by 'supportedcombinationrequest' command",
                ));
            }
        })),
        "generateresourcerequest" => Ok(GenerateResourceRequest(
            if let Some(argument) = arguments.get(0) {
                if *argument == "0" {
                    super::ExplorerID::First
                } else {
                    super::ExplorerID::Second
                }
            } else {
                return Err(String::from(
                    "No arguments given, but needed by 'generateresourcerequest' command",
                ));
            },
            if let Ok(ResourceType::Basic(res)) =
                string_to_resource(if let Some(argument) = arguments.get(1) {
                    *argument
                } else {
                    return Err(String::from(
                        "Second argument missing, needed by 'generateresourcerequest' command",
                    ));
                })
            {
                res
            } else {
                return Err(String::from(
                    "expected Basic resource, found complex resource",
                ));
            },
        )),
        "combineresourcerequest" => Ok(CombineResourceRequest(
            if let Some(argument) = arguments.get(0) {
                if *argument == "0" {
                    super::ExplorerID::First
                } else {
                    super::ExplorerID::Second
                }
            } else {
                return Err(String::from(
                    "No arguments given, but needed by 'combineresourcerequest' command",
                ));
            },
            if let Ok(ResourceType::Complex(res)) =
                string_to_resource(if let Some(argument) = arguments.get(1) {
                    *argument
                } else {
                    return Err(String::from(
                        "Second argument missing, needed by 'combineresourcerequest' command",
                    ));
                })
            {
                res
            } else {
                return Err(String::from(
                    "expected Complex resource, found basic resource",
                ));
            },
        )),
        "bagcontentrequest" => Ok(BagContentRequest({
            if let Some(argument) = arguments.get(0) {
                if *argument == "0" {
                    super::ExplorerID::First
                } else {
                    super::ExplorerID::Second
                }
            } else {
                return Err(String::from(
                    "No arguments given, but needed by 'bagcontentrequest' command",
                ));
            }
        })),
        "startplanets" => Ok(StartPlanets),
        "stopplanets" => Ok(StopPlanets),
        "killplanets" => Ok(KillPlanets),
        "startplanet" => Ok(StartPlanet(if let Some(argument) = arguments.get(0) {
            argument.parse::<ID>().map_err(|err| err.to_string())?
        } else {
            return Err(String::from(
                "No argument given, needed by 'startplanet' command",
            ));
        })),
        "stopplanet" => Ok(StopPlanet(if let Some(argument) = arguments.get(0) {
            argument.parse::<ID>().map_err(|err| err.to_string())?
        } else {
            return Err(String::from(
                "No argument given, needed by 'stopplanet' command",
            ));
        })),
        "killplanet" => Ok(KillPlanet(if let Some(argument) = arguments.get(0) {
            argument.parse::<ID>().map_err(|err| err.to_string())?
        } else {
            return Err(String::from(
                "No argument given, needed by 'killplanet' command",
            ));
        })),
        "sendsunray" => Ok(SendSunray(if let Some(argument) = arguments.get(0) {
            argument.parse::<ID>().map_err(|err| err.to_string())?
        } else {
            return Err(String::from(
                "No argument given, needed by 'sendsunray' command",
            ));
        })),
        "sendasteroid" => Ok(SendAsteroid(if let Some(argument) = arguments.get(0) {
            argument.parse::<ID>().map_err(|err| err.to_string())?
        } else {
            return Err(String::from(
                "No argument given, needed by 'sendasteroid' command",
            ));
        })),
        "internalstaterequest" => Ok(InternalStateRequest(
            if let Some(argument) = arguments.get(0) {
                argument.parse::<ID>().map_err(|err| err.to_string())?
            } else {
                return Err(String::from(
                    "No argument given, needed by 'internalstaterequest' command",
                ));
            },
        )),
        "spawnai" => Ok(SpawnAi(
            if let Ok(ai_type) = string_to_ai(if let Some(argument) = arguments.get(0) {
                *argument
            } else {
                return Err(String::from(
                    "No argument given, needed by 'spawnai' command",
                ));
            }) {
                ai_type
            } else {
                return Err(String::from("Not a valid Ai format"));
            },
        )),
        "killai" => Ok(KillAi(if let Some(argument) = arguments.get(0) {
            argument.parse::<ID>().map_err(|err| err.to_string())?
        } else {
            return Err(String::from(
                "No argument given, needed by 'killai' command",
            ));
        })),
        "showrunningais" => Ok(ShowRunningAis),
        "exit" => Ok(Exit),
        "help" => Ok(Nothing),
        _ => Err(format!("Command '{}' is invalid", command)),
    }
}
fn string_to_ai(str: &str) -> Result<AiType, ()> {
    match str.to_ascii_lowercase().as_str() {
        "richardrandom" => Ok(RichardRandom),
        _ => Err(()),
    }
}
fn string_to_vendor(str: &str) -> Result<ExplorerVendor, ()> {
    match str.to_ascii_lowercase().as_str() {
        "lorenzo" => Ok(ExplorerVendor::Lorenzo),
        "alessio" => Ok(ExplorerVendor::Alessio),
        "luca" => Ok(ExplorerVendor::Luca),
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
        "aipartner" => Ok(ResourceType::Complex(AIPartner)),
        _ => Err(()),
    }
}
fn print_commands() {
    print!(
        "Available commands:
  Planets:
    startplanets / stopplanets / killplanets
    startplanet <id> / stopplanet <id> / killplanet <id>
    sendsunray <id> / sendasteroid <id>
    internalstaterequest <id>

  Explorers:
    resumeexplorers / stopexplorers / killexplorers
    spawnexplorer <0|1> <vendor> <destination_planet_id>
    startexplorer <0|1> / stopexplorer <0|1> / killexplorer <0|1> / resetexplorer <0|1>
    moveexplorer <planet_id> <0|1>

  Explorer queries:
    currentplanetrequest <0|1>
    supportedresourcerequest <0|1>
    supportedcombinationrequest <0|1>
    bagcontentrequest <0|1>
    generateresourcerequest <0|1> <resource>
    combineresourcerequest <0|1> <resource>

  AI:
    spawnai <ai_type>
    killai <id>
    showrunningais

  Other:
    help
    exit

  Values:
    <vendor>   = lorenzo | alessio | luca
    <ai_type>  = richardrandom
    <resource> (basic)   = oxygen | hydrogen | carbon | silicon
    <resource> (complex) = diamond | water | life | robot | dolphin | aipartner
"
    );
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
