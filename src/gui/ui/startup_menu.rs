use eframe::egui;
use crate::compat::ExplorerType;

use crate::gui::state::StartupState;

pub struct StartupMenuResult {
    pub start_requested: bool,
}

#[allow(clippy::too_many_lines)]
pub fn show_startup_menu(ctx: &egui::Context, state: &mut StartupState) -> StartupMenuResult {
    let mut start_requested = false;

    egui::CentralPanel::default().show(ctx, |ui| {
        let available = ui.available_rect_before_wrap();
        let panel_width = available.width().clamp(420.0, 720.0);
        let panel_height = available.height().min(820.0);
        let panel_rect =
            egui::Rect::from_center_size(available.center(), egui::vec2(panel_width, panel_height));

        ui.scope_builder(egui::UiBuilder::new().max_rect(panel_rect), |ui| {
            ui.set_width(panel_width);

            ui.vertical_centered(|ui| {
                ui.heading("Start Game");
                ui.label("Configure your crew, pace, and galaxy");
            });

            ui.add_space(16.0);

            egui::Frame::group(ui.style())
                .inner_margin(egui::Margin::same(14))
                .show(ui, |ui| {
                    ui.label(egui::RichText::new("Explorer selection").strong());
                    ui.add_space(6.0);

                    ui.columns(2, |cols| {
                        explorer_combo(
                            &mut cols[0],
                            "Explorer slot 1 (required)",
                            &mut state.explorer_slot_one,
                            false,
                        );
                        explorer_combo(
                            &mut cols[1],
                            "Explorer slot 2 (optional)",
                            &mut state.explorer_slot_two,
                            true,
                        );
                    });
                });

            ui.add_space(12.0);

            egui::Frame::group(ui.style())
                .inner_margin(egui::Margin::same(14))
                .show(ui, |ui| {
                    ui.label(egui::RichText::new("Game step").strong());
                    ui.add_space(6.0);
                    ui.add_sized(
                        [panel_width - 40.0, 0.0],
                        egui::Slider::new(&mut state.game_step_ms, 500..=10000)
                            .text("Step (ms)")
                            .clamping(egui::SliderClamping::Always),
                    );
                });

            ui.add_space(12.0);

            egui::Frame::group(ui.style())
                .inner_margin(egui::Margin::same(14))
                .show(ui, |ui| {
                    ui.label(egui::RichText::new("Galaxy file").strong());
                    ui.add_space(6.0);

                    ui.horizontal(|ui| {
                        ui.label("Path:");
                        ui.add_sized(
                            [panel_width - 220.0, 0.0],
                            egui::TextEdit::singleline(&mut state.galaxy_path),
                        );
                        if ui.button("Load").clicked() {
                            let mut dialog = rfd::FileDialog::new()
                                .add_filter("Text Files", &["txt"])
                                .add_filter("All Files", &["*"]);

                            if let Some(parent) = std::path::Path::new(&state.galaxy_path).parent()
                                && parent.exists()
                            {
                                dialog = dialog.set_directory(parent);
                            }

                            if let Some(path) = dialog.pick_file() {
                                state.galaxy_path = path.to_string_lossy().to_string();
                                state.load_galaxy_file();
                            }
                        }
                        if ui.button("Save").clicked() {
                            state.save_galaxy_file();
                        }
                    });

                    ui.add_space(8.0);

                    let editor = egui::TextEdit::multiline(&mut state.galaxy_contents)
                        .desired_rows(14)
                        .lock_focus(true)
                        .code_editor();
                    if ui.add(editor).changed() {
                        state.galaxy_dirty = true;
                    }

                    ui.add_space(6.0);

                    if state.galaxy_dirty {
                        ui.label("Unsaved changes");
                    }

                    if let Some(msg) = &state.last_file_status {
                        ui.label(msg);
                    }
                    if let Some(err) = &state.last_file_error {
                        ui.colored_label(egui::Color32::RED, err);
                    }
                });

            ui.add_space(16.0);

            let can_start = state.explorer_slot_one.is_some();
            ui.vertical_centered(|ui| {
                if ui
                    .add_enabled(
                        can_start,
                        egui::Button::new("Start Game").min_size(egui::vec2(180.0, 36.0)),
                    )
                    .clicked()
                {
                    start_requested = true;
                }
                if !can_start {
                    ui.add_space(6.0);
                    ui.label("Select at least one explorer to start.");
                }
            });
        });
    });

    StartupMenuResult { start_requested }
}

fn explorer_combo(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut Option<ExplorerType>,
    allow_none: bool,
) {
    let selected_text = match value {
        Some(ExplorerType::Explorer) => "Nico Explorer",
        Some(ExplorerType::Vojager) => "Rob Vojager",
        Some(ExplorerType::Nomad) => "Jaco Nomad",
        None => "None",
    };

    egui::ComboBox::from_label(label)
        .selected_text(selected_text)
        .show_ui(ui, |ui| {
            if allow_none && ui.selectable_label(value.is_none(), "None").clicked() {
                *value = None;
            }

            if ui
                .selectable_label(
                    matches!(value, Some(ExplorerType::Explorer)),
                    "Nico Explorer",
                )
                .clicked()
            {
                *value = Some(ExplorerType::Explorer);
            }
            if ui
                .selectable_label(matches!(value, Some(ExplorerType::Vojager)), "Rob Vojager")
                .clicked()
            {
                *value = Some(ExplorerType::Vojager);
            }
            if ui
                .selectable_label(matches!(value, Some(ExplorerType::Nomad)), "Jaco Nomad")
                .clicked()
            {
                *value = Some(ExplorerType::Nomad);
            }
        });
}
