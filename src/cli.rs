use crate::{
    galaxy_generator::Galaxy,
    orchestrator::{ExplorerVendor, Explorers, Orchestrator},
};
use clap::{Parser, Subcommand, ValueEnum};
use std::{
    collections::VecDeque,
    fs::{create_dir_all, read_to_string, write},
    io::Error,
    sync::{Arc, Mutex},
};

#[derive(Parser)]
pub struct Cli {
    #[command(subcommand)]
    pub command: ClapCommand,
}

#[derive(Subcommand)]
pub enum ClapCommand {
    Generate {
        name: String,
        #[arg(default_value_t = 7)]
        planet_count: i32,
        #[arg(default_value_t = 80.0)]
        expected_percentage: f64,
    },
    Run {
        galaxy_name: String,
        explorer1: ExplorerVendor,
        explorer2: Option<ExplorerVendor>,
    },
}

#[derive(ValueEnum, Clone)]
pub enum AiModeCommand {
    Manual,
    Auto,
}
pub enum CommandResult {
    Generated,
    Running(Orchestrator),
}
impl ClapCommand {
    pub fn run(self) -> Result<CommandResult, Error> {
        match self {
            ClapCommand::Generate {
                name: galaxy_name,
                planet_count,
                expected_percentage,
            } => {
                let galaxy = Galaxy::from_random_distribution(planet_count, expected_percentage);

                let json = serde_json::to_string_pretty(&galaxy)?;
                create_dir_all("./galaxies")?;
                write(format!("./galaxies/{}.json", galaxy_name), json)?;
                Ok(CommandResult::Generated)
            }
            ClapCommand::Run {
                galaxy_name,
                explorer1,
                explorer2,
            } => {
                let galaxy_str = read_to_string(format!("./galaxies/{}.json", galaxy_name))?;
                let galaxy: Galaxy = serde_json::from_str(&galaxy_str)?;
                let explorers = {
                    if let Some(explorer2) = explorer2 {
                        Explorers::Two(explorer1, explorer2)
                    } else {
                        Explorers::One(explorer1)
                    }
                };
                Ok(CommandResult::Running(Orchestrator::new(
                    galaxy,
                    Arc::new(Mutex::new(VecDeque::new())),
                    Arc::new(Mutex::new(VecDeque::new())),
                    explorers,
                )))
            }
        }
    }
}
