mod explorer_handle;
mod shell;

use crate::galaxy_generator::{Galaxy, PlanetContainer};
use crate::orchestrator::Command::*;
use crate::orchestrator::ai::AiType::RichardRandom as RichardRandomType;
use crate::orchestrator::ai::{Ai, AiArgs, AiType, RichardRandom};
use crate::orchestrator::explorer_handle::{
    ExplorerSet, GenericExplorer, MoveResult, MoveResultError, PausedExploererHandle, PlacedResult,
    RunningExploererHandle, UnbornExplorerHandle,
};
use crate::orchestrator::shell::Shell;
use common_game::components::asteroid::Asteroid;
use common_game::components::planet::DummyPlanetState;
use common_game::components::resource::{BasicResourceType, ComplexResourceType};
use common_game::components::sunray::Sunray;
use common_game::protocols::orchestrator_planet::PlanetToOrchestrator::AsteroidAck;
use common_game::{
    protocols::{orchestrator_planet, planet_explorer},
    utils::ID,
};
use orchestrator_planet::{OrchestratorToPlanet, PlanetToOrchestrator};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::sync::atomic::Ordering::{Relaxed, Release};
use std::sync::atomic::{AtomicBool, AtomicU32};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    thread::{self, JoinHandle},
};
use std::{println, todo};
pub mod ai;

pub struct Orchestrator {
    galaxy: Galaxy,
    ai_queue: Arc<Mutex<VecDeque<Command>>>,
    user_queue: Arc<Mutex<VecDeque<Command>>>,
    dead_ids: Arc<Mutex<Vec<ID>>>,
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
        let id = NEXT_ID.load(Relaxed);
        NEXT_ID.fetch_add(1, Relaxed);
        Self {
            id: id,
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
    ResumeExplorers,
    StopExplorers,
    KillExplorers,
    SpawnExplorer {
        explorer_id: ExplorerID,
        vendor: ExplorerVendor,
        destination_planet: ID,
    },
    StartExplorer(ExplorerID),
    StopExplorer(ExplorerID),
    KillExplorer(ExplorerID),
    ResetExplorer(ExplorerID),
    MoveExplorer {
        planet_id: ID,
        explorer: ExplorerID,
    },
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

    Nothing, // To be returned by the parserer when the command doesn't involve any orchestrator action
}

impl Orchestrator {
    pub fn new(
        galaxy: Galaxy,
        ai_queue: Arc<Mutex<VecDeque<Command>>>,
        user_queue: Arc<Mutex<VecDeque<Command>>>,
    ) -> Self {
        Orchestrator {
            galaxy,
            ai_queue,
            user_queue,
            dead_ids: Arc::new(Mutex::new(Vec::new())),
        }
    }
    pub fn run(&mut self) {
        let mut planet_handles: HashMap<ID, PlanetHandle> = self
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
        //Add the unborn explorer handles to the appropriate map
        let mut explorer_handles = ExplorerSet(HashMap::from([
            (
                ExplorerID::First,
                GenericExplorer::Unborn(explorer_handle::new(ExplorerID::First)),
            ),
            (
                ExplorerID::Second,
                GenericExplorer::Unborn(explorer_handle::new(ExplorerID::Second)),
            ),
        ]));
        loop {
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
                    explorer_handles.bulk_running_op(|_key, mut explorer| {
                        match explorer.handle_one_request(&self.galaxy, &planet_handles) {
                            Ok(_) => Some(GenericExplorer::Running(explorer)),
                            Err(()) => {
                                //An error occured while handling an explorer's request.
                                //In this case we chose to kill the explorer and continue execution with a single explorer
                                log::info!("Attempting to kill explorer due to previous error");
                                match explorer.kill() {
                                    Ok(join_handle) => {
                                        join_handle.join();
                                    }
                                    Err(_) => log::error!(
                                        "Could not shut down explorer cleanly, continuing anyway"
                                    ),
                                }
                                None
                            }
                        }
                    });
                    continue;
                }
            };

