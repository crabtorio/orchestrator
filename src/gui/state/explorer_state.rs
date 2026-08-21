use common_game::utils::ID;
use std::collections::HashMap;

use crate::gui::models::Explorer;

pub struct ExplorerState {
    pub _explorers: Vec<Explorer>,
    pub explorer_positions: HashMap<ID, ID>, // explorer_id -> planet_id
    pub explorer_bags: HashMap<ID, crate::compat::common_explorer::ExplorerBagContent>,
}

impl ExplorerState {
    pub fn new() -> Self {
        Self {
            _explorers: Vec::new(),
            explorer_positions: HashMap::new(),
            explorer_bags: HashMap::new(),
        }
    }
}
