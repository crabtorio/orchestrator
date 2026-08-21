use common_game::utils::ID;
use eframe::egui;
use crate::compat::planet::PlanetMap;
use crate::compat::{
    id::IdManager,
    id::PlanetKind};
use std::collections::HashSet;

use crate::gui::models::Planet;

fn canonical_edge(a: ID, b: ID) -> Option<(ID, ID)> {
    match a.cmp(&b) {
        std::cmp::Ordering::Equal => None,
        std::cmp::Ordering::Less => Some((a, b)),
        std::cmp::Ordering::Greater => Some((b, a)),
    }
}

#[allow(dead_code)]
pub fn format_bag_content(bag: &crate::compat::common_explorer::ExplorerBagContent) -> String {
    if bag.resources_amounts.is_empty() {
        return "bag: empty".to_string();
    }

    let mut entries: Vec<String> = bag
        .resources_amounts
        .iter()
        .map(|(k, v)| format!("{k:?}:{v}"))
        .collect();
    entries.sort();
    format!("bag: {}", entries.join(", "))
}

pub fn planet_group_name_from_id(id: ID) -> &'static str {
    match IdManager::planet_kind(id) {
        PlanetKind::Trip => "Trip",
        PlanetKind::Rustrelli => "Rustrelli",
        PlanetKind::Luna4 => "Luna4",
        PlanetKind::RustyCrab => "Rusty Crabs",
        PlanetKind::Enterprise => "Enterprise",
        PlanetKind::Orbitron => "Orbitron",
        PlanetKind::Houston => "Houston",
    }
}



// Map the orchestrator's galaxy snapshot into renderable planets + edges for egui
#[expect(
    clippy::cast_precision_loss,
    reason = "planet count and index cast to f32 for circular layout angles; counts are always small"
)]
pub fn build_planets_and_edges_from_galaxy(
    galaxy: &PlanetMap,
    center: egui::Pos2,
    radius: f32,
) -> (Vec<Planet>, Vec<(ID, ID)>) {
    let guard = galaxy
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner);

    let mut ids: Vec<ID> = guard.keys().copied().collect();
    ids.sort_unstable();

    let count = ids.len().max(1) as f32;
    let mut planets = Vec::with_capacity(ids.len());

    for (i, id) in ids.iter().enumerate() {
        let angle = (i as f32 / count) * std::f32::consts::TAU;
        let pos = egui::Pos2::new(
            center.x + radius * angle.cos(),
            center.y + radius * angle.sin(),
        );

        planets.push(Planet {
            id: *id,
            pos,
            name: format!("{} ({id})", planet_group_name_from_id(*id)),
            active: guard.get(id).is_none_or(|p| p.is_alive()),
        });
    }

    let mut edges = HashSet::new();
    for (id, node) in guard.iter() {
        for neighbor in node.neighbors_snapshot() {
            if let Some(edge) = canonical_edge(*id, neighbor) {
                edges.insert(edge);
            }
        }
    }

    (planets, edges.into_iter().collect())
}

pub fn display_explorer_name(name: &str) -> String {
    let lower = name.to_lowercase();
    if lower.contains("vojager") {
        "Rob Vojager".to_string()
    } else if lower.contains("nomad") {
        "Jaco Nomad".to_string()
    } else if lower.contains("nico") || lower.contains("explorer") {
        "Nico Explorer".to_string()
    } else {
        name.to_string()
    }
}
