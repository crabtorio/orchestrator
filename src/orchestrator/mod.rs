use crate::{galaxy_generator::Galaxy, orchestrator::ai::Ai};
mod ai;
struct Orchestrator {
    galaxy: Galaxy,
    ai: Box<dyn Ai>,
    //todo
}
