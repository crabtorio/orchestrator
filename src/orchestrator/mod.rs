use crate::galaxy_generator::{Galaxy, PlanetContainer};
use crate::orchestrator::Command::*;
use crate::orchestrator::ai::AiType::RichardRandom as RichardRandomType;
use crate::orchestrator::ai::{Ai, AiType, RichardRandom};
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
mod shell;
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
    fn new(ai: Box<dyn Ai>, queue: Arc<Mutex<VecDeque<Command>>>, first: ID, last: ID) -> Self {
        static NEXT_ID: AtomicU32 = AtomicU32::new(0);
        let run_flag = Arc::new(AtomicBool::new(true));
        let closure_flag = run_flag.clone();
        Self {
            id: NEXT_ID.load(Relaxed),
            handle: { thread::spawn(move || ai.run(queue, closure_flag, first, last)) },
            run_flag: run_flag,
        }
    }
}
pub struct PlanetHandle {
    id: ID,
    handle: JoinHandle<()>,
    tx_planet: crossbeam_channel::Sender<orchestrator_planet::OrchestratorToPlanet>,
    rx_planet: crossbeam_channel::Receiver<orchestrator_planet::PlanetToOrchestrator>,
    tx_explorer: crossbeam_channel::Sender<planet_explorer::ExplorerToPlanet>,
}

#[derive(Clone, Copy)]
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

pub struct ExplorerHandle {
    id: ExplorerID,
    channel: LoggedChannel<OrchestratorToExplorer, ExplorerToOrchestrator<Bag>>,
    current_planet: ID,
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
        let mut explorer_handles = match &self.explorers {
            Explorers::One(explorer_vendor) => vec![
                ExplorerHandle::spawn(ExplorerID::First,*explorer_vendor),
            ],
            Explorers::Two(explorer_vendor, explorer_vendor1) => vec![
                ExplorerHandle::spawn(ExplorerID::First,*explorer_vendor),
                ExplorerHandle::spawn(ExplorerID::Second,*explorer_vendor1)
            ],
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
                        0,
                        (self.galaxy.planets.len() - 1) as ID,
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

            self.handle_request(&explorer_handles);
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

    ///Poll clients and handle first found request.
    fn handle_request(&mut self, explorers : &Vec<ExplorerHandle>) {
        for explorer in explorers {
            match explorer.channel.poll() {
                Err(()) => {
                    todo!("Kill explorer");
                    return;
                }
                Ok(None) => continue,
                //Handle the request
                Ok(Some(request)) => match request {
                    ExplorerToOrchestrator::NeighborsRequest {
                        explorer_id: _,
                        current_planet_id,
                    } => {
                        let neighbors = if explorer.current_planet != current_planet_id {
                            log::error!(
                                "Explorer {:?} is requesting for neighbhors of a planet it is not on",
                                explorer.channel.reciever_ident
                            );
                            vec![]
                        } else {
                            match self.galaxy.planets.get(&current_planet_id) {
                                Some(planet) => planet
                                    .lock()
                                    .expect("Not poisoned")
                                    .adj
                                    .iter()
                                    .map(|planet| planet.lock().expect("Not poisoned").id())
                                    .collect(),
                                None => {
                                    log::error!(
                                        "Explorer {:?} is somehow on invalid/dead planet",
                                        explorer.channel.reciever_ident
                                    );
                                    vec![]
                                }
                            }
                        };
                        let result = explorer
                            .channel
                            .send(OrchestratorToExplorer::NeighborsResponse { neighbors });
                        if result.is_err() {
                            todo!("Kill explorer");
                            return;
                        }
                    }
                    ExplorerToOrchestrator::TravelToPlanetRequest {
                        explorer_id: _,
                        current_planet_id,
                        dst_planet_id,
                    } => {
                        //Check if the explorer can reach the planet
                        let travel_to_planet = if explorer.current_planet != current_planet_id {
                            log::error!(
                                "Explorer {:?} tried to move out of planet it is not on",
                                explorer.channel.reciever_ident
                            );
                            None
                        } else {
                            self.galaxy
                                .planets
                                .get(&current_planet_id)
                                .map(|current_planet| {
                                    let current_planet = current_planet
                                        .lock()
                                        .expect("Planet thread must not be poisoned");
                                    current_planet.adj.iter().find_map(|neighbor| {
                                        let neighbor = neighbor
                                            .lock()
                                            .expect("Planet thread must not be poisoned");
                                        if neighbor.id() == dst_planet_id {
                                            //Planet found among neighbors, clone the sender so it can be provided to the Explorer
                                            Some(neighbor.tx_explorer.clone())
                                        } else {
                                            None
                                        }
                                    })
                                })
                                .flatten()
                        };

                        match travel_to_planet {
                            //Success
                            Some(tx_explorer) => {
                                explorer.move_to_planet(Some(tx_explorer), dst_planet_id)
                            }
                            //Failure (Don't move)
                            None => explorer.move_to_planet(None, explorer.current_planet),
                        }
                    }
                    _ => {
                        log::error!(
                            "Explorer sent response {:?} while orchestrator awiating for requests",
                            request
                        );
                        todo!("Kill explorer");
                        return;
                    }
                },
            };
        }
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
            Ok(()) => (),
            Err(_) => log::error!("Could not join thread of planet {}", self.id),
        }
    }
}

impl ExplorerHandle {

    fn spawn(id: ExplorerID, vendor: ExplorerVendor) -> Self {
        todo!()
    }

