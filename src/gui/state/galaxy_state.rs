use common_game::components::planet::DummyPlanetState;
use common_game::utils::ID;
use eframe::egui;
use crate::compat::planet::PlanetMap;
use std::collections::{HashMap, HashSet};

use crate::gui::helpers::build_planets_and_edges_from_galaxy;
use crate::gui::models::Planet;

pub struct GalaxyState {
    pub galaxy: Option<PlanetMap>,
    pub planets: Vec<Planet>,
    pub edges: Vec<(ID, ID)>,
    pub planet_states: HashMap<ID, DummyPlanetState>,
    pub galaxy_needs_rebuild: bool,
    pub cached_pos_by_id: HashMap<ID, egui::Pos2>,
    pub last_layout_center: Option<egui::Pos2>,
    pub last_canvas_size: Option<egui::Vec2>,
}

impl GalaxyState {
    pub fn new() -> Self {
        Self {
            galaxy: None,
            planets: Vec::new(),
            edges: Vec::new(),
            planet_states: HashMap::new(),
            galaxy_needs_rebuild: true,
            cached_pos_by_id: HashMap::new(),
            last_layout_center: None,
            last_canvas_size: None,
        }
    }

    /// Rebuild the circular layout from the galaxy snapshot (only when flagged dirty).
    pub fn rebuild_if_needed(&mut self, canvas_rect: egui::Rect, layout_center: egui::Pos2) {
        let canvas_size = canvas_rect.size();
        let center_moved = self
            .last_layout_center
            .is_none_or(|prev| prev.distance(layout_center) > 0.5);
        let size_changed = self.last_canvas_size.is_none_or(|prev| {
            (prev.x - canvas_size.x).abs() > 0.5 || (prev.y - canvas_size.y).abs() > 0.5
        });
        let should_rebuild =
            self.galaxy_needs_rebuild || self.planets.is_empty() || center_moved || size_changed;

        if should_rebuild && let Some(galaxy) = &self.galaxy {
            let radius = (canvas_rect.width().min(canvas_rect.height()) * 0.35).max(50.0);
            let (planets, edges) =
                build_planets_and_edges_from_galaxy(galaxy, layout_center, radius);

            // Clean up planet states and cached positions for removed planets
            let planet_ids: HashSet<ID> = planets.iter().map(|p| p.id).collect();
            self.planet_states.retain(|id, _| planet_ids.contains(id));
            self.cached_pos_by_id
                .retain(|id, _| planet_ids.contains(id));

            self.planets = planets;
            self.edges = edges;

            self.galaxy_needs_rebuild = false;
            self.last_layout_center = Some(layout_center);
            self.last_canvas_size = Some(canvas_size);
        }
    }

    /// Rebuild the id -> screen position cache when the planet list changes.
    pub fn refresh_pos_cache(&mut self) {
        // Rebuild every frame: planet positions can change when layout center/size changes,
        // even if the number of planets stays the same.
        self.cached_pos_by_id.clear();
        for planet in &self.planets {
            self.cached_pos_by_id.insert(planet.id, planet.pos);
        }
    }
}
