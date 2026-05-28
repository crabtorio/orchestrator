use crate::galaxy_generator::{Galaxy, PlanetContainer};
use common_game::{
    protocols::{orchestrator_planet, planet_explorer},
    utils::ID,
};
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
struct PlanetHandle {
    id: ID,
    handle: JoinHandle<()>,
    tx_planet: crossbeam_channel::Sender<orchestrator_planet::OrchestratorToPlanet>,
    rx_planet: crossbeam_channel::Receiver<orchestrator_planet::PlanetToOrchestrator>,
    tx_explorer: crossbeam_channel::Sender<planet_explorer::ExplorerToPlanet>,
}
impl PlanetHandle {
    fn spawn(planet: Arc<Mutex<PlanetContainer>>) -> Self {
        let (tx_planet, rx_planet, tx_explorer, id) = {
            let lock = planet.lock().unwrap();
            (
                lock.tx_planet.clone(),
                lock.rx_planet.clone(),
                lock.tx_explorer.clone(),
                lock.id(),
            )
        };
        PlanetHandle {
            id,
            handle: thread::spawn(move || planet.lock().unwrap().run()),
            tx_planet,
            rx_planet,
            tx_explorer,
        }
    }
    fn start_planet(&self) {
        match self
            .tx_planet
            .send(orchestrator_planet::OrchestratorToPlanet::StartPlanetAI)
        {
            Ok(()) => (),
            Err(_) => log::error!("Could not start planet {}", self.id),
        }
    }
    fn stop_planet(&self) {
        match self
            .tx_planet
            .send(orchestrator_planet::OrchestratorToPlanet::StopPlanetAI)
        {
            Ok(()) => (),
            Err(_) => log::error!("Could not stop planet {}", self.id),
        }
    }
    fn join_thread(self) {
        match self.handle.join() {
            Ok(()) => (),
            Err(_) => log::error!("Could not join thread of planet {}", self.id),
        }
    }
}
impl Orchestrator {
    pub fn new(galaxy: Galaxy, auto: bool) -> Self {
        Orchestrator { galaxy, auto }
    }
    pub fn run(&mut self) {
        if self.auto {
            print!("Ciao");
            let planet_handles: HashMap<ID, PlanetHandle> = self
                .galaxy
                .iter()
                .map(|planet| {
                    (
                        planet.lock().unwrap().id(),
                        PlanetHandle::spawn(planet.clone()),
                    )
                })
                .collect();

            //Start all planets
            for (_, handle) in &planet_handles {
                handle.start_planet();
            }

            // Join all planet threads
            for (_, handle) in planet_handles {
                handle.join_thread();
            }
        } else {
        }
    }
}