            //Helper functions
            fn kill_generic_explorer(
                id: ExplorerID,
                explorer: GenericExplorer,
            ) -> UnbornExplorerHandle {
                let join_handle = match explorer {
                    GenericExplorer::Unborn(_explorer_handle) => return explorer_handle::new(id),
                    //The otheres must go through the kill procedure
                    GenericExplorer::Running(explorer_handle) => explorer_handle.kill(),
                    GenericExplorer::Stopped(explorer_handle) => explorer_handle.kill(),
                };
                //Join the threads
                match join_handle {
                    Ok(handle) => {
                        handle.join();
                    }
                    Err(_) => println!("An error occured while killing explorer {id}"),
                };
                explorer_handle::new(id)
            }

            fn reset_generic_explorer<'a>(
                id: ExplorerID,
                explorer: GenericExplorer,
            ) -> Option<GenericExplorer> {
                let reset_result = match explorer {
                    //Unborn explorers cannot be reset.
                    GenericExplorer::Unborn(explorer_handle) => {
                        return Some(GenericExplorer::Unborn(explorer_handle));
                    }
                    //All the others can
                    GenericExplorer::Running(explorer_handle) => explorer_handle.reset(),
                    GenericExplorer::Stopped(explorer_handle) => explorer_handle.reset(),
                };
                match reset_result {
                    Ok(new_handle) => Some(GenericExplorer::Stopped(new_handle)),
                    Err(_) => {
                        println!("An error occured while resetting explorer {id}");
                        None
                    }
                }
            }

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
                KillPlanet(id) => {
                    //Kill all explorers in the planet
                    explorer_handles.bulk_op(|explorer_id, explorer| {
                        let explorer_planet_id = match &explorer {
                            GenericExplorer::Unborn(_) => return Some(explorer),
                            GenericExplorer::Running(explorer_handle) => explorer_handle.get_current_planet(),
                            GenericExplorer::Stopped(explorer_handle) => explorer_handle.get_current_planet(),
                        };
                        if explorer_planet_id == id {
                            log::info!("Found explorer {} on planet {} that is being killed, killing explorer",explorer_id,id);
                            Some(GenericExplorer::Unborn(kill_generic_explorer(
                                explorer_id,
                                explorer,
                            )))
                        } else {
                            Some(explorer)
                        }
                    });
                    self.dead_ids.lock().unwrap().push(id);
                    planet_handles[&id].kill_planet();
                    self.galaxy.drop_planet(id); //Planet dropped from the galaxy
                    let _res = planet_handles.remove(&id);
                }
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
                            (self.galaxy.planets.len() - 1) as ID,
                            self.dead_ids.clone(),
                        ),
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
                ResumeExplorers => {
                    explorer_handles.bulk_paused_op(|key, explorer| match explorer.start() {
                        Ok(explorer) => Some(GenericExplorer::Running(explorer)),
                        Err(_) => {
                            println!("An error occured while unpausing explorer {key}");
                            None
                        }
                    });
                }
                StopExplorers => {
                    explorer_handles.bulk_running_op(|key, explorer| match explorer.stop() {
                        Ok(explorer) => Some(GenericExplorer::Stopped(explorer)),
                        Err(_) => {
                            println!("An error occured while pausing explorer {key}");
                            None
                        }
                    })
                }
                KillExplorers => explorer_handles.bulk_op(|id, explorer| {
                    Some(GenericExplorer::Unborn(kill_generic_explorer(id, explorer)))
                }),
                SpawnExplorer {
                    explorer_id,
                    vendor,
                    destination_planet,
                } => explorer_handles.take_explorer(explorer_id, |maybe_explorer| {
                    let maybe_planet = planet_handles.get(&destination_planet);

                    fn spawn_explorer(
                        planet_handle: &PlanetHandle,
                        vendor: ExplorerVendor,
                        explorer_handle: UnbornExplorerHandle,
                    ) -> GenericExplorer {
                        use explorer_handle::PlacedResult::*;
                        let place_result = match vendor {
                            ExplorerVendor::Lorenzo => explorer_handle
                                .spawn_in_place::<ml_explorer::Explorer>(planet_handle),
                            ExplorerVendor::Alessio => todo!(),
                            ExplorerVendor::Luca => explorer_handle
                                .spawn_in_place::<fl_explorer::Explorer>(planet_handle),
                        };

                        match place_result {
                            Placed(explorer_handle) => GenericExplorer::Stopped(explorer_handle),
                            DestinationPlanetRefused { handle, reason } => {
                                println!("Could not place explorer, planet refused: {reason}");
                                GenericExplorer::Unborn(handle)
                            }
                            DestinationPlanetFailed(handle) => {
                                println!("Could not place explorer, planet failed");
                                GenericExplorer::Unborn(handle)
                            }
                        }
                    }

                    match (maybe_planet, maybe_explorer) {
                        (Some(planet_handle), Some(GenericExplorer::Unborn(explorer_handle))) => {
                            Some(spawn_explorer(planet_handle, vendor, explorer_handle))
                        }
                        (Some(planet_handle), None) => {
                            Some(spawn_explorer(planet_handle, vendor, explorer_handle::new(explorer_id)))
                        }
                        (Some(_), Some(explorer)) => {
                            println!("This explorer is already running");
                            Some(explorer)
                        }
                        (None, None) => {
                            println!("Explorer not found");
                            println!("Planet not found");
                            None
                        }
                        (None, Some(explorer)) => {
                            println!("Planet not found");
                            Some(explorer)
                        }
                    }
                }),
                StartExplorer(explorer_id) => {
                    explorer_handles.take_explorer(explorer_id, |maybe_explorer| {
                        match maybe_explorer {
                            Some(GenericExplorer::Stopped(explorer_handle)) => {
                                match explorer_handle.start() {
                                    Ok(explorer_handle) => {
                                        Some(GenericExplorer::Running(explorer_handle))
                                    }
                                    Err(_) => {
                                        println!("Internal error while resuming explorer");
                                        None
                                    }
                                }
                            }
                            Some(GenericExplorer::Running(explorer)) => {
                                println!("The explorer is already running");
                                Some(GenericExplorer::Running(explorer))
                            }
                            Some(explorer) => {
                                println!("The explorer can not be resumed");
                                Some(explorer)
                            }
                            None => {
                                println!("Explorer not found");
                                None
                            }
                        }
                    })
                }
                StopExplorer(explorer_id) => {
                    explorer_handles.take_explorer(explorer_id, |maybe_explorer| {
                        match maybe_explorer {
                            Some(GenericExplorer::Running(explorer_handle)) => {
                                match explorer_handle.stop() {
                                    Ok(explorer_handle) => {
                                        Some(GenericExplorer::Stopped(explorer_handle))
                                    }
                                    Err(_) => {
                                        println!("Internal error while stopping explorer");
                                        None
                                    }
                                }
                            }
                            Some(GenericExplorer::Stopped(explorer)) => {
                                println!("This explorer is already stopped");
                                Some(GenericExplorer::Stopped(explorer))
                            }
                            Some(explorer) => {
                                println!("This explorer has already been initialized");
                                Some(explorer)
                            }
                            None => {
                                println!("Explorer not found");
                                None
                            }
                        }
                    })
                }
                KillExplorer(explorer_id) => {
                    explorer_handles.take_explorer(explorer_id, |maybe_explorer| {
                        match maybe_explorer {
                            Some(explorer) => Some(GenericExplorer::Unborn(kill_generic_explorer(
                                explorer_id,
                                explorer,
                            ))),
                            None => {
                                println!("Explorer not found");
                                None
                            }
                        }
                    })
                }
                ResetExplorer(explorer_id) => {
                    explorer_handles.take_explorer(explorer_id, |maybe_explorer| {
                        match maybe_explorer {
                            Some(explorer) => reset_generic_explorer(explorer_id, explorer),
                            None => {
                                println!("Explorer not found");
                                None
                            }
                        }
                    })
                }
                MoveExplorer {
                    planet_id,
                    explorer,
                } => {
                    let explorer = match explorer_handles.get_mut(explorer) {
                        Some(GenericExplorer::Stopped(explorer_handle)) => explorer_handle,
                        Some(_) => {
                            println!("Can only manually move stopped explorers");
                            continue;
                        }
                        None => {
                            println!("Explorer not found");
                            continue;
                        }
                    };

                    if !planet_handles.contains_key(&planet_id) {
                        println!("Planet not found");
                        continue;
                    }

                    let res = explorer.move_to_planet(planet_id, &planet_handles);
                    match res {
                        Err(err) => match err {
                            MoveResultError::Planet(_) => {
                                println!("One or more planets encountered an error during the move")
                            }
                            MoveResultError::ExplorerFailed => {
                                println!("Explorer encountered an error while moving")
                            }
                        },
                        Ok(res) => match res {
                            MoveResult::Moved => (),
                            MoveResult::SourcePlanetRefused => {
                                println!("Source planet refused to allow move")
                            }
                            MoveResult::DestPlanetRefused => {
                                println!("Destination planet refused to allow move")
                            }
                        },
                    }
                }
                CurrentPlanetRequest(explorer_id) => {
                    let current_planet = match explorer_handles.get(explorer_id) {
                        Some(GenericExplorer::Running(explorer)) => explorer.get_current_planet(),
                        Some(GenericExplorer::Stopped(explorer)) => explorer.get_current_planet(),
                        Some(_) => {
                            println!("Explorer is not on any planet");
                            return;
                        }
                        None => {
                            println!("Not found");
                            return;
                        }
                    };

                    println!("{current_planet}")
                }
                SupportedResourceRequest(explorer_id) => {
                    let supported_resources = match explorer_handles.get(explorer_id) {
                        Some(GenericExplorer::Stopped(explorer)) => {
                            explorer.get_supported_resources()
                        }
                        Some(_) => {
                            println!("Explorer is not stopped");
                            return;
                        }
                        None => {
                            println!("Not found");
                            return;
                        }
                    };

                    match supported_resources {
                        Ok(resources) => println!("{resources:#?}"),
                        Err(_) => {
                            println!("There was an error while getting the supported resources")
                        }
                    }
                }
                SupportedCombinationRequest(explorer_id) => {
                    let supported_combinations = match explorer_handles.get(explorer_id) {
                        Some(GenericExplorer::Stopped(explorer)) => {
                            explorer.get_supported_combinations()
                        }
                        Some(_) => {
                            println!("Explorer is not stopped");
                            return;
                        }
                        None => {
                            println!("Not found");
                            return;
                        }
                    };

                    match supported_combinations {
                        Ok(combinations) => println!("{combinations:#?}"),
                        Err(_) => {
                            println!("There was an error while getting the supported combiniations")
                        }
                    }
                }
                BagContentRquest(explorer_id) => {
                    let bag_content = match explorer_handles.get(explorer_id) {
                        Some(GenericExplorer::Running(explorer)) => explorer.get_bag_content(),
                        Some(GenericExplorer::Stopped(explorer)) => explorer.get_bag_content(),
                        Some(_) => {
                            println!("Explorer has not been initialized");
                            return;
                        }
                        None => {
                            println!("Not found");
                            return;
                        }
                    };

                    match bag_content {
                        Ok(bag_content) => println!("{bag_content:#?}"),
                        Err(_) => {
                            println!("There was an error while getting the supported combiniations")
                        }
                    }
                }
                InternalStateRequest(_) => (),
                GenerateResourceRequest(explorer_id, basic_resource_type) => {
                    let result = match explorer_handles.get(explorer_id) {
                        Some(GenericExplorer::Stopped(explorer)) => {
                            explorer.try_generate_resource(basic_resource_type)
                        }
                        Some(_) => {
                            println!("Explorer is not in manual mode");
                            return;
                        }
                        None => {
                            println!("Not found");
                            return;
                        }
                    };

                    match result {
                        Ok(Ok(())) => (),
                        Ok(Err(reason)) => println!("Could not generate the reosurce: `{reason}`"),
                        Err(()) => println!("There was an error while generating the resource"),
                    }
                }
                CombineResourceRequest(explorer_id, complex_resource_type) => {
                    let result = match explorer_handles.get(explorer_id) {
                        Some(GenericExplorer::Stopped(explorer)) => {
                            explorer.try_combine_resources(complex_resource_type)
                        }
                        Some(_) => {
                            println!("Explorer is not in manual mode");
                            return;
                        }
                        None => {
                            println!("Not found");
                            return;
                        }
                    };

                    match result {
                        Ok(Ok(())) => (),
                        Ok(Err(reason)) => {
                            println!("Could not complete the combination: `{reason}`")
                        }
                        Err(()) => println!("There was an error while combining the resources"),
                    }
                }
                Nothing => (),
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
        explorers: &HashMap<ExplorerID, RefCell<RunningExploererHandle>>,
        planet_handles: &HashMap<ID, PlanetHandle>,
    ) -> Result<(), ExplorerID> {
        for (explorer_index, explorer) in explorers.iter() {
            let result = explorer
                .borrow_mut()
                .handle_one_request(&self.galaxy, planet_handles);
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
        let mut planet = planet.lock().unwrap().extract_planet().unwrap();
        PlanetHandle {
            id,
            handle: thread::spawn(move || planet.run()),
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

    fn incoming_explorer_request(
        &self,
        explorer_id: ExplorerID,
        new_sender: crossbeam_channel::Sender<planet_explorer::PlanetToExplorer>,
    ) -> Result<Result<(), String>, ()> {
        let send_result = self
            .tx_planet
            .send(OrchestratorToPlanet::IncomingExplorerRequest {
                explorer_id: explorer_id as ID,
                new_sender,
            });
        match send_result {
            Ok(()) => log::info!(
                "IncomingExplorerRequest message sent to planet {} for explorer {}",
                self.id,
                explorer_id
            ),
            Err(_) => {
                log::error!(
                    "Could not send IncomingExplorerRequest message to planet {}",
                    self.id
                );
                return Err(());
            }
        };

        match self.rx_planet.recv() {
            Ok(message) => {
                if let PlanetToOrchestrator::IncomingExplorerResponse {
                    planet_id: _,
                    explorer_id: _,
                    res,
                } = message
                {
                    match res {
                        Ok(()) => {
                            log::info!(
                                "Successfully received IncomingExplorerResponse from planet {}",
                                self.id
                            );
                            Ok(Ok(()))
                        }
                        Err(errmsg) => {
                            log::info!(
                                "Planet {} rejected explorer {} with message {}",
                                self.id,
                                explorer_id,
                                errmsg
                            );
                            Ok(Err(errmsg))
                        }
                    }
                } else {
                    log::error!(
                        "Expected IncomingExplorerResponse, received {:?}, while sending IncomingExplorerRequest to planet {}",
                        message,
                        self.id
                    );
                    Err(())
                }
            }
            Err(error) => {
                log::error!(
                    "Error while waiting for IncomingExplorerResponse message from planet {}, error: {}",
                    self.id,
                    error
                );
                Err(())
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

    fn outgoing_explorer_request(&self, explorer_id: ExplorerID) -> Result<Result<(), String>, ()> {
        let send_result = self
            .tx_planet
            .send(OrchestratorToPlanet::OutgoingExplorerRequest {
                explorer_id: explorer_id as ID,
            });
        match send_result {
            Ok(()) => log::info!(
                "OutgoingExplorerRequest message sent to planet {} for explorer {}",
                self.id,
                explorer_id
            ),
            Err(_) => {
                log::error!(
                    "Could not send OutgoingExplorerRequest message to planet {}",
                    self.id
                );
                return Err(());
            }
        };

        match self.rx_planet.recv() {
            Ok(message) => {
                if let PlanetToOrchestrator::OutgoingExplorerResponse {
                    planet_id: _,
                    explorer_id: _,
                    res,
                } = message
                {
                    match res {
                        Ok(()) => {
                            log::info!(
                                "Successfully received OutgoingExplorerResponse from planet {}",
                                self.id
                            );
                            Ok(Ok(()))
                        }
                        Err(errmsg) => {
                            log::info!(
                                "Planet {} did not allow explorer to move out {} with message {}",
                                self.id,
                                explorer_id,
                                errmsg
                            );
                            Ok(Err(errmsg))
                        }
                    }
                } else {
                    log::error!(
                        "Expected OutgoingExplorerResponse, received {:?}, while sending IncomingExplorerRequest to planet {}",
                        message,
                        self.id
                    );
                    Err(())
                }
            }
            Err(error) => {
                log::error!(
                    "Error while waiting for OutgoingExplorerResponse message from planet {}, error: {}",
                    self.id,
                    error
                );
                Err(())
            }
        }
    }
}
