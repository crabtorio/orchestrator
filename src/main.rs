use crate::galaxy_generator::Galaxy;
use crate::logging::init;
use clap::Parser;
use cli::Cli;
use cli::Command::Generate;
mod cli;
mod galaxy_generator;
mod initialization;
mod logging;
mod orchestrator;

fn main() {
    init();
    let cli = Cli::parse();

    match cli.command {
        Generate {
            planet_count,
            expected_percentage,
        } => {
            Galaxy::from_random_distribution(planet_count, expected_percentage);
        }
    }
}
