use std::{
    fmt::format,
    format,
    marker::PhantomData,
    thread::{self, JoinHandle},
    vec,
};

use crate::{
    galaxy_generator::Galaxy,
    orchestrator::{ExplorerID, ExplorerVendor},
};
use explorer_common::logged_channel::LoggedChannel;

use common_game::{
    logging::{ActorType, Channel as LogChannel, EventType, LogEvent, Participant, Payload},
    protocols::{
        orchestrator_explorer::{
            ExplorerToOrchestrator, ExplorerToOrchestratorKind, OrchestratorToExplorer,
        },
        planet_explorer,
    },
    utils::ID,
};
use explorer_common::Bag;

pub type Channel = LoggedChannel<OrchestratorToExplorer, ExplorerToOrchestrator<Bag>>;
pub type UnbornExploererHandle = ExplorerHandle<Unborn>;
pub type PausedExploererHandle = ExplorerHandle<Born<Placed<Paused>>>;
pub type RunningExploererHandle = ExplorerHandle<Born<Placed<Running>>>;

pub struct ExplorerHandle<State> {
    id: ExplorerID,
    explorer_vendor: ExplorerVendor,
    state: State,
}
pub struct Unborn;
pub struct Born<Location> {
    channel: Channel,
    handle: JoinHandle<()>,
    location: Location,
}
pub struct Unplaced;
pub struct Placed<Substate> {
    planet_id: ID,
    run_state: Substate,
}
pub struct Paused;
pub struct Running;

impl<Any> ExplorerHandle<Any> {
    fn make_internal_log_event(&self, channel: LogChannel, payload: Payload) -> LogEvent {
        LogEvent::self_directed(
            Participant {
                actor_type: ActorType::Explorer,
                id: self.id as u32,
            },
            EventType::InternalExplorerAction,
            channel,
            payload,
        )
    }

    fn make_inbound_msg_log_event(&self, channel: LogChannel, payload: Payload) -> LogEvent {
        LogEvent::new(
            Some(Participant {
                actor_type: ActorType::Explorer,
                id: self.id as u32,
            }),
            Some(Participant {
                actor_type: ActorType::Orchestrator,
                id: self.id as u32,
            }),
            EventType::MessageExplorerToOrchestrator,
            channel,
            payload,
        )
    }
}

impl<Any> ExplorerHandle<Born<Any>> {
    pub fn kill_explorer_ai(self) -> Result<JoinHandle<()>, ()> {
        if let Ok(_) = self.state.channel.send_and_check_ack(
            OrchestratorToExplorer::KillExplorer,
            ExplorerToOrchestratorKind::KillExplorerResult,
        ) {
            self.make_internal_log_event(
                LogChannel::Info,
                Payload::from([("Message".into(), "Killed".into())]),
            )
            .emit();
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
            self.make_internal_log_event(
                LogChannel::Info,
                Payload::from([("Message".into(), "Reset".into())]),
            )
            .emit();
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

    pub fn get_bag_content(&self) -> Result<Bag, ()> {
        self.state
            .channel
            .send(OrchestratorToExplorer::BagContentRequest)
            .map_err(|_| {})?;
        let response = self.state.channel.recv().map_err(|_| {})?;
        match response {
            ExplorerToOrchestrator::BagContentResponse {
                explorer_id,
                bag_content,
            } => {
                self.ensure_id_matches(explorer_id)?;
                Ok(bag_content)
            }
            other => {
                let expected = ExplorerToOrchestratorKind::BagContentResponse;
                self.make_inbound_msg_log_event(
                    LogChannel::Error,
                    Payload::from([
                        ("Message".into(), "Recieved invalid response".into()),
                        ("Expected".into(), format!("{expected:?}")),
                        ("Got".into(), format!("{other:?}")),
                    ]),
                );
                Err(())
            }
        }
    }

    fn ensure_id_matches(&self, explorer_id: ID) -> Result<(), ()> {
        if explorer_id != self.id as ID {
            self.make_inbound_msg_log_event(
                LogChannel::Error,
                Payload::from([
                    ("Message".into(), "Explorer returned incoherent ID".into()),
                    ("Expected".into(), format!("{}", self.id)),
                    ("Got".into(), format!("{explorer_id}")),
                ]),
            )
            .emit();
            Err(())
        } else {
            Ok(())
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
            self.make_internal_log_event(
                LogChannel::Info,
                Payload::from([
                    ("Message".into(), "Initialized".into()),
                    ("Planet".into(), format!("{planet_id:}")),
                ]),
            )
            .emit();
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
                    self.ensure_id_matches(explorer_id)?;

                    let old_planet_id = self.state.location.planet_id;
                    self.state.location.planet_id = planet_id;

                    //Check if the returned planet actually matches what is expected, if not the change is reverted and Err(()) is returned
                    self.ensure_planet_matches(
                        planet_id_resp,
                        "Explorer returned unexpected planet_id after move",
                    )
                    .map_err(|_| self.state.location.planet_id = old_planet_id)?;

                    self.make_internal_log_event(
                        LogChannel::Error,
                        Payload::from([
                            ("Message".into(), "Explorer moved to planet".into()),
                            ("Planet".into(), format!("{planet_id:}")),
                        ]),
                    )
                    .emit();
                    self.state.location.planet_id = planet_id;
                    Ok(())
                }
                _ => {
                    let expected = ExplorerToOrchestratorKind::MovedToPlanetResult;
                    self.make_inbound_msg_log_event(
                        LogChannel::Error,
                        Payload::from([
                            ("Message".into(), "Recieved invalid response".into()),
                            ("Expected".into(), format!("{expected:?}")),
                            ("Got".into(), format!("{:?}", val)),
                        ]),
                    )
                    .emit();
                    Err(())
                }
            },
            Err(_) => Err(()),
        }
    }

