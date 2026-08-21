use common_game::logging::Channel;
use common_game::utils::ID;
use eframe::egui;
use crate::compat::ExplorerType;
use crate::compat::id::PlanetKind;
use crate::compat::logging::LogTarget;
use crate::compat::payload;
use crate::compat::ui::UiToOrchestratorCommand;
use std::time::Instant;

use crate::gui::comms::OrchestratorComms;
use crate::gui::models::{Planet, SpawnStage};
use crate::gui::state::{AnimationState, ExplorerState, UiState};

// ---------------------------------------------------------------------------
// Spawn-planet flow
// ---------------------------------------------------------------------------

/// Dispatcher: show the appropriate spawn sub-menu based on the current stage.
pub fn handle_spawn_menus(
    ctx: &egui::Context,
    ui_state: &mut UiState,
    planets: &[Planet],
    comms: &OrchestratorComms,
) {
    // Copy values out so we can pass &mut ui_state to the sub-functions.
    let Some(pos) = ui_state.pending_spawn_pos else {
        return;
    };
    match ui_state.spawn_stage {
        SpawnStage::SelectingType => {
            show_planet_type_menu(ctx, pos, ui_state);
        }
        SpawnStage::SelectingNeighbors(planet_kind) => {
            show_neighbor_selection_menu(ctx, pos, planet_kind, planets, ui_state, comms);
        }
        SpawnStage::None => {}
    }
}

fn show_planet_type_menu(ctx: &egui::Context, pos: egui::Pos2, ui_state: &mut UiState) {
    let mut chosen_type: Option<PlanetKind> = None;

    egui::Area::new(egui::Id::new("planet_type_menu"))
        .fixed_pos(pos)
        .show(ctx, |ui| {
            ui.vertical(|ui| {
                ui.label("Select Planet Type:");
                if ui.button("Rusty Crab").clicked() {
                    chosen_type = Some(PlanetKind::RustyCrab);
                } else if ui.button("Rustrelli").clicked() {
                    chosen_type = Some(PlanetKind::Rustrelli);
                } else if ui.button("Orbitron").clicked() {
                    chosen_type = Some(PlanetKind::Orbitron);
                } else if ui.button("Houston we have a borrow").clicked() {
                    chosen_type = Some(PlanetKind::Houston);
                } else if ui.button("Trip").clicked() {
                    chosen_type = Some(PlanetKind::Trip);
                } else if ui.button("Luna4").clicked() {
                    chosen_type = Some(PlanetKind::Luna4);
                } else if ui.button("Enterprise").clicked() {
                    chosen_type = Some(PlanetKind::Enterprise);
                }
                if ui.button("Cancel").clicked() {
                    ui_state.pending_spawn_pos = None;
                    ui_state.spawn_stage = SpawnStage::None;
                }
            });
        });

    if let Some(new_type) = chosen_type {
        ui_state.spawn_stage = SpawnStage::SelectingNeighbors(new_type);
        ui_state.selected_neighbors.clear();
    }
}

fn show_neighbor_selection_menu(
    ctx: &egui::Context,
    pos: egui::Pos2,
    planet_kind: PlanetKind,
    planets: &[Planet],
    ui_state: &mut UiState,
    comms: &OrchestratorComms,
) {
    let mut confirm_pressed = false;
    let mut cancel_pressed = false;

    egui::Area::new(egui::Id::new("neighbor_selection_menu"))
        .fixed_pos(pos)
        .show(ctx, |ui| {
            ui.vertical(|ui| {
                ui.label("Select Neighbors (or none):");

                for planet in planets {
                    let mut is_selected = ui_state.selected_neighbors.contains(&planet.id);
                    ui.checkbox(&mut is_selected, planet.name.clone());
                    if is_selected {
                        ui_state.selected_neighbors.insert(planet.id);
                    } else {
                        ui_state.selected_neighbors.remove(&planet.id);
                    }
                }

                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("✓ Confirm").clicked() {
                        confirm_pressed = true;
                    }
                    if ui.button("✗ Cancel").clicked() {
                        cancel_pressed = true;
                    }
                });
            });
        });

    if confirm_pressed {
        let neighbors: Vec<ID> = ui_state.selected_neighbors.iter().copied().collect();
        // Don't add to self.planets manually - let the galaxy rebuild handle positioning
        // This ensures all planets are arranged in the circle layout

        comms.send_expect(
            UiToOrchestratorCommand::AddPlanet(planet_kind, neighbors),
            "Failed to send AddPlanet command",
        );

        // Request galaxy update to get proper circular positioning
        comms.send_expect(
            UiToOrchestratorCommand::GetGalaxy,
            "Failed to send GetGalaxy command",
        );

        ui_state.pending_spawn_pos = None;
        ui_state.spawn_stage = SpawnStage::None;
        ui_state.selected_neighbors.clear();
    }

    if cancel_pressed {
        ui_state.pending_spawn_pos = None;
        ui_state.spawn_stage = SpawnStage::None;
        ui_state.selected_neighbors.clear();
    }
}

