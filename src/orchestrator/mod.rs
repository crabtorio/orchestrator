use crate::{initialization::Galaxy, orchestrator::ai::Ai};
mod ai;
struct Orchestrator {
    galaxy: Galaxy,
    ai: Box<dyn Ai>,
    //todo
}
