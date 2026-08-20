use std::{
    format,
    marker::PhantomData,
    thread::{self, JoinHandle},
    todo,
    vec,
};

use crate::{galaxy_generator::Galaxy, orchestrator::{ExplorerID, ExplorerVendor}};
use explorer_common::logged_channel::LoggedChannel;

use common_game::{
    logging::EventType::MessageExplorerToOrchestrator, protocols::{
        orchestrator_explorer::{
            ExplorerToOrchestrator, ExplorerToOrchestratorKind, OrchestratorToExplorer,
        },
        planet_explorer,
    }, utils::ID,
};
use explorer_common::Bag;

pub type Channel = LoggedChannel<OrchestratorToExplorer, ExplorerToOrchestrator<Bag>>;
pub type UnbornExploererHandle = ExplorerHandle<Unborn>;
pub type PausedExploererHandle = ExplorerHandle<Born<Placed<Paused>>>;
pub type RunningExploererHandle = ExplorerHandle<Born<Placed<Running>>>;

pub struct ExplorerHandle<State> {
    pub id: ExplorerID,
    pub explorer_vendor: ExplorerVendor,
    pub state: State,
}
pub struct Unborn;
pub struct Born<Location> {
    pub channel: Channel,
    pub handle: JoinHandle<()>,
    pub location: Location,
}
pub struct Unplaced;
pub struct Placed<Substate> {
    pub planet_id: ID,
    pub run_state: Substate,
}
pub struct Paused;
pub struct Running;

impl<Any> ExplorerHandle<Born<Any>> {
    pub fn kill_explorer_ai(self) -> Result<JoinHandle<()>, ()> {
        if let Ok(_) = self.state.channel.send_and_check_ack(
            OrchestratorToExplorer::KillExplorer,
            ExplorerToOrchestratorKind::KillExplorerResult,
        ) {
            log::info!("Explorer {} killed", self.id);
            Ok(self.state.handle)
        } else {
            Err(())
        }
    }

    pub fn reset_explorer_ai(self) -> Result<ExplorerHandle<Born<Unplaced>>, ()> {
        if let Ok(_) = self.state.channel.send_and_check_ack(
            OrchestratorToExplorer::ResetExplorerAI,
            ExplorerToOrchestratorKind::ResetExplorerAIResult,
        ) {
            log::info!("Explorer {} killed", self.id);
            Ok(ExplorerHandle {
                id: self.id,
                explorer_vendor: self.explorer_vendor,
                state: Born {
                    channel: self.state.channel,
                    handle: self.state.handle,
                    location: Unplaced,
                },
            })
        } else {
            Err(())
        }
    }
}

impl ExplorerHandle<Unborn> {
    pub fn init_explorer_ai(
        self,
        planet_id: ID,
    ) -> Result<ExplorerHandle<Born<Placed<Running>>>, ()> {
        let (ex_sender, ex_reciever) = crossbeam_channel::unbounded();
        let (ox_sender, ox_reciever) = crossbeam_channel::unbounded();

        let handle = thread::spawn(move || {
            ex_reciever;
            ox_sender;
            //TODO this whole thing
        });
        let channel = Channel::new(ox_reciever, ex_sender, format!("Explorer {}", self.id));
        if let Ok(_) = channel.send_and_check_ack(
            OrchestratorToExplorer::StartExplorerAI,
            ExplorerToOrchestratorKind::StartExplorerAIResult,
        ) {
            Ok(ExplorerHandle {
                id: self.id,
                explorer_vendor: self.explorer_vendor,
                state: Born {
                    channel,
                    handle: handle,
                    location: Placed {
                        planet_id,
                        run_state: Running,
                    },
                },
            })
        } else {
            Err(())
        }
    }
}

impl<Any> ExplorerHandle<Born<Placed<Any>>> {
    fn move_to_planet_intnl(
        &mut self,
        sender: Option<crossbeam_channel::Sender<planet_explorer::ExplorerToPlanet>>,
        planet_id: ID,
    ) -> Result<(), ()> {
        let result = self
            .state
            .channel
            .send(OrchestratorToExplorer::MoveToPlanet {
                sender_to_new_planet: sender,
                planet_id,
            });
        if result.is_err() {
            return Err(());
        }

        match self.state.channel.recv() {
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
                        return Err(());
                    }
                    if planet_id != planet_id_resp {
                        log::error!(
                            "Explorer {:?} returned incohernet planet ID {:?} when sending {:?}",
                            self.id as ID,
                            planet_id,
                            planet_id_resp
                        );
                        return Err(());
                    }

                    log::info!(
                        "Explorer {:?} moved to planet {:?}",
                        self.id as ID,
                        planet_id
                    );
                    self.state.location.planet_id = planet_id;
                    Ok(())
                }
                _ => {
                    log::error!(
                        "Invalid response from Explorer {:?}. Expected {:?}, got {:?}",
                        self.id as ID,
                        ExplorerToOrchestratorKind::MovedToPlanetResult,
                        val
                    );
                    Err(())
                }
            },
            Err(_) => Err(()),
        }
    }
}

