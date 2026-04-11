use crate::{galaxy_generator::Galaxy, orchestrator::ai::Ai};
pub mod ai;
pub struct Orchestrator {
    galaxy: Galaxy,
    ai: Box<dyn Ai>,
}
impl Orchestrator {
    pub fn new(galaxy: Galaxy, ai: Box<dyn Ai>) -> Self {
        Orchestrator { galaxy, ai }
    }
    pub fn run(&mut self) {
        //Todo
    }
}
