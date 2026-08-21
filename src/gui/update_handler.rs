use common_game::components::resource::{BasicResourceType, ComplexResourceType};
use common_game::logging::Channel;
use common_game::utils::ID;
use crate::compat::logging::LogTarget;
use crate::compat::payload;
use crate::compat::ui::{OrchestratorToUiUpdate, UiToOrchestratorCommand};
use std::time::Instant;

use crate::gui::comms::OrchestratorComms;
use crate::gui::state::{AnimationState, ExplorerState, GalaxyState, UiState};

/// Drain all pending messages from the orchestrator and update application state.
#[expect(
    clippy::cast_precision_loss,
    reason = "charged_cells_count (usize) cast to f32 for animation interpolation; cell counts are always small"
)]
pub fn handle_orchestrator_updates(
    comms: &OrchestratorComms,
    galaxy_state: &mut GalaxyState,
    explorer_state: &mut ExplorerState,
    animation_state: &mut AnimationState,
    ui_state: &mut UiState,
    end_game_requested: &mut bool,
    ended: &mut bool,
) {
    while let Ok(cmd) = comms.update_receiver.try_recv() {
        match cmd {
            OrchestratorToUiUpdate::Galaxy(galaxy) => {
                galaxy_state.galaxy = Some(galaxy);
                galaxy_state.galaxy_needs_rebuild = true;
            }
            OrchestratorToUiUpdate::DeadPlanet(id) => {
                handle_dead_planet(id, galaxy_state, explorer_state, ui_state, comms);
            }
            OrchestratorToUiUpdate::ExplorersPosition(positions) => {
                crate::compat::logging::log_internal(
                    LogTarget::ChannelMessages,
                    Channel::Debug,
                    payload!(
                        action : "received_explorers_position",
                    ),
                );
                explorer_state.explorer_positions.clear();
                for entry in &positions {
                    explorer_state
                        .explorer_positions
                        .insert(*entry.key(), *entry.value());
                }
            }
            OrchestratorToUiUpdate::PlanetSnapshot(id, snapshot) => {
                crate::compat::logging::log_internal(
                    LogTarget::ChannelMessages,
                    Channel::Trace,
                    payload!(
                        action : "received_planet_snapshot",
                        planet_id : id,
                    ),
                );
                // ensure we have a displayed counter initialized so it can animate
                animation_state
                    .planet_displayed_charged
                    .entry(id)
                    .or_insert(snapshot.charged_cells_count as f32);
                // store actual snapshot
                galaxy_state.planet_states.insert(id, snapshot);
            }
            OrchestratorToUiUpdate::ExplorerSnapshot(id, bag) => {
                crate::compat::logging::log_internal(
                    LogTarget::ChannelMessages,
                    Channel::Debug,
                    payload!(
                        action : "received_explorer_snapshot",
                        explorer_id : id,
                        bag: format!("{:?}", bag)
                    ),
                );
                explorer_state.explorer_bags.insert(id, bag);
            }

            // draw supported combinations/resources, spawned when someone wants to craft/combine
            OrchestratorToUiUpdate::SupportedCombinations(explorer_id, combinations) => {
                handle_supported_combinations(explorer_id, combinations, ui_state);
            }
            OrchestratorToUiUpdate::SupportedResources(explorer_id, resources) => {
                handle_supported_resources(explorer_id, resources, ui_state);
            }

            // just draw sunray/asteroid
            OrchestratorToUiUpdate::SendAutoSunray(planet_id) => {
                handle_auto_sunray(planet_id, galaxy_state, animation_state, comms);
            }
            OrchestratorToUiUpdate::SendAutoAsteroid(planet_id) => {
                handle_auto_asteroid(planet_id, animation_state, comms);
            }

            OrchestratorToUiUpdate::GameOver(reason) => {
                comms.send(UiToOrchestratorCommand::EndGame);
                *end_game_requested = true;
                *ended = true;
                ui_state.game_over_popup = Some(reason);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn handle_dead_planet(
    id: ID,
    galaxy_state: &mut GalaxyState,
    explorer_state: &mut ExplorerState,
    ui_state: &mut UiState,
    comms: &OrchestratorComms,
) {
    galaxy_state.planets.iter_mut().for_each(|planet| {
        if planet.id == id {
            planet.active = false;
        }
    });
    // Request galaxy update to ensure we have latest state.
    comms.send(UiToOrchestratorCommand::GetGalaxy);

    // Clean up any pending UI operations that reference explorers on
    // the dead planet, but do not remove the explorers from
    // the position/bag maps. Let the normal polling cycle sync
    let dead_explorers: Vec<ID> = explorer_state
        .explorer_positions
        .iter()
        .filter(|(_, planet_id)| **planet_id == id)
        .map(|(explorer_id, _)| *explorer_id)
        .collect();

    for explorer_id in &dead_explorers {
        if ui_state.pending_generate_explorer == Some(*explorer_id) {
            ui_state.pending_generate_explorer = None;
            ui_state.resource_options = None;
        }
        if ui_state.pending_craft_explorer == Some(*explorer_id) {
            ui_state.pending_craft_explorer = None;
            ui_state.combination_options = None;
        }
    }
}

fn handle_supported_combinations(
    explorer_id: ID,
    combinations: std::collections::HashSet<ComplexResourceType>,
    ui_state: &mut UiState,
) {
    crate::compat::logging::log_internal(
        LogTarget::ChannelMessages,
        Channel::Info,
        payload!(
            action : "received_supported_combinations",
            explorer_id : explorer_id,
            supported_combo: format!("{:?}", combinations)
        ),
    );
    let vec: Vec<ComplexResourceType> = combinations.into_iter().collect();
    if ui_state.pending_craft_explorer == Some(explorer_id) {
        ui_state.combination_options = Some(vec);
    }
}

fn handle_supported_resources(
    explorer_id: ID,
    resources: std::collections::HashSet<BasicResourceType>,
    ui_state: &mut UiState,
) {
    crate::compat::logging::log_internal(
        LogTarget::ChannelMessages,
        Channel::Info,
        payload!(
            action : "received_supported_resources",
            explorer_id : explorer_id,
            supported_resources: format!("{:?}", resources)
        ),
    );
    let vec: Vec<BasicResourceType> = resources.into_iter().collect();
    // cache by planet id (look up planet from explorer position)
    if ui_state.pending_generate_explorer == Some(explorer_id) {
        ui_state.resource_options = Some(vec);
    }
}

#[expect(
    clippy::cast_precision_loss,
    reason = "energy_cells.len() (usize) cast to f32 for animation clamp; cell count is always small"
)]
fn handle_auto_sunray(
    planet_id: ID,
    galaxy_state: &GalaxyState,
    animation_state: &mut AnimationState,
    comms: &OrchestratorComms,
) {
    crate::compat::logging::log_internal(
        LogTarget::ChannelMessages,
        Channel::Debug,
        payload!(
            action : "auto sunray received: drawing it",
            planet_id : planet_id,
        ),
    );
    animation_state.sending_sunray = Some((planet_id, Instant::now()));
    // Schedule refresh after a short delay to let orchestrator process the sunray
    animation_state
        .planets_to_refresh
        .push((planet_id, Instant::now()));

    // Request immediate snapshot to catch rocket status
    comms.send(UiToOrchestratorCommand::GetPlanetSnapshot(planet_id));

    // Clamp displayed charged counter to the maximum number of energy cells
    if let Some(state) = galaxy_state.planet_states.get(&planet_id) {
        let max_charged = state.energy_cells.len() as f32;
        animation_state
            .planet_displayed_charged
            .entry(planet_id)
            .and_modify(|v| {
                if *v > max_charged {
                    *v = max_charged;
                }
            })
            .or_insert(max_charged);
    }
}

fn handle_auto_asteroid(
    planet_id: ID,
    animation_state: &mut AnimationState,
    comms: &OrchestratorComms,
) {
    animation_state.sending_asteroid = Some((planet_id, Instant::now()));

    // Schedule refresh after 100ms to let orchestrator process the asteroid
    animation_state
        .planets_to_refresh
        .push((planet_id, Instant::now()));

    // Request galaxy update to catch planet death
    comms.send(UiToOrchestratorCommand::GetGalaxy);

    // Request immediate snapshot to catch planet death/energy decrease
    comms.send(UiToOrchestratorCommand::GetPlanetSnapshot(planet_id));
    crate::compat::logging::log_internal(
        LogTarget::ChannelMessages,
        Channel::Debug,
        payload!(
            action : "auto asteroid received: drawing it",
            planet_id : planet_id,
        ),
    );
}