    fn ensure_planet_matches(&self, planet_id: ID, err_message: &str) -> Result<(), ()> {
        if planet_id != self.state.location.planet_id as ID {
            self.make_inbound_msg_log_event(
                LogChannel::Error,
                Payload::from([
                    ("Message".into(), err_message.into()),
                    ("Expected".into(), format!("{}", self.id)),
                    ("Got".into(), format!("{planet_id}")),
                ]),
            )
            .emit();
            Err(())
        } else {
            Ok(())
        }
    }
}

impl ExplorerHandle<Born<Placed<Paused>>> {
    pub fn unpause_explorer_ai(self) -> Result<ExplorerHandle<Born<Placed<Running>>>, ()> {
        if let Ok(_) = self.state.channel.send_and_check_ack(
            OrchestratorToExplorer::StartExplorerAI,
            ExplorerToOrchestratorKind::StartExplorerAIResult,
        ) {
            self.make_internal_log_event(
                LogChannel::Info,
                Payload::from([("Message".into(), "Unpaused".into())]),
            )
            .emit();
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
            self.make_internal_log_event(
                LogChannel::Info,
                Payload::from([("Message".into(), "Paused".into())]),
            )
            .emit();
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

    /// Hanldles a single incoming request from the explorer.
    /// ---
    /// Returns:
    /// - Ok(true) if a reqeust was handled
    /// - Ok(false) if a request was not handled
    /// - Err(()) when an internal error occurs
    pub fn handle_one_request(&mut self, galaxy: &Galaxy) -> Result<bool, ()> {
        let result = match self.state.channel.poll() {
            //There was an error while polling
            Err(_) => Err(()),
            //No request found
            Ok(None) => Ok(false),
            //Handle the request
            Ok(Some(request)) => match request {
                ExplorerToOrchestrator::NeighborsRequest {
                    explorer_id: recvd_explorer_id,
                    current_planet_id,
                } => self.handle_neighbhors_request(galaxy, recvd_explorer_id, current_planet_id),
                ExplorerToOrchestrator::TravelToPlanetRequest {
                    explorer_id: recvd_explorer_id,
                    current_planet_id,
                    dst_planet_id,
                } => self.handle_travel_to_planet_request(
                    galaxy,
                    recvd_explorer_id,
                    current_planet_id,
                    dst_planet_id,
                ),
                _ => {
                    self.make_inbound_msg_log_event(
                        LogChannel::Error,
                        Payload::from([
                            (
                                "Message".into(),
                                "Recieved response while awaiting requests".into(),
                            ),
                            (
                                "Expected".into(),
                                format!(
                                    "One of {:?}",
                                    [
                                        ExplorerToOrchestratorKind::TravelToPlanetRequest,
                                        ExplorerToOrchestratorKind::NeighborsRequest
                                    ]
                                ),
                            ),
                            ("Got".into(), format!("{request:?}")),
                        ]),
                    )
                    .emit();
                    Err(())
                }
            }
            .map(|_| true),
        };

        return result;
    }

    fn handle_neighbhors_request(
        &self,
        galaxy: &Galaxy,
        explorer_id: ID,
        current_planet_id: ID,
    ) -> Result<(), ()> {
        let assertions_result =
            self.ensure_id_matches(explorer_id)
                .and(self.ensure_planet_matches(
                    current_planet_id,
                    "Explorer asked for neighbhors of planet it is not on",
                ));

        let neighbors = if assertions_result.is_err() {
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
                None => vec![],
            }
        };

        if self
            .state
            .channel
            .send(OrchestratorToExplorer::NeighborsResponse { neighbors })
            .is_err()
        {
            Err(())
        } else {
            Ok(())
        }
        .and(assertions_result) //If some previos assertion has failed, return Err(())
    }

    fn handle_travel_to_planet_request(
        &mut self,
        galaxy: &Galaxy,
        recv_explorer_id: ID,
        current_planet_id: ID,
        dst_planet_id: ID,
    ) -> Result<(), ()> {
        let assertions_result = self
            .ensure_planet_matches(
                current_planet_id,
                "Explorer tried to move out of planet it is not on",
            )
            .and(self.ensure_id_matches(recv_explorer_id));

        let maybe_explorer_to_planet = if assertions_result.is_err() {
            None
        } else {
            galaxy
                .planets
                .get(&current_planet_id)
                .map(|current_planet| {
                    current_planet
                        .lock()
                        .expect("Planet thread must not be poisoned")
                        .adj
                        .iter()
                        .find_map(|neighbor| {
                            let neighbor =
                                neighbor.lock().expect("Planet thread must not be poisoned");
                            (neighbor.id() == dst_planet_id).then(|| neighbor.tx_explorer.clone())
                        })
                })
                .flatten()
        };

        match maybe_explorer_to_planet {
            //Success
            Some(tx_explorer) => self.move_to_planet_intnl(Some(tx_explorer), dst_planet_id),
            //Failure (Planet may have been killed before Explorer had a chance to react. Do not move)
            None => self.move_to_planet_intnl(None, self.state.location.planet_id),
        }
        .and(assertions_result)
    }
}

pub fn new(id: ExplorerID, explorer_vendor: ExplorerVendor) -> ExplorerHandle<Unborn> {
    ExplorerHandle::<Unborn> {
        id,
        explorer_vendor,
        state: Unborn,
    }
}
