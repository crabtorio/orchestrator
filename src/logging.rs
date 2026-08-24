use env_logger::Builder;
use log::LevelFilter;

pub fn init() {
    // To silence all other crates' logging
    Builder::new()
        .filter(None, LevelFilter::Off)
        .filter(Some("orchestrator"), LevelFilter::Debug)
        .init();
}
