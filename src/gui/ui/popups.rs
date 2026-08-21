use common_game::components::resource::{BasicResourceType, ComplexResourceType};
use common_game::logging::Channel;
use common_game::utils::ID;
use eframe::egui;
use crate::compat::logging::LogTarget;
use crate::compat::payload;
use crate::compat::ui::UiToOrchestratorCommand;

use crate::gui::comms::OrchestratorComms;
use crate::gui::state::{ExplorerState, GalaxyState, UiState};

// ---------------------------------------------------------------------------
// Explorer move-to-planet selector
// ---------------------------------------------------------------------------

pub fn show_move_selector(
    ctx: &egui::Context,
    ui_state: &mut UiState,
    explorer_state: &ExplorerState,
    galaxy_state: &GalaxyState,
    comms: &OrchestratorComms,
) {
    let Some(explorer_id) = ui_state.pending_move_explorer else {
        return;
    };
    let Some(pos) = ui_state.pending_move_pos else {
        return;
    };

    // determine current planet for this explorer
    if let Some(current_planet) = explorer_state.explorer_positions.get(&explorer_id).copied() {
        // collect neighbors from galaxy snapshot
        let mut neighbors: Vec<ID> = Vec::new();
        if let Some(galaxy) = &galaxy_state.galaxy {
            let guard = galaxy
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if let Some(node) = guard.get(&current_planet) {
                neighbors = node.neighbors_snapshot();
            }
        }

        let current_planet_name = galaxy_state
            .planets
            .iter()
            .find(|p| p.id == current_planet)
            .map_or("Unknown", |p| p.name.as_str());

        egui::Area::new(egui::Id::new("explorer_move_selector"))
            .fixed_pos(pos)
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    ui.label(format!(
                        "Move Explorer {explorer_id} from {current_planet_name} to:"
                    ));
                    ui.separator();

                    if neighbors.is_empty() {
                        ui.label("No neighbors available");
                    } else {
                        for nid in &neighbors {
                            let neighbor_name = galaxy_state
                                .planets
                                .iter()
                                .find(|p| p.id == *nid)
                                .map_or("Unknown", |p| p.name.as_str());
                            if ui.button(neighbor_name).clicked() {
                                // log and send move command: Explorer ID, from, to
                                crate::compat::logging::log_internal(
                                    LogTarget::ChannelMessages,
                                    Channel::Info,
                                    payload!(
                                        action : "manual_move_explorer",
                                        explorer_id : explorer_id,
                                        from_planet : current_planet,
                                        to_planet : *nid,
                                    ),
                                );
                                comms.send(UiToOrchestratorCommand::ManualMoveExplorer(
                                    explorer_id,
                                    Some(current_planet),
                                    *nid,
                                ));
                                // request refreshed positions
                                comms.send(UiToOrchestratorCommand::GetExplorersPosition);
                                // clear pending selector
                                ui_state.pending_move_explorer = None;
                                ui_state.pending_move_pos = None;
                            }
                        }
                    }

                    ui.separator();
                    if ui.button("✗ Cancel").clicked() {
                        ui_state.pending_move_explorer = None;
                        ui_state.pending_move_pos = None;
                    }
                });
            });
    } else {
        // Explorer has no known planet: just clear selector
        ui_state.pending_move_explorer = None;
        ui_state.pending_move_pos = None;
    }
}

// ---------------------------------------------------------------------------
// Explorer-limit notice
// ---------------------------------------------------------------------------

pub fn show_explorer_limit_popup(ctx: &egui::Context, ui_state: &mut UiState) {
    // Clone message to avoid borrowing `self` inside closure
    if let Some(msg) = ui_state.explorer_limit_popup.clone() {
        egui::Window::new("Notice")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label(msg);
                if ui.button("OK").clicked() {
                    ui_state.explorer_limit_popup = None;
                }
            });
    }
}

// ---------------------------------------------------------------------------
// Game-over popup
// ---------------------------------------------------------------------------

