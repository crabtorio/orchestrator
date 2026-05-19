use crate::galaxy_generator::{Galaxy, PlanetContainer};
use common_game::utils::ID;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    thread::{self, JoinHandle},
};
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
        match self.auto {
            true => {
                let mut planet_threads: HashMap<ID, JoinHandle<Arc<Mutex<PlanetContainer>>>>;
                for planet in self.galaxy.iter() {
                    //let handle = thread::spawn(move || planet.); Needs the galaxy to store Arc<Mutex<>>
                    //planet_threads.insert(planet.borrow().id(), handle);
                }
            }
            false => {}
        }
    }
}
