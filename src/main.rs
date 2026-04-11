use crate::cli::AiMode;
use crate::galaxy_generator::Galaxy;
use crate::logging::init;
use crate::orchestrator::Orchestrator;
use crate::orchestrator::ai::Ai;
use crate::orchestrator::ai::{Auto, Manual};
use clap::Parser;
use cli::Cli;
use cli::Command::{Generate, Run};
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
            name,
            planet_count,
            expected_percentage,
        } => {
            let galaxy = Galaxy::from_random_distribution(planet_count, expected_percentage);
            //Then this galaxy must be serialized into its own file named <name>
        }
        Run { galaxy_name, mode } => {
            let ai: Box<dyn Ai> = match mode {
                AiMode::Auto => Box::new(Auto),
                AiMode::Manual => Box::new(Manual),
            };
            let galaxy: Galaxy; //Must be deserialized from the file <galaxy_name>
            //let orchestrator = Orchestrator::new(galaxy, ai);
            //orchestrator.run();
        }
    }
}
