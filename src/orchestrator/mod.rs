use crate::galaxy_generator::Galaxy;
//pub mod ai;
pub struct Orchestrator {
    galaxy: Galaxy,
    auto: bool,
}
impl Orchestrator {
    pub fn new(galaxy: Galaxy, auto: bool) -> Self {
        Orchestrator { galaxy, auto }
    }
    pub fn run(&mut self) {
        //Todo
    }
}
