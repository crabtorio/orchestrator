use crate::{galaxy_generator::Galaxy, orchestrator::Orchestrator};
use clap::{Parser, Subcommand, ValueEnum};
use std::{
    fs::{create_dir_all, read_to_string, write},
    io::Error,
};

#[derive(Parser)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    Generate {
        name: String,
        #[arg(default_value_t = 7)]
        planet_count: i32,
        #[arg(default_value_t = 80.0)]
        expected_percentage: f64,
    },
    Run {
        galaxy_name: String,
        mode: AiModeCommand,
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
impl Command {
    pub fn run(self) -> Result<CommandResult, Error> {
        match self {
            Command::Generate {
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
            Command::Run { galaxy_name, mode } => {
                let galaxy_str = read_to_string(format!("./galaxies/{}.json", galaxy_name))?;
                let galaxy: Galaxy = serde_json::from_str(&galaxy_str)?;
                let mode = match mode {
                    AiModeCommand::Manual => false,
                    AiModeCommand::Auto => true,
                };
                Ok(CommandResult::Running(Orchestrator::new(galaxy, mode)))
            }
        }
    }
}
