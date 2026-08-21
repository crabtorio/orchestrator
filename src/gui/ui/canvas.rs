use common_game::utils::ID;
use eframe::egui;
use image::GenericImageView;
use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use crate::gui::models::Planet;
use crate::gui::state::{AnimationState, ExplorerState, GalaxyState, UiState};

// ---------------------------------------------------------------------------
// Background
// ---------------------------------------------------------------------------

#[expect(
    clippy::cast_possible_truncation,
    reason = "f64 ->f32 narrowing for egui coords is intentional; f64 precision result fits in f32 display range"
)]
#[expect(
    clippy::cast_precision_loss,
    reason = "seed%3 is 0..=2, exactly representable in f32; seed%140 fits in u8"
)]
pub fn draw_background(painter: &egui::Painter, canvas_rect: egui::Rect) {
    // draw space background dark blue
    painter.rect_filled(
        canvas_rect,
        0.0, // Corner rounding
        egui::Color32::from_rgb(8, 10, 20),
    );

    // Subtle nebula glow
    let glow_center = canvas_rect.center();
    let glow_radius = canvas_rect.width().max(canvas_rect.height()) * 0.55;
    painter.circle_filled(
        glow_center,
        glow_radius,
        egui::Color32::from_rgba_unmultiplied(40, 60, 110, 30),
    );

    // Starfield (deterministic, no RNG dependency)
    let mut seed = 0x4A2D_9E1Bu32;
    for _ in 0..120 {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        // Use f64 for the division to avoid precision loss, then narrow to f32 for egui
        let x = canvas_rect.left()
            + (f64::from(seed) / f64::from(u32::MAX)) as f32 * canvas_rect.width();
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let y = canvas_rect.top()
            + (f64::from(seed) / f64::from(u32::MAX)) as f32 * canvas_rect.height();
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        // seed % 140 is always 0..=139, fits in u8; seed % 3 is always 0..=2, fits in f32 exactly
        let alpha = 40 + (seed % 140) as u8;
        let radius = 0.5 + (seed % 3) as f32 * 0.4;
        painter.circle_filled(
            egui::pos2(x, y),
            radius,
            egui::Color32::from_rgba_unmultiplied(200, 220, 255, alpha),
        );
    }
}

// ---------------------------------------------------------------------------
// Edges
// ---------------------------------------------------------------------------

