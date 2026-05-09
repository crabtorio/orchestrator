use crate::{
    cli::CommandResult::{Generated, Running},
    logging::init,
};
use clap::Parser;
use cli::Cli;
mod cli;
mod galaxy_generator;
mod logging;
mod orchestrator;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    init();
    let cli = Cli::parse();

    match cli.command.run()? {
        Generated => {}
        Running(mut orchestrator) => {
            orchestrator.run();
        }
    }
    Ok(())
}
