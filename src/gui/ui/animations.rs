use eframe::egui;

use crate::gui::models::Planet;
use crate::gui::state::AnimationState;

// ---------------------------------------------------------------------------
// Asteroid visual effect
// ---------------------------------------------------------------------------
pub fn draw_asteroid_animation(
    painter: &egui::Painter,
    canvas_rect: egui::Rect,
    animation_state: &mut AnimationState,
    planets: &[Planet],
) {
    let Some((planet_id, start_time)) = animation_state.sending_asteroid else {
        return;
    };

    // Make asteroid effect
    let display_duration = 0.25_f32;
    let elapsed = start_time.elapsed().as_secs_f32();

    if let Some(planet) = planets.iter().find(|p| p.id == planet_id) {
        let origin = canvas_rect.center();

        painter.line_segment(
            [origin, planet.pos],
            egui::Stroke::new(3.0, egui::Color32::from_rgba_unmultiplied(255, 80, 80, 220)),
        );

        // Impact dot
        painter.circle_filled(planet.pos, 6.0, egui::Color32::RED);

        let ring_progress = (elapsed / display_duration).min(1.0);
        let ring_radius = 10.0 + 30.0 * ring_progress;
        #[expect(
            clippy::cast_possible_truncation,
            reason = "value is clamped to [0, 255] before cast"
        )]
        #[expect(
            clippy::cast_sign_loss,
            reason = "value is clamped to [0, 255] so it is always non-negative"
        )]
        let ring_alpha = ((1.0 - ring_progress) * 200.0).clamp(0.0, 255.0) as u8;
        painter.circle_stroke(
            planet.pos,
            ring_radius,
            egui::Stroke::new(
                2.0,
                egui::Color32::from_rgba_unmultiplied(255, 80, 80, ring_alpha),
            ),
        );

        if elapsed >= display_duration {
            animation_state.sending_asteroid = None;
        } else {
            animation_state.sending_asteroid = Some((planet_id, start_time));
        }
    } else {
        animation_state.sending_asteroid = None;
    }
}

// ---------------------------------------------------------------------------
// Sunray visual effect
// ---------------------------------------------------------------------------
pub fn draw_sunray_animation(
    painter: &egui::Painter,
    canvas_rect: egui::Rect,
    animation_state: &mut AnimationState,
    planets: &[Planet],
) {
    let Some((planet_id, start_time)) = animation_state.sending_sunray else {
        return;
    };

    // Make sunray animation
    let display_duration = 0.15_f32;
    let elapsed = start_time.elapsed().as_secs_f32();

    if let Some(planet) = planets.iter().find(|p| p.id == planet_id) {
        let origin = canvas_rect.center();

        painter.line_segment(
            [origin, planet.pos],
            egui::Stroke::new(
                3.0,
                egui::Color32::from_rgba_unmultiplied(255, 230, 120, 220),
            ),
        );

        // Impact dot
        painter.circle_filled(planet.pos, 6.0, egui::Color32::YELLOW);

        let ring_progress = (elapsed / display_duration).min(1.0);
        let ring_radius = 12.0 + 28.0 * ring_progress;
        #[expect(
            clippy::cast_possible_truncation,
            reason = "value is clamped to [0, 255] before cast"
        )]
        #[expect(
            clippy::cast_sign_loss,
            reason = "value is clamped to [0, 255] so it is always non-negative"
        )]
        let ring_alpha = ((1.0 - ring_progress) * 180.0).clamp(0.0, 255.0) as u8;
        painter.circle_stroke(
            planet.pos,
            ring_radius,
            egui::Stroke::new(
                2.0,
                egui::Color32::from_rgba_unmultiplied(255, 230, 120, ring_alpha),
            ),
        );

        if elapsed >= display_duration {
            animation_state.sending_sunray = None;
        } else {
            animation_state.sending_sunray = Some((planet_id, start_time));
        }
    } else {
        animation_state.sending_sunray = None;
    }
}
