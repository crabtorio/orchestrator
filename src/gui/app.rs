use common_game::logging::Channel;
use eframe::egui;
use crate::compat::ExplorerType;
use crate::compat::logging::LogTarget;
use crate::compat::payload;
use crate::compat::ui::UiToOrchestratorCommand;
use std::time::{Duration, Instant};

use crate::gui::comms::OrchestratorComms;
use crate::gui::state::{
    AnimationState, ExplorerState, GalaxyState, PollingTimers, StartupState, UiState,
};
use crate::gui::ui;
use crate::gui::update_handler;

struct StartConfig {
    explorer_one: ExplorerType,
    explorer_two: Option<ExplorerType>,
    game_step_ms: u64,
    galaxy_path: String,
}

struct GameRuntime {
    galaxy_state: GalaxyState,
    explorer_state: ExplorerState,
    ui_state: UiState,
    animation_state: AnimationState,
    timers: PollingTimers,
    comms: OrchestratorComms,
    end_game_requested: bool,
    end_game_timestamp: Option<Instant>,
    ended: bool,
}

/// Main application struct
pub struct GalaxyApp {
    startup_state: StartupState,
    runtime: Option<GameRuntime>,
    theme_applied: bool,
}

impl GalaxyApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let mut startup_state = StartupState::new();
        startup_state.load_galaxy_file();

        Self {
            startup_state,
            runtime: None,
            theme_applied: false,
        }
    }

    fn apply_theme(ctx: &egui::Context) {
        let mut style = (*ctx.style()).clone();
        style.spacing.window_margin = egui::Margin::same(12);
        style.spacing.item_spacing = egui::vec2(10.0, 8.0);
        style.spacing.button_padding = egui::vec2(10.0, 6.0);

        style
            .text_styles
            .insert(egui::TextStyle::Heading, egui::FontId::proportional(22.0));
        style
            .text_styles
            .insert(egui::TextStyle::Body, egui::FontId::proportional(15.0));
        style
            .text_styles
            .insert(egui::TextStyle::Button, egui::FontId::proportional(15.0));
        style
            .text_styles
            .insert(egui::TextStyle::Small, egui::FontId::proportional(12.0));

        let mut visuals = egui::Visuals::dark();
        visuals.extreme_bg_color = egui::Color32::from_rgb(8, 10, 20);
        visuals.panel_fill = egui::Color32::from_rgb(14, 18, 32);
        visuals.faint_bg_color = egui::Color32::from_rgb(22, 28, 46);
        visuals.window_fill = egui::Color32::from_rgb(16, 20, 36);
        visuals.window_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(46, 60, 96));

        visuals.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(16, 20, 36);
        visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(24, 30, 50);
        visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(34, 48, 78);
        visuals.widgets.active.bg_fill = egui::Color32::from_rgb(44, 70, 120);
        visuals.widgets.inactive.bg_stroke =
            egui::Stroke::new(1.0, egui::Color32::from_rgb(52, 70, 110));
        visuals.widgets.hovered.bg_stroke =
            egui::Stroke::new(1.0, egui::Color32::from_rgb(80, 110, 180));
        visuals.widgets.active.bg_stroke =
            egui::Stroke::new(1.0, egui::Color32::from_rgb(120, 170, 255));

        visuals.selection.bg_fill = egui::Color32::from_rgb(82, 140, 255);
        visuals.selection.stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(140, 200, 255));
        visuals.hyperlink_color = egui::Color32::from_rgb(120, 200, 255);

        style.visuals = visuals;
        ctx.set_style(style);
    }

    fn try_start_game(&mut self) {
        if self.startup_state.explorer_slot_one.is_none() {
            return;
        }

        if self.startup_state.galaxy_dirty && !self.startup_state.save_galaxy_file() {
            return;
        }

        let Some(explorer_one) = self.startup_state.explorer_slot_one else {
            return;
        };

        let config = StartConfig {
            explorer_one,
            explorer_two: self.startup_state.explorer_slot_two,
            game_step_ms: u64::from(self.startup_state.game_step_ms),
            galaxy_path: self.startup_state.galaxy_path.clone(),
        };

        self.start_game(&config);
    }

    fn start_game(&mut self, config: &StartConfig) {
        let (mut orch, cmd_sender, update_receiver) = crate::compat::create_with_path(
            &config.galaxy_path,
            config.explorer_one,
            config.explorer_two,
            None,
            config.game_step_ms,
        );

        cmd_sender
            .send(UiToOrchestratorCommand::GetGalaxy)
            .expect("Failed to send initial GetGalaxy command");

        cmd_sender
            .send(UiToOrchestratorCommand::GetExplorersPosition)
            .expect("Failed to send initial GetExplorerPosition command");

        std::thread::spawn(move || {
            orch.run();
            crate::compat::logging::log_internal(
                LogTarget::General,
                Channel::Info,
                payload!(
                    message : "Orchestrator created",
                ),
            );

            let tick = std::time::Duration::from_millis(16);
            loop {
                std::thread::sleep(tick);
            }
        });

        self.runtime = Some(GameRuntime {
            galaxy_state: GalaxyState::new(),
            explorer_state: ExplorerState::new(),
            ui_state: UiState::new(),
            animation_state: AnimationState::new(),
            timers: PollingTimers::new(config.game_step_ms),
            comms: OrchestratorComms::new(cmd_sender, update_receiver),
            end_game_requested: false,
            end_game_timestamp: None,
            ended: false,
        });
    }
}

