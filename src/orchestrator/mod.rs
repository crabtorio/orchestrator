mod explorer_handle;
mod shell;

use crate::galaxy_generator::{Galaxy, PlanetContainer};
use crate::orchestrator::Command::*;
use crate::orchestrator::ai::AiType::RichardRandom as RichardRandomType;
use crate::orchestrator::ai::{Ai, AiArgs, AiType, RichardRandom};
use crate::orchestrator::explorer_handle::{RunningExploererHandle, UnbornExploererHandle};
use crate::orchestrator::shell::Shell;
use common_game::components::asteroid::Asteroid;
use common_game::components::planet::DummyPlanetState;
use common_game::components::resource::{BasicResourceType, ComplexResourceType};
use common_game::components::sunray::Sunray;
use common_game::protocols::orchestrator_planet::PlanetToOrchestrator::AsteroidAck;
use common_game::{
    protocols::{orchestrator_explorer, orchestrator_planet, planet_explorer},
    utils::ID,
};
use explorer_common::Bag;
use orchestrator_explorer::{
    ExplorerToOrchestrator, ExplorerToOrchestratorKind, OrchestratorToExplorer,
};
use orchestrator_planet::{OrchestratorToPlanet, PlanetToOrchestrator};
use std::collections::VecDeque;
use std::sync::atomic::Ordering::{Relaxed, Release};
use std::sync::atomic::{AtomicBool, AtomicU32};
use std::thread::sleep;
use std::time::Duration;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    thread::{self, JoinHandle},
};
use std::{todo, vec};
pub mod ai;

pub struct Orchestrator {
    galaxy: Galaxy,
    ai_queue: Arc<Mutex<VecDeque<Command>>>,
    user_queue: Arc<Mutex<VecDeque<Command>>>,
    explorers: Explorers,
}
pub struct AiHandle {
    id: ID,
    handle: JoinHandle<()>,
    run_flag: Arc<AtomicBool>, // Ais will check periodically this flag and return if false. Only way to stop a thread from the outside
                               // We could potentially add corssbeam channels to communicate with the AI if we ever wanted
}
impl AiHandle {
    fn new(ai: Box<dyn Ai>, queue: Arc<Mutex<VecDeque<Command>>>, args: AiArgs) -> Self {
        static NEXT_ID: AtomicU32 = AtomicU32::new(0);
        let run_flag = Arc::new(AtomicBool::new(true));
        let closure_flag = run_flag.clone();
        Self {
            id: NEXT_ID.load(Relaxed),
            handle: { thread::spawn(move || ai.run(queue, closure_flag, args)) },
            run_flag: run_flag,
        }
    }
}
pub struct PlanetHandle {
    id: ID,
    handle: JoinHandle<Result<(), String>>,
    tx_planet: crossbeam_channel::Sender<orchestrator_planet::OrchestratorToPlanet>,
    rx_planet: crossbeam_channel::Receiver<orchestrator_planet::PlanetToOrchestrator>,
    tx_explorer: crossbeam_channel::Sender<planet_explorer::ExplorerToPlanet>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExplorerID {
    First = 0,
    Second = 1,
}

pub enum Explorers {
    One(ExplorerVendor),
    Two(ExplorerVendor, ExplorerVendor),
}
#[derive(clap::ValueEnum, Clone, Copy)]
pub enum ExplorerVendor {
    Lorenzo,
    Alessio,
    Luca,
}

impl std::fmt::Display for ExplorerID {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            ExplorerID::First => "0",
            ExplorerID::Second => "1",
        })
    }
}

pub enum Command {
    // Explorer
    StartExplorers,
    StopExplorers,
    KillExplorers,
    ResetExplorers,
    StartExplorer(ExplorerID),
    StopExplorer(ExplorerID),
    KillExplorer(ExplorerID),
    ResetExplorer(ExplorerID),
    MoveExplorer { planet_id: ID, explorer: ExplorerID },
    // Manual mode explorer
    CurrentPlanetRequest(ExplorerID),
    SupportedResourceRequest(ExplorerID),
    SupportedCombinationRequest(ExplorerID),
    GenerateResourceRequest(ExplorerID, BasicResourceType),
    CombineResourceRequest(ExplorerID, ComplexResourceType),
    BagContentRquest(ExplorerID),

