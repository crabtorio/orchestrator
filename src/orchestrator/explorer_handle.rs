use std::{
    collections::{HashMap, HashSet},
    format,
    thread::{self, JoinHandle},
    vec,
};

use crate::{
    galaxy_generator::Galaxy,
    orchestrator::{ExplorerID, PlanetHandle},
};
use explorer_common::{Bag, BagContent, Explorer, logged_channel::LoggedChannel};

use common_game::{
    components::resource::{BasicResourceType, ComplexResourceType},
    logging::{ActorType, Channel as LogChannel, EventType, LogEvent, Participant, Payload},
    protocols::{
        orchestrator_explorer::{
            ExplorerToOrchestrator, ExplorerToOrchestratorKind, OrchestratorToExplorer,
        },
        planet_explorer::PlanetToExplorer,
    },
    utils::ID,
};
pub type Channel = LoggedChannel<OrchestratorToExplorer, ExplorerToOrchestrator<BagContent>>;

pub struct ExplorerHandle<State> {
    id: ExplorerID,
    state: State,
}
pub struct Unborn;
pub struct Born<Location> {
    channel: Channel,
    planet_sender: crossbeam_channel::Sender<PlanetToExplorer>,
    handle: JoinHandle<()>,
    location: Location,
}
pub struct Placed<Substate> {
    planet: ID,
    _run_state: Substate,
}
pub struct Paused;
pub struct Running;

pub type UnbornExplorerHandle = ExplorerHandle<Unborn>;
pub type PausedExploererHandle = ExplorerHandle<Born<Placed<Paused>>>;
pub type RunningExploererHandle = ExplorerHandle<Born<Placed<Running>>>;

pub enum GenericExplorer {
    Unborn(UnbornExplorerHandle),
    Running(RunningExploererHandle),
    Stopped(PausedExploererHandle),
}

pub struct ExplorerSet(pub HashMap<ExplorerID, GenericExplorer>);

impl ExplorerSet {
    pub fn get(&self, id: ExplorerID) -> Option<&GenericExplorer> {
        self.0.get(&id)
    }

    pub fn get_mut(&mut self, id: ExplorerID) -> Option<&mut GenericExplorer> {
        self.0.get_mut(&id)
    }