// ---------------------------------------------------------------------------
// Planet context menu
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_lines)]
pub fn show_context_menu(
    ctx: &egui::Context,
    pos: egui::Pos2,
    planet_id: ID,
    ui_state: &mut UiState,
    explorer_state: &ExplorerState,
    animation_state: &mut AnimationState,
    comms: &OrchestratorComms,
) {
    let mut manual_asteroid = false;
    let mut manual_sunray = false;
    let mut start_ai = false;
    let mut stop_ai = false;
    let mut reset_ai = false;
    let mut kill_planet = false;
    let mut spawn_nico_explorer = false;
    let mut spawn_vojager = false;
    let mut spawn_nomad = false;
    let mut close_menu = false;

    egui::Area::new(egui::Id::new("context_menu"))
        .fixed_pos(pos)
        .show(ctx, |ui| {
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(6.0, 4.0);
                ui.spacing_mut().button_padding = egui::vec2(6.0, 4.0);

                ui.label(egui::RichText::new(format!("Planet {planet_id}")).size(13.0));
                ui.separator();

                if ui
                    .add(egui::Button::new(
                        egui::RichText::new("👤 Spawn Nico Explorer").size(13.0),
                    ))
                    .clicked()
                {
                    spawn_nico_explorer = true;
                }
                if ui
                    .add(egui::Button::new(
                        egui::RichText::new("👤 Spawn Rob Vojager").size(13.0),
                    ))
                    .clicked()
                {
                    spawn_vojager = true;
                }
                if ui
                    .add(egui::Button::new(
                        egui::RichText::new("👤 Spawn Jaco Nomad").size(13.0),
                    ))
                    .clicked()
                {
                    spawn_nomad = true;
                }
                if ui
                    .add(egui::Button::new(
                        egui::RichText::new("Send Asteroid").size(13.0),
                    ))
                    .clicked()
                {
                    manual_asteroid = true;
                }
                if ui
                    .add(egui::Button::new(
                        egui::RichText::new("Send Sunray").size(13.0),
                    ))
                    .clicked()
                {
                    manual_sunray = true;
                }
                if ui
                    .add(egui::Button::new(
                        egui::RichText::new("Start Planet AI").size(13.0),
                    ))
                    .clicked()
                {
                    start_ai = true;
                }
                if ui
                    .add(egui::Button::new(
                        egui::RichText::new("Stop Planet AI").size(13.0),
                    ))
                    .clicked()
                {
                    stop_ai = true;
                }
                if ui
                    .add(egui::Button::new(
                        egui::RichText::new("Reset Planet AI").size(13.0),
                    ))
                    .clicked()
                {
                    reset_ai = true;
                }
                if ui
                    .add(egui::Button::new(
                        egui::RichText::new("Kill Planet").size(13.0),
                    ))
                    .clicked()
                {
                    kill_planet = true;
                }
                if ui
                    .add(egui::Button::new(egui::RichText::new("✗ Close").size(13.0)))
                    .clicked()
                {
                    close_menu = true;
                }
            });
        });

    // Handling selections outside the closure to avoid borrow issues
    if spawn_nico_explorer || spawn_vojager || spawn_nomad {
        let expl_type;
        if spawn_nico_explorer {
            expl_type = ExplorerType::Explorer;
        } else if spawn_vojager {
            expl_type = ExplorerType::Vojager;
        } else {
            expl_type = ExplorerType::Nomad;
        }

        // Use the authoritative explorer positions map for counting explorers
        let explorer_count = explorer_state.explorer_positions.len();
        if explorer_count >= 2 {
            let msg = "❗ Explorer limit reached, cannot spawn more.".to_string();
            crate::compat::logging::log_internal(
                LogTarget::General,
                Channel::Warning,
                payload!(
                    message : msg.clone(),
                    explorer_count : explorer_count,
                ),
            );
            ui_state.explorer_limit_popup = Some(msg);
        } else {
            comms.send(UiToOrchestratorCommand::AddExplorer(expl_type, planet_id));
            // Immediately request updated explorer positions and planet state
            comms.send(UiToOrchestratorCommand::GetExplorersPosition);
            comms.send(UiToOrchestratorCommand::GetPlanetSnapshot(planet_id));
        }
        close_menu = true;
    }

    if manual_asteroid {
        comms.send(UiToOrchestratorCommand::SendManualAsteroid(planet_id));
        animation_state.sending_asteroid = Some((planet_id, Instant::now()));
        // Schedule refresh after 100ms to let orchestrator process the asteroid
        animation_state
            .planets_to_refresh
            .push((planet_id, Instant::now()));
        // Request galaxy update to catch planet death
        comms.send(UiToOrchestratorCommand::GetGalaxy);
        // Request planet snapshot to see damage/death
        comms.send(UiToOrchestratorCommand::GetPlanetSnapshot(planet_id));
        close_menu = true;
    }

    if manual_sunray {
        crate::compat::logging::log_internal(
            LogTarget::ChannelMessages,
            Channel::Info,
            payload!(
                action : "send_manual_sunray",
                planet_id : planet_id,
            ),
        );
        comms.send(UiToOrchestratorCommand::SendManualSunray(planet_id));
        animation_state.sending_sunray = Some((planet_id, Instant::now()));
        // Schedule refresh after 100ms to let orchestrator process the sunray
        animation_state
            .planets_to_refresh
            .push((planet_id, Instant::now()));

        // Request planet snapshot to see sunray effect
        comms.send(UiToOrchestratorCommand::GetPlanetSnapshot(planet_id));

        close_menu = true;
    }

    if start_ai {
        comms.send(UiToOrchestratorCommand::StartPlanetAI(planet_id));
        // Request immediate update to see rocket status changes
        comms.send(UiToOrchestratorCommand::GetPlanetSnapshot(planet_id));
        close_menu = true;
    }

    if stop_ai {
        comms.send(UiToOrchestratorCommand::StopPlanetAI(planet_id));
        comms.send(UiToOrchestratorCommand::GetPlanetSnapshot(planet_id));
        close_menu = true;
    }

    if reset_ai {
        comms.send(UiToOrchestratorCommand::ResetPlanetAI(planet_id));
        comms.send(UiToOrchestratorCommand::GetPlanetSnapshot(planet_id));
        close_menu = true;
    }

    if kill_planet {
        comms.send(UiToOrchestratorCommand::KillPlanet(planet_id));
        comms.send(UiToOrchestratorCommand::GetGalaxy);
        close_menu = true;
    }

    if close_menu {
        ui_state.close_planet_menu();
    }
}