    // Planet
    StartPlanets,
    StopPlanets,
    KillPlanets,
    StartPlanet(ID),
    StopPlanet(ID),
    KillPlanet(ID),
    SendSunray(ID),
    SendAsteroid(ID),
    InternalStateRequest(ID),

    // Orchestrator AI
    //SpawnShell,
    SpawnAi(AiType),
    KillAi(ID),     // ai_handles ID
    ShowRunningAis, // Also shows AI IDs
    Exit,
}

enum Event {
    Command(Command),
}

impl Orchestrator {
    pub fn new(
        galaxy: Galaxy,
        ai_queue: Arc<Mutex<VecDeque<Command>>>,
        user_queue: Arc<Mutex<VecDeque<Command>>>,
        explorers: Explorers,
    ) -> Self {
        Orchestrator {
            galaxy,
            ai_queue,
            user_queue,
            explorers,
        }
    }
    pub fn run(&mut self) {
        let planet_handles: HashMap<ID, PlanetHandle> = self
            .galaxy
            .iter()
            .map(|planet| {
                let id = planet.lock().unwrap().id();
                (id, PlanetHandle::spawn(planet.clone()))
            })
            .collect();
        let mut ai_handles: HashMap<ID, AiHandle> = HashMap::new();

        let shell = Shell::new(self.user_queue.clone());
        let shell_handle = thread::spawn(move || shell.run());

        let mut running_explorer_handles = HashMap::new();

        //Add the unborn explorer handles to the appropriate map
        let mut unborn_explorer_handles = match &self.explorers {
            Explorers::One(explorer_vendor) => HashMap::from([(
                ExplorerID::First,
                explorer_handle::new(ExplorerID::First, *explorer_vendor),
            )]),
            Explorers::Two(explorer_vendor1, explorer_vendor2) => HashMap::from([
                (
                    ExplorerID::First,
                    explorer_handle::new(ExplorerID::First, *explorer_vendor1),
                ),
                (
                    ExplorerID::Second,
                    explorer_handle::new(ExplorerID::Second, *explorer_vendor1),
                ),
            ]),
        };

        loop {
            sleep(Duration::from_millis(50));
            let next_command = {
                let command = self
                    .user_queue
                    .lock()
                    .unwrap()
                    .pop_front()
                    .or_else(|| self.ai_queue.lock().unwrap().pop_front());

                if let Some(command) = command {
                    command
                } else {
                    if let Err(explorer_index) =
                        self.poll_and_handle_first_req(&mut running_explorer_handles)
                    {
                        //An error occured while handling an explorer's request.
                        //In this case we chose to kill the explorer and continue execution with a single explorer
                        log::info!("Attempting to kill explorer due to previous error");
                        let explorer = running_explorer_handles
                            .remove(&explorer_index)
                            .expect("Explorer got removed");
                        match explorer.kill_explorer_ai() {
                            Ok(join_handle) => {
                                join_handle.join();
                            }
                            Err(_) => log::error!(
                                "Could not shut down explorer cleanly, continuing anyway"
                            ),
                        }
                    }
                    continue;
                }
            };

            match next_command {
                Exit => break,
                StartPlanets => {
                    for (_, handle) in &planet_handles {
                        handle.start_planet();
                    }
                }
                StopPlanets => {
                    for (_, handle) in &planet_handles {
                        handle.stop_planet();
                    }
                }
                KillPlanets => {
                    for (_, handle) in &planet_handles {
                        handle.kill_planet();
                    }
                }
                StartPlanet(id) => planet_handles[&id].start_planet(),
                StopPlanet(id) => planet_handles[&id].stop_planet(),
                KillPlanet(id) => planet_handles[&id].kill_planet(),
                SendSunray(id) => planet_handles[&id].send_sunray(),
                SendAsteroid(id) => planet_handles[&id].send_asteroid(),
                SpawnAi(ai) => {
                    let new_ai = AiHandle::new(
                        match ai {
                            RichardRandomType => Box::new(RichardRandom),
                        },
                        self.ai_queue.clone(),
                        AiArgs::RichardRandom(
                            0,
                            (self.galaxy.planets.len() - 1) as ID
                        )
                    );
                    ai_handles.insert(new_ai.id, new_ai);
                }
                KillAi(id) => ai_handles[&id].run_flag.store(false, Release), // Not sure about the right ordering, check later
                ShowRunningAis => {
                    println!("Running AIs");
                    for (_, handle) in &ai_handles {
                        println!("id: {}", handle.id);
                    }
                }
                StartExplorers => todo!(),
                StopExplorers => todo!(),
                KillExplorers => todo!(),
                ResetExplorers => todo!(),
                StartExplorer(explorer_id) => todo!(),
                StopExplorer(explorer_id) => todo!(),
                KillExplorer(explorer_id) => todo!(),
                ResetExplorer(explorer_id) => todo!(),
                MoveExplorer {
                    planet_id,
                    explorer,
                } => todo!(),
                CurrentPlanetRequest(explorer_id) => todo!(),
                SupportedResourceRequest(explorer_id) => todo!(),
                SupportedCombinationRequest(explorer_id) => todo!(),
                BagContentRquest(explorer_id) => todo!(),
                InternalStateRequest(_) => todo!(),
                GenerateResourceRequest(explorer_id, basic_resource_type) => todo!(),
                CombineResourceRequest(explorer_id, complex_resource_type) => todo!(),
            }
        }

        //This is for debug, planets should be started, stopped and killed by the user. The only thing that stays below is the thread joining

        // Start all planets
        for (_, handle) in &planet_handles {
            handle.start_planet();
        }

        // Stop and kill planets then join the thread
        for (_, handle) in planet_handles {
            handle.stop_planet();
            handle.kill_planet();
            handle.join_thread();
        }
        shell_handle.join();
    }