/// `eframe::App`, trait implementation. The main loop
impl eframe::App for GalaxyApp {
    #[allow(clippy::too_many_lines)]
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Apply theme once at startup
        if !self.theme_applied {
            Self::apply_theme(ctx);
            self.theme_applied = true;
        }

        if self.runtime.is_none() {
            if ui::startup_menu::show_startup_menu(ctx, &mut self.startup_state).start_requested {
                self.try_start_game();
            }
            return;
        }

        let runtime = self.runtime.as_mut().expect("Game runtime missing");

        // Handle window close with X button
        if ctx.input(|i| i.viewport().close_requested()) && runtime.end_game_timestamp.is_none() {
            runtime.comms.send_expect(
                UiToOrchestratorCommand::EndGame,
                "Failed to send EndGame command",
            );
            runtime.end_game_requested = true;
            runtime.end_game_timestamp = Some(Instant::now());
            runtime.ui_state.game_over_popup = Some("Shutting down".to_owned());
        }

        // Top control bar
        ui::top_panel::show_top_panel(
            ctx,
            &mut runtime.ui_state,
            &runtime.comms,
            &mut runtime.end_game_timestamp,
            &mut runtime.ended,
        );

        // Timer based polling, not every frame to avoid too much overhead
        if runtime.timers.should_poll_planet_snapshots() && !runtime.ended {
            for planet in &runtime.galaxy_state.planets {
                if planet.active {
                    runtime
                        .comms
                        .send(UiToOrchestratorCommand::GetPlanetSnapshot(planet.id));
                }
            }
        }
        if runtime.timers.should_poll_explorer_snapshots() && !runtime.ended {
            for explorer_id in runtime.explorer_state.explorer_positions.keys() {
                runtime
                    .comms
                    .send(UiToOrchestratorCommand::GetExplorerSnapshot(*explorer_id));
            }
        }
        if runtime.timers.should_poll_explorer_positions() && !runtime.ended {
            runtime
                .comms
                .send(UiToOrchestratorCommand::GetExplorersPosition);
        }

        // Drain expired planet refresh requests
        {
            let refresh_delay = std::time::Duration::from_millis(100);
            let mut i = 0;
            while i < runtime.animation_state.planets_to_refresh.len() {
                let (planet_id, queued_at) = runtime.animation_state.planets_to_refresh[i];
                if queued_at.elapsed() >= refresh_delay {
                    runtime.comms.send(
                        UiToOrchestratorCommand::GetPlanetSnapshot(planet_id),
                    );
                    runtime.animation_state.planets_to_refresh.swap_remove(i);
                } else {
                    i += 1;
                }
            }
        }