pub fn show_game_over_popup(ctx: &egui::Context, ui_state: &mut UiState) {
    if let Some(msg) = ui_state.game_over_popup.clone() {
        egui::Window::new("Game Over")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label(msg);
                if ui.button("OK").clicked() {
                    ui_state.game_over_popup = None;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
    }
}

// ---------------------------------------------------------------------------
// Generate-resource popup
// ---------------------------------------------------------------------------

pub fn show_generate_resource_popup(
    ctx: &egui::Context,
    ui_state: &mut UiState,
    comms: &OrchestratorComms,
) {
    let Some(expl_id) = ui_state.pending_generate_explorer else {
        return;
    };

    egui::Area::new(egui::Id::new("generate_resource_menu"))
        .fixed_pos(egui::Pos2::new(100.0, 100.0))
        .show(ctx, |ui| {
            ui.vertical(|ui| {
                ui.label(format!("Generate resource for Explorer {expl_id}:"));
                ui.separator();

                if let Some(res_options) = ui_state.resource_options.as_ref() {
                    if res_options.is_empty() {
                        ui.label("No resources can be generated here.");
                    } else {
                        let mut chosen_resource: Option<BasicResourceType> = None;
                        for opt in res_options {
                            let label = format!("{opt:?}");
                            if ui.button(label).clicked() {
                                chosen_resource = Some(*opt);
                            }
                        }
                        if let Some(res) = chosen_resource {
                            crate::compat::logging::log_internal(
                                LogTarget::General,
                                Channel::Info,
                                payload!(
                                    action : "explorer_generate_resource",
                                    explorer_id : expl_id,
                                    resource : format!("{res:?}"),
                                ),
                            );
                            comms.send(UiToOrchestratorCommand::ExplorerGenerateResource(
                                expl_id, res,
                            ));
                            comms.send(UiToOrchestratorCommand::GetExplorerSnapshot(expl_id));
                            comms.send(UiToOrchestratorCommand::GetExplorersPosition);
                            ui_state.pending_generate_explorer = None;
                            ui_state.resource_options = None;
                        }
                    }
                } else {
                    ui.label("Loading...");
                }

                ui.separator();
                if ui.button("✗ Cancel").clicked() {
                    ui_state.pending_generate_explorer = None;
                    ui_state.resource_options = None;
                }
            });
        });
}

// ---------------------------------------------------------------------------
// Craft-resource popup
// ---------------------------------------------------------------------------

pub fn show_craft_resource_popup(
    ctx: &egui::Context,
    ui_state: &mut UiState,
    comms: &OrchestratorComms,
) {
    let Some(expl_id) = ui_state.pending_craft_explorer else {
        return;
    };

    egui::Area::new(egui::Id::new("craft_resource_menu"))
        .fixed_pos(egui::Pos2::new(100.0, 100.0))
        .show(ctx, |ui| {
            ui.vertical(|ui| {
                ui.label(format!("Craft resource for Explorer {expl_id}:"));
                ui.separator();

                if let Some(comb_options) = ui_state.combination_options.as_ref() {
                    if comb_options.is_empty() {
                        ui.label("No resources can be generated here.");
                    } else {
                        let mut chosen_resource: Option<ComplexResourceType> = None;
                        for opt in comb_options {
                            let label = format!("{opt:?}");
                            if ui.button(label).clicked() {
                                chosen_resource = Some(*opt);
                            }
                        }
                        if let Some(res) = chosen_resource {
                            crate::compat::logging::log_internal(
                                LogTarget::General,
                                Channel::Info,
                                payload!(
                                    action : "explorer_craft_resource",
                                    explorer_id : expl_id,
                                    resource : format!("{res:?}"),
                                ),
                            );
                            comms.send(UiToOrchestratorCommand::ExplorerCombineResource(
                                expl_id, res,
                            ));
                            comms.send(UiToOrchestratorCommand::GetExplorerSnapshot(expl_id));
                            comms.send(UiToOrchestratorCommand::GetExplorersPosition);
                            ui_state.pending_craft_explorer = None;
                            ui_state.combination_options = None;
                        }
                    }
                } else {
                    ui.label("Loading...");
                }

                ui.separator();
                if ui.button("✗ Cancel").clicked() {
                    ui_state.pending_craft_explorer = None;
                    ui_state.combination_options = None;
                }
            });
        });
}