    fn start_explorer_ai(&self) {
        if let Ok(_) = self.channel.send_and_check_ack(
            OrchestratorToExplorer::StartExplorerAI,
            ExplorerToOrchestratorKind::StopExplorerAIResult,
        ) {
            log::info!("Explorer {} started", self.id)
        }
    }

    fn reset_explorer_ai(&self) {
        if let Ok(_) = self.channel.send_and_check_ack(
            OrchestratorToExplorer::ResetExplorerAI,
            ExplorerToOrchestratorKind::ResetExplorerAIResult,
        ) {
            log::info!("Explorer {} reset", self.id)
        }
    }

    fn stop_explorer_ai(&self) {
        if let Ok(_) = self.channel.send_and_check_ack(
            OrchestratorToExplorer::StopExplorerAI,
            ExplorerToOrchestratorKind::StopExplorerAIResult,
        ) {
            log::info!("Explorer {} stopped", self.id)
        }
    }

    fn kill_explorer(&self) {
        if let Ok(_) = self.channel.send_and_check_ack(
            OrchestratorToExplorer::KillExplorer,
            ExplorerToOrchestratorKind::KillExplorerResult,
        ) {
            log::info!("Explorer {} killed", self.id)
        }
    }

    fn move_to_planet(
        &self,
        sender: Option<crossbeam_channel::Sender<planet_explorer::ExplorerToPlanet>>,
        planet_id: ID,
    ) {
        let result = self.channel.send(OrchestratorToExplorer::MoveToPlanet {
            sender_to_new_planet: sender,
            planet_id,
        });
        if result.is_err() {
            return;
        }

        match self.channel.recv() {
            Ok(val) => match val {
                ExplorerToOrchestrator::MovedToPlanetResult {
                    explorer_id,
                    planet_id: planet_id_resp,
                } => {
                    if explorer_id != self.id as ID {
                        log::error!(
                            "Explorer {:?} returned incohernet ID {:?} when sending {:?}",
                            self.id as ID,
                            explorer_id,
                            planet_id_resp
                        );
                        return;
                    }
                    if planet_id != planet_id_resp {
                        log::error!(
                            "Explorer {:?} returned incohernet planet ID {:?} when sending {:?}",
                            self.id as ID,
                            planet_id,
                            planet_id_resp
                        );
                        return;
                    }

                    log::info!(
                        "Explorer {:?} moved to planet {:?}",
                        self.id as ID,
                        planet_id
                    );
                }
                _ => log::error!(
                    "Invalid response from {:?}. Expected {:?}, got {:?}",
                    self.channel.reciever_ident,
                    ExplorerToOrchestratorKind::MovedToPlanetResult,
                    val
                ),
            },
            Err(_) => {}
        }
    }
}

struct LoggedChannel<SendT, RecvT> {
    reciever: crossbeam_channel::Receiver<RecvT>,
    sender: crossbeam_channel::Sender<SendT>,
    reciever_ident: String,
}
enum ChannelError<T> {
    SendError(crossbeam_channel::SendError<T>),
    RecvError(crossbeam_channel::RecvError),
    InvalidResponseError,
}

impl<SendT: std::fmt::Debug, RecvT: std::fmt::Debug> LoggedChannel<SendT, RecvT> {
    fn send(&self, val: SendT) -> Result<(), crossbeam_channel::SendError<SendT>> {
        let val_debug = std::format!("{:?}", val);
        let result = self.sender.send(val);

        if result.is_ok() {
            log::debug!("{} sent to {}", val_debug, self.reciever_ident)
        } else {
            log::error!("Could not send {} to {}", val_debug, self.reciever_ident)
        }

        return result;
    }

    fn recv(&self) -> Result<RecvT, crossbeam_channel::RecvError> {
        let result = self.reciever.recv();
        match result {
            Ok(val) => {
                log::debug!("Recieved {:?} from {}", val, self.reciever_ident);
                Ok(val)
            }
            Err(err) => {
                log::error!(
                    "{} error while waiting on response from {}",
                    err,
                    self.reciever_ident
                );
                Err(err)
            }
        }
    }

    fn send_and_check_ack<T: PartialEq + std::fmt::Debug>(
        &self,
        val: SendT,
        ack_to_ckeck: T,
    ) -> Result<(), ChannelError<SendT>>
    where
        RecvT: Into<T>,
    {
        let send_result = self.send(val).map(|val| {});
        match send_result {
            Ok(()) => match self.recv() {
                Ok(res) => {
                    let res_debug = std::format!("{:?}", res);
                    if ack_to_ckeck != res.into() {
                        log::error!(
                            "Invalid response from {:?}. Expected {:?}, got {}",
                            self.reciever_ident,
                            ack_to_ckeck,
                            res_debug
                        )
                    }

                    Ok(())
                }
                Err(err) => Err(ChannelError::RecvError(err)),
            },
            Err(err) => Err(ChannelError::SendError(err)),
        }
    }

    fn poll(&self) -> Result<Option<RecvT>, ()> {
        log::trace!("Polling {:?}...", self.reciever_ident);
        match self.reciever.try_recv() {
            Ok(val) => {
                log::debug!("Recieved {:?} from {}", val, self.reciever_ident);
                Ok(Some(val))
            }
            Err(err) => match err {
                crossbeam_channel::TryRecvError::Empty => {
                    log::trace!("Polled {:?}, got no response", self.reciever_ident);
                    Ok(None)
                }
                crossbeam_channel::TryRecvError::Disconnected => {
                    log::error!(
                        "{:?} disconnected unexpectedly while being polled",
                        self.reciever_ident
                    );
                    Err(())
                }
            },
        }
    }
}