impl ExplorerHandle<Born<Placed<Paused>>> {
    pub fn unpause_explorer_ai(self) -> Result<ExplorerHandle<Born<Placed<Running>>>, ()> {
        if let Ok(_) = self.state.channel.send_and_check_ack(
            OrchestratorToExplorer::StartExplorerAI,
            ExplorerToOrchestratorKind::StartExplorerAIResult,
        ) {
            Ok(ExplorerHandle {
                id: self.id,
                explorer_vendor: self.explorer_vendor,
                state: Born {
                    channel: self.state.channel,
                    handle: self.state.handle,
                    location: Placed {
                        planet_id: self.state.location.planet_id,
                        run_state: Running,
                    },
                },
            })
        } else {
            Err(())
        }
    }

    pub fn move_to_planet(
        &mut self,
        sender: Option<crossbeam_channel::Sender<planet_explorer::ExplorerToPlanet>>,
        planet_id: ID,
    ) -> Result<(), ()> {
        self.move_to_planet_intnl(sender, planet_id)
    }

    //TODO all other manual mode funcs
}

impl ExplorerHandle<Born<Placed<Running>>> {
    pub fn pause_explorer_ai(self) -> Result<ExplorerHandle<Born<Placed<Paused>>>, ()> {
        if let Ok(_) = self.state.channel.send_and_check_ack(
            OrchestratorToExplorer::StopExplorerAI,
            ExplorerToOrchestratorKind::StopExplorerAIResult,
        ) {
            Ok(ExplorerHandle {
                id: self.id,
                explorer_vendor: self.explorer_vendor,
                state: Born {
                    channel: self.state.channel,
                    handle: self.state.handle,
                    location: Placed {
                        planet_id: self.state.location.planet_id,
                        run_state: Paused,
                    },
                },
            })
        } else {
            Err(())
        }
    }

    pub fn poll(&self) -> Result<std::option::Option<ExplorerToOrchestrator<Bag>>, ()> {
        self.state.channel.poll()
    }

    pub fn handle_neighbhors_request(&self,
        galaxy: &Galaxy,
        current_planet_id: ID,
    ) -> Result<(), ()> {
        let neighbors = if self.state.location.planet_id != current_planet_id {
            log::error!(
                "Explorer {:?} is requesting for neighbhors of a planet it is not on",
                self.id as ID
            );
            vec![]
        } else {
            match galaxy.planets.get(&current_planet_id) {
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
                        self.id as ID
                    );
                    vec![]
                }
            }
        };
        if self
            .state.channel
            .send(OrchestratorToExplorer::NeighborsResponse { neighbors })
            .is_err()
        {
            Err(())
        } else {
            Ok(())
        }
    }

    pub fn handle_travel_to_planet_request(&mut self,
        galaxy: &Galaxy,
        current_planet_id: ID,
        dst_planet_id: ID,
    ) -> Result<(), ()> {
        //Check if the explorer can reach the planet
        let travel_to_planet = if self.state.location.planet_id != current_planet_id {
            log::error!(
                "Explorer {:?} tried to move out of planet it is not on",
                self.id as ID
            );
            None
        } else {
                galaxy
                .planets
                .get(&current_planet_id)
                .map(|current_planet| {
                    let current_planet = current_planet
                        .lock()
                        .expect("Planet thread must not be poisoned");
                    current_planet.adj.iter().find_map(|neighbor| {
                        let neighbor =
                            neighbor.lock().expect("Planet thread must not be poisoned");
                        (neighbor.id() == dst_planet_id).then(|| neighbor.tx_explorer.clone())
                    })
                })
                .flatten()
        };

        match travel_to_planet {
            //Success
            Some(tx_explorer) => self.move_to_planet_intnl(Some(tx_explorer), dst_planet_id),
            //Failure (Don't move)
            None => self.move_to_planet_intnl(None, self.state.location.planet_id),
        }
    }

}

pub fn new(id: ExplorerID, explorer_vendor: ExplorerVendor) -> ExplorerHandle<Unborn> {
    ExplorerHandle::<Unborn> {
        id,
        explorer_vendor,
        state: Unborn,
    }
}