pub fn draw_edges(
    painter: &egui::Painter,
    edges: &[(ID, ID)],
    cached_pos_by_id: &HashMap<ID, egui::Pos2>,
) {
    for (a, b) in edges {
        if let (Some(pa), Some(pb)) = (cached_pos_by_id.get(a), cached_pos_by_id.get(b)) {
            painter.line_segment(
                [*pa, *pb],
                egui::Stroke::new(
                    1.2,
                    egui::Color32::from_rgba_unmultiplied(120, 150, 210, 70),
                ),
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Planets & explorers
// ---------------------------------------------------------------------------

pub fn draw_planets_and_explorers(
    ctx: &egui::Context,
    painter: &egui::Painter,
    response: &egui::Response,
    galaxy_state: &GalaxyState,
    explorer_state: &ExplorerState,
    animation_state: &mut AnimationState,
    ui_state: &mut UiState,
) {
    for planet in &galaxy_state.planets {
        draw_single_planet(
            ctx,
            painter,
            response,
            planet,
            &galaxy_state.planet_states,
            explorer_state,
            animation_state,
            ui_state,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_single_planet(
    ctx: &egui::Context,
    painter: &egui::Painter,
    response: &egui::Response,
    planet: &Planet,
    planet_states: &std::collections::HashMap<
        ID,
        common_game::components::planet::DummyPlanetState,
    >,
    explorer_state: &ExplorerState,
    animation_state: &mut AnimationState,
    ui_state: &mut UiState,
) {
    // size
    let radius = 20.0;

    // Check for right-click on this planet
    if let Some(pointer_pos) = response.interact_pointer_pos() {
        let distance = pointer_pos.distance(planet.pos);
        if distance <= radius && response.clicked_by(egui::PointerButton::Secondary) {
            ui_state.selected_planet = Some(planet.id);
            ui_state.context_menu_pos = Some(pointer_pos);
        }
    }

    // Draw colored shadow if planet is selected
    if ui_state.selected_planet == Some(planet.id) {
        painter.circle_filled(
            planet.pos,
            radius + 7.0,
            egui::Color32::from_rgba_unmultiplied(140, 180, 255, 120), // neon glow
        );
    }

    if planet.active {
        let base = planet_base_color(planet.id);
        painter.circle_filled(
            planet.pos,
            radius + 4.0,
            egui::Color32::from_rgba_unmultiplied(base.r(), base.g(), base.b(), 40),
        );
        painter.circle_filled(planet.pos, radius, base);
        painter.circle_filled(
            planet.pos + egui::vec2(-4.0, -4.0),
            radius * 0.4,
            egui::Color32::from_rgba_unmultiplied(255, 255, 255, 50),
        );

        if let Some(texture) = get_planet_texture(ctx, ui_state, planet.id, &planet.name) {
            let size = egui::Vec2::splat(radius * 2.4);
            let rect = egui::Rect::from_center_size(planet.pos, size);
            let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
            painter.image(texture.id(), rect, uv, egui::Color32::WHITE);
        } else if let Some(err) = ui_state.planet_icon_errors.get(&planet_icon_key(planet.id)) {
            painter.text(
                planet.pos + egui::Vec2::new(0.0, radius + 26.0),
                egui::Align2::CENTER_TOP,
                format!("Icon missing for {}: {err}", planet.name),
                egui::FontId::proportional(10.0),
                egui::Color32::from_rgb(255, 120, 120),
            );
        }
    }

    // draw explorers on this planet as small colored dots
    draw_explorers_on_planet(
        ctx,
        painter,
        response,
        planet,
        radius,
        explorer_state,
        ui_state,
    );

    // draw name label centered below planet
    painter.text(
        planet.pos + egui::Vec2::new(0.0, radius + 10.0),
        egui::Align2::CENTER_TOP,
        &planet.name,
        egui::FontId::proportional(14.0),
        egui::Color32::from_rgb(210, 230, 255),
    );

    // Draw planet state if available
    draw_planet_state(ctx, painter, planet, radius, planet_states, animation_state);
}

// ---------------------------------------------------------------------------
// Explorers on a single planet
// ---------------------------------------------------------------------------

#[expect(
    clippy::cast_precision_loss,
    reason = "usize index/length cast to f32 for screen-space angle; max 2 explorers so no precision issue"
)]
fn draw_explorers_on_planet(
    ctx: &egui::Context,
    painter: &egui::Painter,
    response: &egui::Response,
    planet: &Planet,
    planet_radius: f32,
    explorer_state: &ExplorerState,
    ui_state: &mut UiState,
) {
    let explorers_on_planet: Vec<ID> = explorer_state
        .explorer_positions
        .iter()
        .filter(|(_, pid)| **pid == planet.id)
        .map(|(eid, _)| *eid)
        .collect();

    for (i, explorer_id) in explorers_on_planet.iter().enumerate() {
        let angle = (i as f32 / explorers_on_planet.len().max(1) as f32) * std::f32::consts::TAU;
        let explorer_pos = planet.pos
            + egui::Vec2::new(
                (planet_radius + 12.0) * angle.cos(),
                (planet_radius + 12.0) * angle.sin(),
            );
        // Explorer visual parameters
        let explorer_radius = 8.0; // made larger per request

        // Check for right-click on explorer
        if let Some(pointer_pos) = response.interact_pointer_pos() {
            let distance = pointer_pos.distance(explorer_pos);
            if distance <= explorer_radius && response.clicked_by(egui::PointerButton::Secondary) {
                ui_state.selected_explorer = Some(*explorer_id);
                ui_state.selected_planet = None;
                ui_state.context_menu_pos = Some(pointer_pos);
            }
        }

        if let Some(texture) = get_explorer_texture(ctx, ui_state, *explorer_id) {
            let size = egui::Vec2::splat(explorer_radius * 2.2);
            let rect = egui::Rect::from_center_size(explorer_pos, size);
            let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
            painter.image(texture.id(), rect, uv, egui::Color32::WHITE);
        } else if let Some(err) = ui_state
            .explorer_icon_errors
            .get(&explorer_icon_key(*explorer_id))
        {
            painter.text(
                explorer_pos + egui::Vec2::new(0.0, explorer_radius + 8.0),
                egui::Align2::CENTER_TOP,
                format!("Icon missing: {err}"),
                egui::FontId::proportional(9.0),
                egui::Color32::from_rgb(255, 120, 120),
            );
        }

        // Draw explorer label with name and ID (resources are shown in the HUD panels)
        let explorer_name = crate::compat::id::IdManager::explorer_name_from_id(*explorer_id);
        let display_name = crate::gui::helpers::display_explorer_name(explorer_name);
        let label_text = format!("{display_name} ({explorer_id})");
        painter.text(
            explorer_pos + egui::Vec2::new(explorer_radius + 6.0, -explorer_radius - 6.0),
            egui::Align2::LEFT_TOP,
            label_text,
            egui::FontId::proportional(11.0),
            egui::Color32::from_rgb(210, 230, 255),
        );
    }
}

fn planet_base_color(planet_id: ID) -> egui::Color32 {
    let palette = [
        egui::Color32::from_rgb(88, 162, 255),
        egui::Color32::from_rgb(115, 210, 255),
        egui::Color32::from_rgb(120, 130, 255),
        egui::Color32::from_rgb(160, 120, 255),
        egui::Color32::from_rgb(90, 200, 200),
        egui::Color32::from_rgb(130, 240, 200),
    ];
    let idx = (planet_id as usize) % palette.len();
    palette[idx]
}

fn get_explorer_texture(
    ctx: &egui::Context,
    ui_state: &mut UiState,
    explorer_id: ID,
) -> Option<egui::TextureHandle> {
    let key = explorer_icon_key(explorer_id);

    if let Some(texture) = ui_state.explorer_textures.get(&key) {
        return Some(texture.clone());
    }

    if ui_state.explorer_icon_errors.contains_key(&key) {
        return None;
    }

    let path = ui_state
        .explorer_icon_paths
        .get(&key)
        .cloned()
        .or_else(|| fallback_explorer_icon_path(&key));

    let Some(path) = path else {
        ui_state
            .explorer_icon_errors
            .insert(key, "No icon mapping".to_owned());
        return None;
    };

    let texture_name = format!("explorer_{key}");
    match load_texture_from_path(ctx, &path, &texture_name) {
        Ok(texture) => {
            ui_state.explorer_textures.insert(key, texture.clone());
            Some(texture)
        }
        Err(err) => {
            ui_state
                .explorer_icon_errors
                .insert(key, format!("{path}: {err}"));
            None
        }
    }
}

fn explorer_icon_key(explorer_id: ID) -> String {
    let name = crate::compat::id::IdManager::explorer_name_from_id(explorer_id);
    explorer_icon_key_from_name(name)
}

fn explorer_icon_key_from_name(name: &str) -> String {
    let lower = name.to_lowercase();
    if lower.contains("vojager") {
        "Vojager".to_owned()
    } else if lower.contains("nomad") {
        "Nomad".to_owned()
    } else {
        "Nico Explorer".to_owned()
    }
}

fn fallback_explorer_icon_path(explorer_key: &str) -> Option<String> {
    let normalized: String = explorer_key
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    let normalized = normalized.trim_matches('_').to_owned();

    if !normalized.is_empty() {
        return Some(format!("assets/explorers/{normalized}.png"));
    }

    None
}

fn get_planet_texture(
    ctx: &egui::Context,
    ui_state: &mut UiState,
    planet_id: ID,
    planet_name: &str,
) -> Option<egui::TextureHandle> {
    let key = planet_icon_key(planet_id);

    if let Some(texture) = ui_state.planet_textures.get(&key) {
        return Some(texture.clone());
    }

    if ui_state.planet_icon_errors.contains_key(&key) {
        return None;
    }

    let path = ui_state
        .planet_icon_paths
        .get(&key)
        .cloned()
        .or_else(|| fallback_icon_path(&key));

    let Some(path) = path else {
        ui_state
            .planet_icon_errors
            .insert(key, format!("No icon mapping for '{planet_name}'"));
        return None;
    };

    match load_texture_from_path(ctx, &path, &key) {
        Ok(texture) => {
            ui_state.planet_textures.insert(key, texture.clone());
            Some(texture)
        }
        Err(err) => {
            ui_state
                .planet_icon_errors
                .insert(key, format!("{path}: {err}"));
            None
        }
    }
}

fn planet_icon_key(planet_id: ID) -> String {
    let kind = crate::compat::id::IdManager::planet_kind(planet_id);
    let label = match kind {
        crate::compat::id::PlanetKind::RustyCrab => "Rusty Crab",
        crate::compat::id::PlanetKind::Rustrelli => "Rustrelli",
        crate::compat::id::PlanetKind::Orbitron => "Orbitron",
        crate::compat::id::PlanetKind::Houston => "Houston",
        crate::compat::id::PlanetKind::Trip => "Trip",
        crate::compat::id::PlanetKind::Luna4 => "Luna4",
        crate::compat::id::PlanetKind::Enterprise => "Enterprise",
    };
    label.to_owned()
}

fn fallback_icon_path(planet_name: &str) -> Option<String> {
    let normalized: String = planet_name
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    let normalized = normalized.trim_matches('_').to_owned();

    if !normalized.is_empty() {
        return Some(format!("assets/planets/{normalized}.png"));
    }

    None
}

fn load_texture_from_path(
    ctx: &egui::Context,
    path: &str,
    name: &str,
) -> Result<egui::TextureHandle, String> {
    let resolved = resolve_asset_path(path);
    let image = image::open(&resolved).map_err(|err| err.to_string())?;
    let (width, height) = image.dimensions();
    let rgba = image.to_rgba8();
    let size = [width as usize, height as usize];
    let pixels = rgba.into_raw();
    let color_image = egui::ColorImage::from_rgba_unmultiplied(size, &pixels);

    Ok(ctx.load_texture(
        format!("planet_icon_{name}"),
        color_image,
        egui::TextureOptions::LINEAR,
    ))
}

fn resolve_asset_path(path: &str) -> PathBuf {
    let candidate = Path::new(path);
    if candidate.is_absolute() {
        return candidate.to_path_buf();
    }

    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest_dir.join(candidate)
}

// ---------------------------------------------------------------------------
// Planet state overlay (energy cells, rocket icon, …)
// ---------------------------------------------------------------------------

#[expect(
    clippy::cast_precision_loss,
    reason = "charged_cells_count cast to f32 for smooth animation; value is always small"
)]
#[expect(
    clippy::cast_sign_loss,
    reason = "displayed is kept >= 0 by the animation clamp logic; .round() is always non-negative"
)]
#[expect(
    clippy::cast_possible_truncation,
    reason = "displayed counter is bounded by energy_cells.len() which is small"
)]
fn draw_planet_state(
    ctx: &egui::Context,
    painter: &egui::Painter,
    planet: &Planet,
    radius: f32,
    planet_states: &std::collections::HashMap<
        ID,
        common_game::components::planet::DummyPlanetState,
    >,
    animation_state: &mut AnimationState,
) {
    if let Some(state) = planet_states.get(&planet.id) {
        let state_pos = planet.pos + egui::Vec2::new(0.0, radius + 28.0);

        // Animate displayed charged counter towards actual value
        let dt = ctx.input(|i| i.stable_dt);
        let target_charged = state.charged_cells_count as f32;
        let displayed = animation_state
            .planet_displayed_charged
            .entry(planet.id)
            .or_insert(target_charged);
        let speed_per_sec = 20.0; // units per second
        let step = speed_per_sec * dt;

        // Animate in BOTH directions (up and down)
        if (*displayed - target_charged).abs() > 0.01 {
            if *displayed < target_charged {
                *displayed = (*displayed + step).min(target_charged);
            } else {
                *displayed = (*displayed - step).max(target_charged);
            }
        }

        // use the animated value when building text
        let displayed_charged_usize = (*displayed).round() as usize;

        // Build state text with emojis (animated value)
        let mut state_text = String::new();
        if state.has_rocket {
            state_text.push_str("🚀 ");
        }
        let _ = write!(
            state_text,
            "⚡{}/{}",
            displayed_charged_usize,
            state.energy_cells.len()
        );

        // Draw state text - use simpler rendering without extra layout
        painter.text(
            state_pos,
            egui::Align2::CENTER_TOP,
            state_text,
            egui::FontId::proportional(12.0),
            egui::Color32::WHITE,
        );
    }
}

// ---------------------------------------------------------------------------
// Help text
// ---------------------------------------------------------------------------

pub fn draw_help_text(painter: &egui::Painter, canvas_rect: egui::Rect) {
    painter.text(
        canvas_rect.min + egui::Vec2::new(20.0, 20.0),
        egui::Align2::LEFT_TOP,
        "Right-click an entity for options. Use the top-right button to create a planet.",
        egui::FontId::monospace(14.0),
        egui::Color32::YELLOW,
    );
}