        // Main canvas panel
        egui::CentralPanel::default().show(ctx, |ui| {
            // Process all pending orchestrator messages
            update_handler::handle_orchestrator_updates(
                &runtime.comms,
                &mut runtime.galaxy_state,
                &mut runtime.explorer_state,
                &mut runtime.animation_state,
                &mut runtime.ui_state,
                &mut runtime.end_game_requested,
                &mut runtime.ended,
            );

            // Rebuild galaxy layout if the data changed
            let canvas_rect = ui.available_rect_before_wrap();
            let layout_center = ctx.content_rect().center();
            runtime
                .galaxy_state
                .rebuild_if_needed(canvas_rect, layout_center);

            // Allocate painter
            let (response, painter) = ui.allocate_painter(canvas_rect.size(), egui::Sense::click());

            // Draw space background
            ui::canvas::draw_background(&painter, canvas_rect);

            // Handle spawn-planet menus
            ui::menus::handle_spawn_menus(
                ctx,
                &mut runtime.ui_state,
                &runtime.galaxy_state.planets,
                &runtime.comms,
            );

            // Handle explorer move-to-planet selector
            ui::popups::show_move_selector(
                ctx,
                &mut runtime.ui_state,
                &runtime.explorer_state,
                &runtime.galaxy_state,
                &runtime.comms,
            );

            // Draw planet edges
            runtime.galaxy_state.refresh_pos_cache();
            ui::canvas::draw_edges(
                &painter,
                &runtime.galaxy_state.edges,
                &runtime.galaxy_state.cached_pos_by_id,
            );

            // Draw planets & explorers
            ui::canvas::draw_planets_and_explorers(
                ctx,
                &painter,
                &response,
                &runtime.galaxy_state,
                &runtime.explorer_state,
                &mut runtime.animation_state,
                &mut runtime.ui_state,
            );

            // Show planet context menu
            let maybe_planet = runtime.ui_state.selected_planet;
            let maybe_pos = runtime.ui_state.context_menu_pos;
            if let (Some(planet_id), Some(menu_pos)) = (maybe_planet, maybe_pos) {
                // Only show planet menu if no explorer is selected
                if runtime.ui_state.selected_explorer.is_none() {
                    ui::menus::show_context_menu(
                        ctx,
                        menu_pos,
                        planet_id,
                        &mut runtime.ui_state,
                        &runtime.explorer_state,
                        &mut runtime.animation_state,
                        &runtime.comms,
                    );
                }
            }

            // Show explorer context menu
            let maybe_explorer = runtime.ui_state.selected_explorer;
            let maybe_pos = runtime.ui_state.context_menu_pos;
            if let (Some(explorer_id), Some(menu_pos)) = (maybe_explorer, maybe_pos) {
                ui::menus::show_explorer_menu(
                    ctx,
                    menu_pos,
                    explorer_id,
                    &mut runtime.ui_state,
                    &runtime.comms,
                );
            }

            // Show explorer limit popup
            ui::popups::show_explorer_limit_popup(ctx, &mut runtime.ui_state);

            // Show game over popup
            ui::popups::show_game_over_popup(ctx, &mut runtime.ui_state);

            // Show generate resource popup
            ui::popups::show_generate_resource_popup(ctx, &mut runtime.ui_state, &runtime.comms);

            // Show craft resource popup
            ui::popups::show_craft_resource_popup(ctx, &mut runtime.ui_state, &runtime.comms);

            // Draw asteroid animation
            ui::animations::draw_asteroid_animation(
                &painter,
                canvas_rect,
                &mut runtime.animation_state,
                &runtime.galaxy_state.planets,
            );

            // Draw sunray animation
            ui::animations::draw_sunray_animation(
                &painter,
                canvas_rect,
                &mut runtime.animation_state,
                &runtime.galaxy_state.planets,
            );

            // Draw explorer bags panels (bottom corners)
            ui::explorer_bag::show_explorer_bags(ctx, &runtime.explorer_state, canvas_rect);

            // Draw instructions for the user
            ui::canvas::draw_help_text(&painter, canvas_rect);
        });

        // Close window gracefully after EndGame
        if let Some(shutdown_time) = runtime.end_game_timestamp {
            // Wait 2 seconds before closing to let the popup display
            if shutdown_time.elapsed() >= Duration::from_secs(2) {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            } else {
                // Keep repainting to show the popup during shutdown
                ctx.request_repaint_after(Duration::from_millis(100));
            }
        }

        // Schedule next animation repaint
        if runtime.animation_state.has_active_animations() {
            ctx.request_repaint_after(std::time::Duration::from_millis(33));
        } else {
            // Low-frequency repaint so polling timers keep firing and
            // orchestrator messages get drained even when the user is not
            // interacting
            let next_poll = runtime.timers.time_until_next_poll();
            ctx.request_repaint_after(next_poll + std::time::Duration::from_millis(50));
        }
    }
}
