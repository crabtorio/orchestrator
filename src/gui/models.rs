use common_game::utils::ID;
use eframe::egui;
use crate::compat::{ExplorerType, id::PlanetKind};

#[derive(Clone)]
pub struct Planet {
    pub id: ID,
    pub pos: egui::Pos2, // Where it is on screen
    pub name: String,
    pub active: bool,
}

pub struct Explorer {
    pub _id: ID,
    pub _pos: egui::Pos2,
    pub _exp_type: ExplorerType,
    pub _bag: crate::compat::common_explorer::ExplorerBagContent,
}

#[derive(Clone, Copy)]
pub enum SpawnStage {
    None,
    SelectingType,
    SelectingNeighbors(PlanetKind), // Store the planet ID chosen
}