    /// Take a value and, if it's present, do something with it.
    /// If the return value of `op` is None, the explorer is dropped from the set.
    pub fn take_explorer<F: Fn(Option<GenericExplorer>) -> Option<GenericExplorer>>(
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

    pub fn bulk_op<F: Fn(ExplorerID, GenericExplorer) -> Option<GenericExplorer>>(
        &mut self,
        op: F,
    ) {
        self.0 = std::mem::take(&mut self.0)
            .into_iter()
            .filter_map(|(key, explorer)| op(key, explorer).map(|explorer| (key, explorer)))
            .collect();
    }

    pub fn bulk_paused_op<F: Fn(ExplorerID, PausedExploererHandle) -> Option<GenericExplorer>>(
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

    pub fn bulk_running_op<F: Fn(ExplorerID, RunningExploererHandle) -> Option<GenericExplorer>>(
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

    fn make_unexpected_response_log_event(
        &self,
        expected: &dyn std::fmt::Debug,
        got: &dyn std::fmt::Debug,
    ) -> LogEvent {
        self.make_inbound_msg_log_event(
            LogChannel::Error,
            Payload::from([
                ("Message".into(), "Recieved invalid response".into()),
                ("Expected".into(), format!("{expected:?}")),
                ("Got".into(), format!("{got:?}")),
            ]),
        )
    }
}

impl<Any> ExplorerHandle<Born<Any>> {
    /// A leniant version of send_and_check_ack, will check for one extra response before returning err(), dropping all responses.
    fn send_and_check_ack_leniant(
        &self,
        send_val: OrchestratorToExplorer,
        check_val: ExplorerToOrchestratorKind,
    ) -> Result<(), ()> {
        let recieve_and_check_match = || {
            let res = self.state.channel.recv().map_err(|_| {})?.into();
            Ok((check_val == res, res))
        };

        self.state.channel.send(send_val).map_err(|_| {})?;
        let mut tries = 2;
        while tries > 0 {
            match recieve_and_check_match()? {
                (true, _) => return Ok(()),
                (false, val) => {
                    if tries > 0 {
                        self.make_inbound_msg_log_event(LogChannel::Trace, Payload::from([
                            ("Message".into(),"Got first unexpected response while recieveing leniantly, dropping this request.".into()),
                            ("Got".into(),format!("{:?}",val)),
                            ("Expeced".into(),format!("{:?}",check_val))
                        ])).emit()
                    } else {
                        self.make_inbound_msg_log_event(LogChannel::Error, Payload::from([
                            ("Message".into(),"Got second unexpected response while recieveing leniantly, this is an error.".into()),
                            ("Got".into(),format!("{:?}",val)),
                            ("Expeced".into(),format!("{:?}",check_val))
                        ])).emit()
                    }
                }
            };
            tries -= 1;
        }
        Err(())
    }

    pub fn kill(self) -> Result<JoinHandle<()>, ()> {
        if let Ok(_) = self.send_and_check_ack_leniant(
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

    pub fn get_bag_content(&self) -> Result<BagContent, ()> {
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
                self.make_unexpected_response_log_event(&expected, &other)
                    .emit();
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

pub enum PlacedResult<InitialExplorerHandle> {
    DestinationPlanetRefused {
        handle: InitialExplorerHandle,
        reason: String,
    },
    Placed(PausedExploererHandle),
    DestinationPlanetFailed(InitialExplorerHandle),
}
impl ExplorerHandle<Unborn> {
    pub fn spawn_in_place<ExplorerImplementation: Explorer>(
        self,
        initial_planet: &PlanetHandle,
    ) -> PlacedResult<Self> {
        let (tx_explorer, rx_explorer) = crossbeam_channel::unbounded();
        let (tx_orchestrator, rx_orchestrator) = crossbeam_channel::unbounded();
        let (tx_planet, rx_planet) = crossbeam_channel::unbounded();

        let initial_planet_id = initial_planet.id;
        let planet_tx_explorer = initial_planet.tx_explorer.clone();
        
        //Perform check-in
        match initial_planet.incoming_explorer_request(self.id, tx_planet.clone()) {
            //Do nothing
            Ok(Ok(())) => (),
            //Return that the planet was not interested
            Ok(Err(errmsg)) => {
                return PlacedResult::DestinationPlanetRefused {
                    handle: self,
                    reason: errmsg,
                };
            }
            Err(()) => return PlacedResult::DestinationPlanetFailed(self),
        };

        //If the check-in went correctly, spawn the explorer
        let handle = thread::spawn(move || {
            ExplorerImplementation::new(
                self.id as ID,
                Bag::new(),
                initial_planet_id,
                LoggedChannel::new(
                    rx_planet,
                    planet_tx_explorer,
                    Participant {
                        actor_type: ActorType::Explorer,
                        id: self.id as ID,
                    },
                    Participant {
                        actor_type: ActorType::Planet,
                        id: initial_planet_id,
                    },
                    EventType::MessageExplorerToPlanet,
                    EventType::MessagePlanetToExplorer,
                ),
                LoggedChannel::new(
                    rx_orchestrator,
                    tx_explorer,
                    Participant {
                        actor_type: ActorType::Explorer,
                        id: self.id as ID,
                    },
                    Participant {
                        actor_type: ActorType::Orchestrator,
                        id: 0,
                    },
                    EventType::MessageExplorerToOrchestrator,
                    EventType::MessageOrchestratorToExplorer,
                ),
            )
            .run();
        });

        let channel = Channel::new(
            rx_explorer,
            tx_orchestrator,
            Participant {
                actor_type: ActorType::Orchestrator,
                id: self.id as ID,
            },
            Participant {
                actor_type: ActorType::Explorer,
                id: 0,
            },
            EventType::MessageOrchestratorToExplorer,
            EventType::MessageExplorerToOrchestrator,
        );

        self.make_internal_log_event(
            LogChannel::Info,
            Payload::from([("Message".into(), "Initialized".into())]),
        )
        .emit();

        let born_explorer_handle = ExplorerHandle {
            id: self.id,
            state: Born {
                channel,
                handle: handle,
                planet_sender: tx_planet,
                location: Placed {
                    planet: initial_planet_id,
                    _run_state: Paused,
                },
            },
        };
        PlacedResult::Placed(born_explorer_handle)
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

impl<Any> ExplorerHandle<Born<Placed<Any>>> {
    fn move_to_planet_intnl(
        &mut self,
        planet_handles: &HashMap<ID, PlanetHandle>,
        maybe_dest_planet: Option<ID>,
    ) -> Result<MoveResult, MoveResultError> {
        let current_planet_id = self.state.location.planet;

        // If we are actually about to move, check out of the previous planet and check in with the new planet.
        let check_in_out_result = maybe_dest_planet.map(|dest_planet_id| {
            use MoveResult::*;
            use MoveResultPlanetError::*;

            let dest_planet = match planet_handles.get(&dest_planet_id) {
                Some(planet) => planet,
                None => return (dest_planet_id, Err(DestPlanetFailed)),
            };
            let current_planet = match planet_handles.get(&current_planet_id) {
                Some(planet) => planet,
                None => return (dest_planet_id, Err(SourcePlanetFailed)),
            };

            let result = match dest_planet
                .incoming_explorer_request(self.id, self.state.planet_sender.clone())
            {
                //Request completed successfully and was accpeted
                Ok(Ok(())) => match current_planet.outgoing_explorer_request(self.id) {
                    //Request completed successfully and was accpeted
                    Ok(Ok(())) => Ok(Ok(())),
                    //Request did not go through fully. We must revert the request for the explorer to move
                    current_planet_res => match (
                        current_planet_res,
                        dest_planet.outgoing_explorer_request(self.id),
                    ) {
                        //Properly map the return status
                        (Ok(_), Ok(Ok(()))) => Ok(Err(SourcePlanetRefused)),
                        (Ok(_), Ok(Err(_)) | Err(_)) => Err(DestPlanetFailed),
                        (Err(()), Ok(Ok(()))) => Err(SourcePlanetFailed),
                        (Err(()), Ok(Err(_)) | Err(_)) => Err(BothPlanetsFailed),
                    },
                },
                Ok(Err(_)) => Ok(Err(DestPlanetRefused)),
                Err(()) => Err(DestPlanetFailed),
            };
            (dest_planet_id, result)
        });

        let (dest_planet_id, new_sender) = match check_in_out_result {
            Some((planet_id, Ok(Ok(())))) => (
                planet_id,
                planet_handles
                    .get(&planet_id)
                    .map(|planet| planet.tx_explorer.clone()),
            ),
            _ => (current_planet_id, None),
        };

        self.state
            .channel
            .send(OrchestratorToExplorer::MoveToPlanet {
                sender_to_new_planet: new_sender,
                planet_id: dest_planet_id,
            })
            .map_err(|_| MoveResultError::ExplorerFailed)?;

        match self.state.channel.recv() {
            Ok(ExplorerToOrchestrator::MovedToPlanetResult {
                explorer_id,
                planet_id: planet_id_resp,
            }) => {
                self.ensure_id_matches(explorer_id)
                    .map_err(|_| MoveResultError::ExplorerFailed)?;

                if dest_planet_id != planet_id_resp {
                    self.make_inbound_msg_log_event(
                        LogChannel::Error,
                        Payload::from([
                            (
                                "Message".into(),
                                "Explorer returned unexpected planet_id after move".into(),
                            ),
                            ("Expected".into(), format!("{dest_planet_id}")),
                            ("Got".into(), format!("{planet_id_resp}")),
                        ]),
                    )
                    .emit();
                    return Err(MoveResultError::ExplorerFailed);
                }

                self.state.location.planet = dest_planet_id;
                self.make_internal_log_event(
                    LogChannel::Error,
                    Payload::from([
                        ("Message".into(), "Explorer moved to planet".into()),
                        ("Planet".into(), format!("{}", self.state.location.planet)),
                    ]),
                )
                .emit();

                match check_in_out_result {
                    Some((_, Ok(Ok(())))) => Ok(MoveResult::Moved),
                    Some((_, Ok(Err(res)))) => Ok(res),
                    Some((_, Err(res))) => Err(MoveResultError::Planet(res)),
                    None => Ok(MoveResult::Moved),
                }
            }
            Ok(val) => {
                let expected = ExplorerToOrchestratorKind::MovedToPlanetResult;
                self.make_unexpected_response_log_event(&expected, &val)
                    .emit();
                Err(MoveResultError::ExplorerFailed)
            }
            Err(_) => Err(MoveResultError::ExplorerFailed),
        }
    }

    /// Reset the explorer's AI and send it to `dest_planet`.
    pub fn reset(self) -> Result<PausedExploererHandle, ()> {
        if let Ok(_) = self.send_and_check_ack_leniant(
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
                state: Born {
                    channel: self.state.channel,
                    handle: self.state.handle,
                    planet_sender: self.state.planet_sender,
                    location: Placed {
                        planet: self.state.location.planet,
                        _run_state: Paused,
                    },
                },
            })
        } else {
            Err(())
        }
    }

    fn ensure_planet_matches(&self, planet_id: ID, err_message: &str) -> Result<(), ()> {
        if planet_id != self.state.location.planet {
            self.make_inbound_msg_log_event(
                LogChannel::Error,
                Payload::from([
                    ("Message".into(), err_message.into()),
                    ("Expected".into(), format!("{}", self.state.location.planet)),
                    ("Got".into(), format!("{planet_id}")),
                ]),
            )
            .emit();
            Err(())
        } else {
            Ok(())
        }
    }

    /// This method just returns the current planet of the explorer, as is known by the Orchestrator
    pub fn get_current_planet(&self) -> ID {
        self.state.location.planet
    }
}

impl ExplorerHandle<Born<Placed<Paused>>> {
    pub fn start(self) -> Result<RunningExploererHandle, ()> {
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
                state: Born {
                    channel: self.state.channel,
                    handle: self.state.handle,
                    planet_sender: self.state.planet_sender,
                    location: Placed {
                        planet: self.state.location.planet,
                        _run_state: Running,
                    },
                },
            })
        } else {
            Err(())
        }
    }

    pub fn move_to_planet(
        &mut self,
        planet_id: ID,
        planet_handles: &HashMap<ID, PlanetHandle>,
    ) -> Result<MoveResult, MoveResultError> {
        self.move_to_planet_intnl(planet_handles, Some(planet_id))
    }

    pub fn try_combine_resources(
        &self,
        to_generate: ComplexResourceType,
    ) -> Result<Result<(), String>, ()> {
        self.state
            .channel
            .send(OrchestratorToExplorer::CombineResourceRequest { to_generate })
            .map_err(|_| {})?;
        let response = self.state.channel.recv().map_err(|_| {})?;
        match response {
            ExplorerToOrchestrator::CombineResourceResponse {
                explorer_id,
                generated,
            } => {
                self.ensure_id_matches(explorer_id)?;
                Ok(generated)
            }
            other => {
                let expected = ExplorerToOrchestratorKind::CombineResourceResponse;
                self.make_unexpected_response_log_event(&expected, &other)
                    .emit();
                Err(())
            }
        }
    }

    pub fn try_generate_resource(
        &self,
        to_generate: BasicResourceType,
    ) -> Result<Result<(), String>, ()> {
        self.state
            .channel
            .send(OrchestratorToExplorer::GenerateResourceRequest { to_generate })
            .map_err(|_| {})?;
        let response = self.state.channel.recv().map_err(|_| {})?;
        match response {
            ExplorerToOrchestrator::GenerateResourceResponse {
                explorer_id,
                generated,
            } => {
                self.ensure_id_matches(explorer_id)?;
                Ok(generated)
            }
            other => {
                let expected = ExplorerToOrchestratorKind::GenerateResourceResponse;
                self.make_inbound_msg_log_event(
                    LogChannel::Error,
                    Payload::from([
                        ("Message".into(), "Recieved invalid response".into()),
                        ("Expected".into(), format!("{expected:?}")),
                        ("Got".into(), format!("{other:?}")),
                    ]),
                )
                .emit();
                Err(())
            }
        }
    }

    pub fn get_supported_combinations(&self) -> Result<HashSet<ComplexResourceType>, ()> {
        self.state
            .channel
            .send(OrchestratorToExplorer::SupportedCombinationRequest)
            .map_err(|_| {})?;
        let response = self.state.channel.recv().map_err(|_| {})?;
        match response {
            ExplorerToOrchestrator::SupportedCombinationResult {
                explorer_id,
                combination_list,
            } => {
                self.ensure_id_matches(explorer_id)?;
                Ok(combination_list)
            }
            other => {
                let expected = ExplorerToOrchestratorKind::SupportedCombinationResult;
                self.make_unexpected_response_log_event(&expected, &other)
                    .emit();
                Err(())
            }
        }
    }

    pub fn get_supported_resources(&self) -> Result<HashSet<BasicResourceType>, ()> {
        self.state
            .channel
            .send(OrchestratorToExplorer::SupportedResourceRequest)
            .map_err(|_| {})?;
        let response = self.state.channel.recv().map_err(|_| {})?;
        match response {
            ExplorerToOrchestrator::SupportedResourceResult {
                explorer_id,
                supported_resources,
            } => {
                self.ensure_id_matches(explorer_id)?;
                Ok(supported_resources)
            }
            other => {
                let expected = ExplorerToOrchestratorKind::SupportedResourceResult;
                self.make_unexpected_response_log_event(&expected, &other)
                    .emit();
                Err(())
            }
        }
    }
}

impl ExplorerHandle<Born<Placed<Running>>> {
    pub fn stop(self) -> Result<PausedExploererHandle, ()> {
        if let Ok(_) = self.send_and_check_ack_leniant(
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
                state: Born {
                    channel: self.state.channel,
                    handle: self.state.handle,
                    planet_sender: self.state.planet_sender,
                    location: Placed {
                        planet: self.state.location.planet,
                        _run_state: Paused,
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
        planet_handles: &HashMap<ID, PlanetHandle>,
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
                        dst_planet_id,
                        planet_handles,
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
        dest_planet_id: ID,
        planet_handles: &HashMap<ID, PlanetHandle>,
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
                .get(&self.state.location.planet)
                .map(|current_planet| {
                    current_planet
                        .lock()
                        .expect("Planet thread must not be poisoned")
                        .adj
                        .iter()
                        .find_map(|neighbor| {
                            (neighbor
                                .lock()
                                .expect("Planet thread must not be poisoned")
                                .id()
                                == dest_planet_id)
                                .then_some(())
                        })
                })
                .flatten()
        };

        match assertions_result {
            Ok(_) => match maybe_neighbor {
                // Success
                Some(_) => self.move_to_planet_intnl(planet_handles, Some(dest_planet_id)),
                // Failure (the planet may have been killed before the explorer reacted).
                None => self.move_to_planet_intnl(planet_handles, None),
            },
            Err(_) => Err(MoveResultError::ExplorerFailed),
        }
    }
}

pub fn new(id: ExplorerID) -> ExplorerHandle<Unborn> {
    ExplorerHandle::<Unborn> { id, state: Unborn }
}
