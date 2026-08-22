use std::{
    collections::HashMap,
    format,
    thread::{self, JoinHandle},
    vec,
};

use crate::{
    galaxy_generator::Galaxy,
    orchestrator::{ExplorerID, ExplorerVendor, PlanetHandle},
};
use explorer_common::logged_channel::LoggedChannel;

use common_game::{
    logging::{ActorType, Channel as LogChannel, EventType, LogEvent, Participant, Payload},
    protocols::orchestrator_explorer::{
        ExplorerToOrchestrator, ExplorerToOrchestratorKind, OrchestratorToExplorer,
    },
    utils::ID,
};
use explorer_common::Bag;
use luna4::planet::PlanetToExplorer;

pub type Channel = LoggedChannel<OrchestratorToExplorer, ExplorerToOrchestrator<Bag>>;

pub struct ExplorerHandle<State> {
    id: ExplorerID,
    explorer_vendor: ExplorerVendor,
    state: State,
}
pub struct Unborn;
pub struct Born<Location> {
    channel: Channel,
    planet_sender: crossbeam_channel::Sender<PlanetToExplorer>,
    handle: JoinHandle<()>,
    location: Location,
}
pub struct Unplaced;
pub struct Placed<'a, Substate> {
    planet: &'a PlanetHandle,
    run_state: Substate,
}
pub struct Paused;
pub struct Running;

pub type UnbornExplorerHandle = ExplorerHandle<Unborn>;
pub type UnplacedExplorerHandle = ExplorerHandle<Born<Unplaced>>;
pub type PausedExploererHandle<'a> = ExplorerHandle<Born<Placed<'a, Paused>>>;
pub type RunningExploererHandle<'a> = ExplorerHandle<Born<Placed<'a, Running>>>;

pub enum GenericExplorer<'a> {
    Unborn(UnbornExplorerHandle),
    Unplaced(UnplacedExplorerHandle),
    Running(RunningExploererHandle<'a>),
    Stopped(PausedExploererHandle<'a>),
}

pub struct ExplorerSet<'a>(pub HashMap<ExplorerID, GenericExplorer<'a>>);

