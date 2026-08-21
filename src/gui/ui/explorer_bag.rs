use common_game::utils::ID;
use eframe::egui;

use crate::gui::state::ExplorerState;

// ---------------------------------------------------------------------------
// Explorer resource panels (bottom corners)
// ---------------------------------------------------------------------------

/// Draw up to 2 small resource-counter panels, one per explorer, anchored to
/// the bottom corners of the canvas rect.
/// The resources are removed if their counter is zero.
pub fn show_explorer_bags(
    ctx: &egui::Context,
    explorer_state: &ExplorerState,
    canvas_rect: egui::Rect,
) {
    // Panel visual constants
    const PANEL_WIDTH: f32 = 210.0;
    const MARGIN: f32 = 12.0;

    // Collect and sort explorer IDs for a stable ordering between frames.
    let mut explorer_ids: Vec<ID> = explorer_state.explorer_positions.keys().copied().collect();
    explorer_ids.sort_unstable();

    for (slot, &explorer_id) in explorer_ids.iter().take(2).enumerate() {
        // Decide which corner this slot uses
        let anchor = if slot == 0 {
            // bottom-left: we'll pin the top-left of the panel
            egui::Align2::LEFT_BOTTOM
        } else {
            // bottom-right: we'll pin the top-right of the panel
            egui::Align2::RIGHT_BOTTOM
        };

        let fixed_pos = if slot == 0 {
            egui::pos2(canvas_rect.left() + MARGIN, canvas_rect.bottom() - MARGIN)
        } else {
            egui::pos2(canvas_rect.right() - MARGIN, canvas_rect.bottom() - MARGIN)
        };

        let explorer_name = crate::compat::id::IdManager::explorer_name_from_id(explorer_id);
        let display_name = crate::gui::helpers::display_explorer_name(explorer_name);
        let title = format!("{display_name} ({explorer_id})");

        // Clone bag data needed for rendering (avoids holding borrow inside closure)
        let bag_entries: Vec<(String, u64)> = match explorer_state.explorer_bags.get(&explorer_id) {
            Some(bag) if !bag.resources_amounts.is_empty() => {
                let mut entries: Vec<(String, u64)> = bag
                    .resources_amounts
                    .iter()
                    .filter(|(_, v)| **v > 0)
                    .map(|(k, v)| (format!("{k:?}"), *v))
                    .collect();
                entries.sort_by(|a, b| a.0.cmp(&b.0));
                entries
            }
            _ => vec![],
        };

        let area_id = egui::Id::new(format!("explorer_hud_{slot}"));

        egui::Area::new(area_id)
            .anchor(anchor, egui::vec2(0.0, 0.0))
            .fixed_pos(fixed_pos)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                ui.set_max_width(PANEL_WIDTH);

                egui::Frame::new()
                    .fill(egui::Color32::from_rgba_unmultiplied(12, 16, 30, 220))
                    .stroke(egui::Stroke::new(
                        1.0,
                        egui::Color32::from_rgba_unmultiplied(70, 100, 180, 180),
                    ))
                    .corner_radius(egui::CornerRadius::same(8))
                    .inner_margin(egui::Margin::symmetric(10, 8))
                    .show(ui, |ui| {
                        ui.set_max_width(PANEL_WIDTH - 24.0); // account for inner margin

                        // Header
                        ui.label(
                            egui::RichText::new(&title)
                                .size(12.0)
                                .color(egui::Color32::from_rgb(150, 190, 255))
                                .strong(),
                        );

                        ui.add(egui::Separator::default().spacing(4.0));

                        // Resource rows
                        if bag_entries.is_empty() {
                            ui.label(
                                egui::RichText::new("bag: empty")
                                    .size(11.5)
                                    .color(egui::Color32::from_rgb(120, 130, 150))
                                    .italics(),
                            );
                        } else {
                            for (resource_name, amount) in &bag_entries {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        egui::RichText::new(format!("{resource_name}:"))
                                            .size(11.5)
                                            .color(egui::Color32::from_rgb(190, 210, 255)),
                                    );
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            ui.label(
                                                egui::RichText::new(format!("{amount}"))
                                                    .size(11.5)
                                                    .color(egui::Color32::from_rgb(255, 220, 120))
                                                    .strong(),
                                            );
                                        },
                                    );
                                });
                            }
                        }
                    });
            });
    }
}
