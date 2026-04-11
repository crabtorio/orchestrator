use clap::{Parser, Subcommand, ValueEnum};

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
        mode: AiMode,
    },
}

#[derive(ValueEnum, Clone)]
pub enum AiMode {
    Manual,
    Auto,
}