    ///Poll explorers and handle first found request. Reutnrs `Err(explorer_index)` if there is an error while handling an explorer's request
    fn poll_and_handle_first_req(
        &mut self,
        explorers: &mut HashMap<ExplorerID, RunningExploererHandle>,
    ) -> Result<(), ExplorerID> {
        for (explorer_index, explorer) in explorers.iter_mut() {
            let result = explorer.handle_one_request(&self.galaxy);
            match result {
                Ok(true) => return Ok(()),
                Ok(false) => continue,
                Err(()) => return Err(*explorer_index),
            }
        }

        Ok(())
    }
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
        match self.tx_planet.send(OrchestratorToPlanet::StartPlanetAI) {
            Ok(()) => log::info!("StartPlanetAI message sent to planet {}", self.id),
            Err(_) => log::error!("Could not send StartPlanetAI message to planet {}", self.id),
        }
        match self.rx_planet.recv() {
            Ok(message) => {
                if let PlanetToOrchestrator::StartPlanetAIResult { planet_id } = message {
                    log::info!("Planet {} successfully started", self.id);
                } else {
                    log::debug!(
                        "Expected StartPlanetAIResult, received {:?}, while starting planet {}",
                        message,
                        self.id
                    );
                }
            }
            Err(error) => {
                log::error!(
                    "Error while waiting for StartPlanetAIResult message from planet {}, error: {}",
                    self.id,
                    error
                )
            }
        }
    }
    fn stop_planet(&self) {
        match self.tx_planet.send(OrchestratorToPlanet::StopPlanetAI) {
            Ok(()) => log::info!("StopPlanetAI message sent to planet {}", self.id),
            Err(_) => log::error!("Could not send StopPlanetAI message to planet {}", self.id),
        }
        match self.rx_planet.recv() {
            Ok(message) => {
                if let PlanetToOrchestrator::StopPlanetAIResult { planet_id } = message {
                    log::info!("Planet {} successfully stopped", self.id);
                } else {
                    log::debug!(
                        "Expected StopPlanetAIResult, received {:?}, while stopping planet {}",
                        message,
                        self.id
                    );
                }
            }
            Err(error) => {
                log::error!(
                    "Error while waiting for StopPlanetAIResult message from planet {}, error: {}",
                    self.id,
                    error
                )
            }
        }
    }
    fn kill_planet(&self) {
        match self.tx_planet.send(OrchestratorToPlanet::KillPlanet) {
            Ok(()) => log::info!("KillPlanet message sent to planet {}", self.id),
            Err(_) => log::error!("Could not send KillPlanet message to planet {}", self.id),
        }
        match self.rx_planet.recv() {
            Ok(message) => {
                if let PlanetToOrchestrator::KillPlanetResult { planet_id } = message {
                    log::info!("Planet {} successfully killed", self.id);
                } else {
                    log::debug!(
                        "Expected KillPlanetResult, received {:?}, while killing planet {}",
                        message,
                        self.id
                    );
                }
            }
            Err(error) => {
                log::error!(
                    "Error while waiting for KillPlanetResult message from planet {}, error: {}",
                    self.id,
                    error
                )
            }
        }
    }
    fn send_sunray(&self) {
        match self
            .tx_planet
            .send(OrchestratorToPlanet::Sunray(Sunray::default()))
        {
            Ok(()) => {
                log::info!("Sunray message sent to planet {}", self.id);
            }
            Err(_) => log::error!("Could not send Sunray message to planet {}", self.id),
        }
        match self.rx_planet.recv() {
            Ok(message) => {
                if let PlanetToOrchestrator::SunrayAck { planet_id } = message {
                    log::info!("Sunray successully sent to planet {}", self.id);
                } else {
                    log::debug!(
                        "Expected SunrayAck, received {:?}, while sending sunray to planet {}",
                        message,
                        self.id
                    );
                }
            }
            Err(error) => {
                log::error!(
                    "Error while waiting for SunrayAck message from planet {}, error: {}",
                    self.id,
                    error
                )
            }
        }
    }
    fn send_asteroid(&self) {
        match self
            .tx_planet
            .send(OrchestratorToPlanet::Asteroid(Asteroid::default()))
        {
            Ok(()) => {
                log::info!("Asteroid message sent to planet {}", self.id);
            }
            Err(_) => log::error!("Could not send Asteroid message to planet {}", self.id),
        }
        match self.rx_planet.recv() {
            Ok(message) => {
                match message {
                    //Planet has rocket
                    AsteroidAck {
                        planet_id,
                        rocket: Some(rocket),
                    } => {
                        log::info!(
                            "Asteroid successfully sent to planet {}, who had a rocket and survived",
                            self.id
                        );
                    }
                    //Planet doesn't have rocket
                    AsteroidAck {
                        planet_id,
                        rocket: None,
                    } => {
                        log::info!(
                            "Asteroid successfully sent to planet {}, who didn't have a rocket",
                            self.id
                        );
                        self.kill_planet();
                    }
                    _ => {}
                }
            }
            Err(error) => {
                log::error!(
                    "Error while waiting for AsteroidAck message from planet {}, error: {}",
                    self.id,
                    error
                )
            }
        }
    }
    fn internal_state_request(&self) -> Option<DummyPlanetState> {
        match self
            .tx_planet
            .send(OrchestratorToPlanet::InternalStateRequest)
        {
            Ok(()) => log::info!("InternalStateRequest message sent to planet {}", self.id),
            Err(_) => {
                log::error!(
                    "Could not send InternalStateRequest message to planet {}",
                    self.id
                );
                return None;
            }
        }
        match self.rx_planet.recv() {
            Ok(message) => {
                if let PlanetToOrchestrator::InternalStateResponse {
                    planet_id,
                    planet_state,
                } = message
                {
                    log::info!(
                        "Successfully received InternalStateResponse from planet {}",
                        self.id
                    );
                    return Some(planet_state);
                } else {
                    log::debug!(
                        "Expected InternalStateResponse, received {:?}, during an InternalStateRequest to planet {}",
                        message,
                        self.id
                    );
                    return None;
                }
            }
            Err(error) => {
                log::error!(
                    "Error while waiting for InternalStateResponse message from planet {}, error: {}",
                    self.id,
                    error
                );
                return None;
            }
        }
    }
    fn join_thread(self) {
        match self.handle.join() {
            Ok(planet_result) => match planet_result {
                Ok(()) => log::info!("Planet {} joined successfully", self.id),
                Err(error) => log::error!("Error when joining planet {}: {}", self.id, error),
            },
            Err(_) => log::error!("Could not join thread of planet {}", self.id),
        }
    }
}
