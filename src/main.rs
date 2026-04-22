use crate::logging::init;
use clap::Parser;
use cli::Cli;
mod cli;
mod galaxy_generator;
mod logging;
mod orchestrator;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    init();
    let cli = Cli::parse();

    let galaxy = cli.command.run()?;

    Ok(())
}