impl<'a> ExplorerSet<'a> {
    pub fn get(&self, id: ExplorerID) -> Option<&GenericExplorer<'a>> {
        self.0.get(&id)
    }

    pub fn get_mut(&mut self, id: ExplorerID) -> Option<&mut GenericExplorer<'a>> {
        self.0.get_mut(&id)
    }

    /// Take a value and, if it's present, do something with it.
    /// If the return value of `op` is None, the explorer is dropped from the set.
    pub fn take_explorer<F: Fn(Option<GenericExplorer<'a>>) -> Option<GenericExplorer<'a>>>(
        &mut self,
        id: ExplorerID,
        op: F,
    ) {
        let res = op(self.0.remove(&id));
        match res {
            Some(explorer) => {
                self.0.insert(id, explorer);
            }
            None => {}
        }
    }

    pub fn bulk_op<F: Fn(ExplorerID, GenericExplorer<'a>) -> Option<GenericExplorer<'a>>>(
        &mut self,
        op: F,
    ) {
        self.0 = std::mem::take(&mut self.0)
            .into_iter()
            .filter_map(|(key, explorer)| op(key, explorer).map(|explorer| (key, explorer)))
            .collect();
    }

    pub fn bulk_unborn_op<
        F: Fn(ExplorerID, UnbornExplorerHandle) -> Option<GenericExplorer<'a>>,
    >(
        &mut self,
        op: F,
    ) {
        self.bulk_op(|key, explorer| {
            if let GenericExplorer::Unborn(explorer) = explorer {
                op(key, explorer)
            } else {
                Some(explorer)
            }
        });
    }

    pub fn bulk_unplaced_op<
        F: Fn(ExplorerID, UnplacedExplorerHandle) -> Option<GenericExplorer<'a>>,
    >(
        &mut self,
        op: F,
    ) {
        self.bulk_op(|key, explorer| {
            if let GenericExplorer::Unplaced(explorer) = explorer {
                op(key, explorer)
            } else {
                Some(explorer)
            }
        });
    }

    pub fn bulk_paused_op<
        F: Fn(ExplorerID, PausedExploererHandle<'a>) -> Option<GenericExplorer<'a>>,
    >(
        &mut self,
        op: F,
    ) {
        self.bulk_op(|key, explorer| {
            if let GenericExplorer::Stopped(explorer) = explorer {
                op(key, explorer)
            } else {
                Some(explorer)
            }
        });
    }

    pub fn bulk_running_op<
        F: Fn(ExplorerID, RunningExploererHandle<'a>) -> Option<GenericExplorer<'a>>,
    >(
        &mut self,
        op: F,
    ) {
        self.bulk_op(|key, explorer| {
            if let GenericExplorer::Running(explorer) = explorer {
                op(key, explorer)
            } else {
                Some(explorer)
            }
        });
    }
}

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
    pub fn kill(self) -> Result<JoinHandle<()>, ()> {
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

    pub fn reset(self) -> Result<ExplorerHandle<Born<Unplaced>>, ()> {
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
                    planet_sender: self.state.planet_sender,
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

impl<'a> ExplorerHandle<Unborn> {
    pub fn init(self) -> Result<ExplorerHandle<Born<Unplaced>>, ()> {
        let (ex_sender, ex_reciever) = crossbeam_channel::unbounded();
        let (ox_sender, ox_reciever) = crossbeam_channel::unbounded();
        let (planet_sender, planet_reciever) = crossbeam_channel::unbounded();

        let handle = thread::spawn(move || {
            ex_reciever;
            ox_sender;
            planet_reciever;
            //TODO this whole thing
        });
        let channel = Channel::new(ox_reciever, ex_sender, format!("Explorer {}", self.id));
        if let Ok(_) = channel.send_and_check_ack(
            OrchestratorToExplorer::StartExplorerAI,
            ExplorerToOrchestratorKind::StartExplorerAIResult,
        ) {
            self.make_internal_log_event(
                LogChannel::Info,
                Payload::from([("Message".into(), "Initialized".into())]),
            )
            .emit();
            Ok(ExplorerHandle {
                id: self.id,
                explorer_vendor: self.explorer_vendor,
                state: Born {
                    channel,
                    handle: handle,
                    planet_sender,
                    location: Unplaced,
                },
            })
        } else {
            Err(())
        }
    }
}

pub enum MoveResult {
    Moved,
    SourcePlanetRefused,
    DestPlanetRefused,
}

pub enum MoveResultPlanetError {
    SourcePlanetFailed,
    DestPlanetFailed,
    BothPlanetsFailed,
}

pub enum MoveResultError {
    Planet(MoveResultPlanetError),
    ExplorerFailed,
}

impl<'a, Any> ExplorerHandle<Born<Placed<'a, Any>>> {
    fn move_to_planet_intnl(
        &mut self,
        maybe_dest_planet: Option<&'a PlanetHandle>,
    ) -> Result<MoveResult, MoveResultError> {
        let mut current_planet = &self.state.location.planet;

        //If we are actually about to move, we need to check out of the previous planet and check in with the new planet.
        let check_in_out_result = maybe_dest_planet.map(
            |dest_planet| -> (
                &PlanetHandle,
                Result<Result<(), MoveResult>, MoveResultPlanetError>,
            ) {
                let result = match dest_planet
                    .incoming_explorer_request(self.id, self.state.planet_sender.clone())
                {
                    //Request completed successfully and was accpeted
                    Ok(Ok(())) => {
                        match current_planet.outgoing_explorer_request(self.id) {
                            //Request completed successfully and was accpeted
                            Ok(Ok(())) => Ok(Ok(())),
                            //Request did not go through fully. We must revert the request for the explorer to move
                            res => match dest_planet.outgoing_explorer_request(self.id) {
                                Ok(Ok(v)) => Ok(Err(MoveResult::SourcePlanetRefused)),
                                Err(_) | Ok(Err(_)) => {
                                    if res.is_err() {
                                        Err(MoveResultPlanetError::BothPlanetsFailed)
                                    } else {
                                        Err(MoveResultPlanetError::DestPlanetFailed)
                                    }
                                }
                            },
                        }
                    }
                    //Request completed successfully, but was refued.
                    Ok(Err(_)) => Ok(Err(MoveResult::DestPlanetRefused)),
                    //Request failed
                    Err(()) => Err(MoveResultPlanetError::DestPlanetFailed),
                };
                (dest_planet, result)
            },
        );

        let move_result = self.state.channel.send(match check_in_out_result {
            //If we are moving and if the check in/out went out correctly.
            Some((planet, Ok(Ok(())))) => OrchestratorToExplorer::MoveToPlanet {
                sender_to_new_planet: Some(planet.tx_explorer.clone()),
                planet_id: planet.id,
            },
            //Othrewiese
            _ => OrchestratorToExplorer::MoveToPlanet {
                sender_to_new_planet: None,
                planet_id: self.state.location.planet.id,
            },
        });

        if move_result.is_err() {
            return Err(MoveResultError::ExplorerFailed);
        }

        //Check the response's validity
        match self.state.channel.recv() {
            Ok(val) => match val {
                ExplorerToOrchestrator::MovedToPlanetResult {
                    explorer_id,
                    planet_id: planet_id_resp,
                } => {
                    self.ensure_id_matches(explorer_id)
                        .map_err(|_| MoveResultError::ExplorerFailed)?;

                    let expected_planet = maybe_dest_planet.unwrap_or(self.state.location.planet);

                    if expected_planet.id != planet_id_resp {
                        self.make_inbound_msg_log_event(
                            LogChannel::Error,
                            Payload::from([
                                (
                                    "Message".into(),
                                    "Explorer returned unexpected planet_id after move".into(),
                                ),
                                ("Expected".into(), format!("{}", expected_planet.id)),
                                ("Got".into(), format!("{planet_id_resp}")),
                            ]),
                        )
                        .emit();

                        return Err(MoveResultError::ExplorerFailed);
                    }

                    self.state.location.planet = expected_planet;

                    self.make_internal_log_event(
                        LogChannel::Error,
                        Payload::from([
                            ("Message".into(), "Explorer moved to planet".into()),
                            (
                                "Planet".into(),
                                format!("{:}", self.state.location.planet.id),
                            ),
                        ]),
                    )
                    .emit();

                    // Map the errors properly before returning
                    match check_in_out_result {
                        Some((_, Ok(Ok(())))) => Ok(MoveResult::Moved),
                        Some((_, Ok(Err(res)))) => Ok(res),
                        Some((_, Err(res))) => Err(MoveResultError::Planet(res)),
                        None => Ok(MoveResult::Moved),
                    }
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
                    Err(MoveResultError::ExplorerFailed)
                }
            },
            Err(_) => Err(MoveResultError::ExplorerFailed),
        }
    }

    fn ensure_planet_matches(&self, planet_id: ID, err_message: &str) -> Result<(), ()> {
        if planet_id != self.state.location.planet.id as ID {
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

impl<'a> ExplorerHandle<Born<Placed<'a, Paused>>> {
    pub fn resume(self) -> Result<ExplorerHandle<Born<Placed<'a, Running>>>, ()> {
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
                    planet_sender: self.state.planet_sender,
                    location: Placed {
                        planet: self.state.location.planet,
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
        planet: &'a PlanetHandle,
    ) -> Result<MoveResult, MoveResultError> {
        self.move_to_planet_intnl(Some(planet))
    }

    //TODO all other manual mode funcs
}

impl<'a> ExplorerHandle<Born<Placed<'a, Running>>> {
    pub fn stop(self) -> Result<ExplorerHandle<Born<Placed<'a, Paused>>>, ()> {
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
                    planet_sender: self.state.planet_sender,
                    location: Placed {
                        planet: self.state.location.planet,
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
    pub fn handle_one_request(
        &mut self,
        galaxy: &Galaxy,
        planet_handles: &'a HashMap<ID, PlanetHandle>,
    ) -> Result<bool, ()> {
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
                } => self
                    .handle_travel_to_planet_request(
                        galaxy,
                        recvd_explorer_id,
                        current_planet_id,
                        planet_handles.get(&dst_planet_id),
                    )
                    .map(|_| {})
                    .map_err(|_| {}),
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
        dest_planet: Option<&'a PlanetHandle>,
    ) -> Result<MoveResult, MoveResultError> {
        let assertions_result = self
            .ensure_planet_matches(
                current_planet_id,
                "Explorer tried to move out of planet it is not on",
            )
            .and(self.ensure_id_matches(recv_explorer_id));

        let maybe_neighbor = if assertions_result.is_err() {
            None
        } else {
            galaxy
                .planets
                .get(&self.state.location.planet.id)
                .map(|current_planet| {
                    current_planet
                        .lock()
                        .expect("Planet thread must not be poisoned")
                        .adj
                        .iter()
                        .find_map(|neighbor| {
                            let neighbor = neighbor;
                            (neighbor
                                .lock()
                                .expect("Planet thread must not be poisoned")
                                .id()
                                == dest_planet.unwrap_or(self.state.location.planet).id)
                                .then(|| neighbor.clone())
                        })
                })
                .flatten()
        };

        match assertions_result {
            Ok(_) => match maybe_neighbor {
                //Success
                Some(_) => self.move_to_planet_intnl(dest_planet),
                //Failure (Planet may have been killed before Explorer had a chance to react. Do not move)
                None => self.move_to_planet_intnl(None),
            },
            Err(_) => Err(MoveResultError::ExplorerFailed),
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
