use crate::orchestrator::{Command, PlanetHandle};
use common_game::utils::ID;
use std::collections::HashMap;

pub trait Ai {
    fn run(&self, planet_handles: &HashMap<ID, PlanetHandle>);
}

pub struct Auto;

impl Ai for Auto {
    fn run(&self, planet_handles: &HashMap<ID, PlanetHandle>) {
        //Todo
    }
}