// ---------------------------------------------------------------------------
// Explorer context menu
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_lines)]
pub fn show_explorer_menu(
    ctx: &egui::Context,
    pos: egui::Pos2,
    explorer_id: ID,
    ui_state: &mut UiState,
    comms: &OrchestratorComms,
) {
    let mut stop_expl_ai = false;
    let mut reset_expl_ai = false;
    let mut start_expl_ai = false;
    let mut kill_explorer = false;
    let mut move_to_planet = false;
    let mut generate_resource = false;
    let mut craft_resource = false;
    let mut close = false;

    egui::Area::new(egui::Id::new("explorer_context_menu"))
        .fixed_pos(pos)
        .show(ctx, |ui| {
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(6.0, 4.0);
                ui.spacing_mut().button_padding = egui::vec2(6.0, 4.0);

                ui.label(egui::RichText::new(format!("Explorer {explorer_id}")).size(13.0));
                ui.separator();

                if ui
                    .add(egui::Button::new(
                        egui::RichText::new("Move To Planet").size(13.0),
                    ))
                    .clicked()
                {
                    // open neighbor-only selector after closing this menu
                    move_to_planet = true;
                }
                if ui
                    .add(egui::Button::new(
                        egui::RichText::new("Generate Resource").size(13.0),
                    ))
                    .clicked()
                {
                    generate_resource = true;
                }
                if ui
                    .add(egui::Button::new(
                        egui::RichText::new("Craft Resource").size(13.0),
                    ))
                    .clicked()
                {
                    craft_resource = true;
                }
                if ui
                    .add(egui::Button::new(
                        egui::RichText::new("Start Explorer AI").size(13.0),
                    ))
                    .clicked()
                {
                    start_expl_ai = true;
                }
                if ui
                    .add(egui::Button::new(
                        egui::RichText::new("Stop Explorer AI").size(13.0),
                    ))
                    .clicked()
                {
                    stop_expl_ai = true;
                }
                if ui
                    .add(egui::Button::new(
                        egui::RichText::new("Reset Explorer AI").size(13.0),
                    ))
                    .clicked()
                {
                    reset_expl_ai = true;
                }
                if ui
                    .add(egui::Button::new(
                        egui::RichText::new("Kill Explorer").size(13.0),
                    ))
                    .clicked()
                {
                    kill_explorer = true;
                }

                if ui
                    .add(egui::Button::new(egui::RichText::new("✗ Close").size(13.0)))
                    .clicked()
                {
                    close = true;
                }
            });
        });
    if start_expl_ai {
        comms.send(UiToOrchestratorCommand::StartExplorerAI(explorer_id));
        // Request updates to track explorer movement
        comms.send(UiToOrchestratorCommand::GetExplorersPosition);
        comms.send(UiToOrchestratorCommand::GetExplorerSnapshot(explorer_id));
        close = true;
    }

    if reset_expl_ai {
        comms.send(UiToOrchestratorCommand::ResetExplorerAI(explorer_id));
        comms.send(UiToOrchestratorCommand::GetExplorersPosition);
        close = true;
    }

    if stop_expl_ai {
        comms.send(UiToOrchestratorCommand::StopExplorerAI(explorer_id));
        comms.send(UiToOrchestratorCommand::GetExplorersPosition);
        close = true;
    }

    if kill_explorer {
        comms.send(UiToOrchestratorCommand::KillExplorer(explorer_id));
        comms.send(UiToOrchestratorCommand::GetExplorersPosition);
        close = true;
    }

    if move_to_planet {
        // prepare neighbor-only selector: record explorer id and position
        ui_state.pending_move_explorer = Some(explorer_id);
        ui_state.pending_move_pos = Some(pos);
        close = true;
    }

    if generate_resource {
        // ask orchestrator for supported resources for the planet where this explorer is
        ui_state.pending_generate_explorer = Some(explorer_id);
        // clear previous options
        ui_state.resource_options = None;
        crate::compat::logging::log_internal(
            LogTarget::ChannelMessages,
            Channel::Info,
            payload!(
                action : "request_supported_resources",
                requester_explorer_id : explorer_id,
            ),
        );
        comms.send(UiToOrchestratorCommand::SupportedResources(explorer_id));

        close = true;
    }

    if craft_resource {
        // ask orchestrator for supported combinations for this explorer, then show selection
        ui_state.pending_craft_explorer = Some(explorer_id);
        ui_state.combination_options = None;
        crate::compat::logging::log_internal(
            LogTarget::ChannelMessages,
            Channel::Info,
            payload!(
                action : "request_supported_combinations",
                requester_explorer_id : explorer_id,
            ),
        );
        comms.send(UiToOrchestratorCommand::SupportedCombinations(explorer_id));
        close = true;
    }

    if close {
        ui_state.close_explorer_menu();
    }
}
