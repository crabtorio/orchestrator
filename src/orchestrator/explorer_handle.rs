use std::thread::{self, JoinHandle};

use common_game::{
    protocols::{
        orchestrator_explorer::{
            ExplorerToOrchestrator, ExplorerToOrchestratorKind, OrchestratorToExplorer,
        },
        planet_explorer,
    },
    utils::ID,
};
use explorer_common::Bag;

use crate::orchestrator::{ExplorerID, ExplorerVendor, LoggedChannel};

pub struct ExplorerHandle {
    pub id: ExplorerID,
    pub current_planet: ID,
    pub channel: LoggedChannel<OrchestratorToExplorer, ExplorerToOrchestrator<Bag>>,
    handle: JoinHandle<()>,
}

impl ExplorerHandle {
    pub fn spawn(id: ExplorerID, vendor: ExplorerVendor, current_planet: ID) -> Self {
        let (ex_sender, ex_reciever) = crossbeam_channel::unbounded();
        let (ox_sender, ox_reciever) = crossbeam_channel::unbounded();

        let handle = thread::spawn(move || {
            ex_reciever;
            ox_sender;
            //TODO this whole thing
        });

        ExplorerHandle {
            id,
            handle,
            current_planet,
            channel: LoggedChannel::<OrchestratorToExplorer, ExplorerToOrchestrator<Bag>> {
                sender: ex_sender,
                reciever: ox_reciever,
                reciever_ident: "()".to_string(),
            },
        }
    }

    pub fn start_explorer_ai(&self) -> Result<(), ()> {
        if let Ok(_) = self.channel.send_and_check_ack(
            OrchestratorToExplorer::StartExplorerAI,
            ExplorerToOrchestratorKind::StopExplorerAIResult,
        ) {
            log::info!("Explorer {} started", self.id);
            Ok(())
        } else {
            Err(())
        }
    }

    pub fn reset_explorer_ai(&self) -> Result<(), ()> {
        if let Ok(_) = self.channel.send_and_check_ack(
            OrchestratorToExplorer::ResetExplorerAI,
            ExplorerToOrchestratorKind::ResetExplorerAIResult,
        ) {
            log::info!("Explorer {} reset", self.id);
            Ok(())
        } else {
            Err(())
        }
    }

    pub fn stop_explorer_ai(&self) -> Result<(), ()> {
        if let Ok(_) = self.channel.send_and_check_ack(
            OrchestratorToExplorer::StopExplorerAI,
            ExplorerToOrchestratorKind::StopExplorerAIResult,
        ) {
            log::info!("Explorer {} stopped", self.id);
            Ok(())
        } else {
            Err(())
        }
    }

    pub fn kill_explorer(self) -> Result<JoinHandle<()>, ()> {
        if let Ok(_) = self.channel.send_and_check_ack(
            OrchestratorToExplorer::KillExplorer,
            ExplorerToOrchestratorKind::KillExplorerResult,
        ) {
            log::info!("Explorer {} killed", self.id);
            Ok(self.handle)
        } else {
            Err(())
        }
    }

    pub fn move_to_planet(
        &self,
        sender: Option<crossbeam_channel::Sender<planet_explorer::ExplorerToPlanet>>,
        planet_id: ID,
    ) -> Result<(), ()> {
        let result = self.channel.send(OrchestratorToExplorer::MoveToPlanet {
            sender_to_new_planet: sender,
            planet_id,
        });
        if result.is_err() {
            return Err(());
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
                    Ok(())
                }
                _ => {
                    log::error!(
                        "Invalid response from {:?}. Expected {:?}, got {:?}",
                        self.channel.reciever_ident,
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
