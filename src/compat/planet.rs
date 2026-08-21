use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::sync::atomic::AtomicBool;
use common_game::utils::ID;

pub type PlanetMap = Arc<RwLock<HashMap<ID, Arc<PlanetNode>>>>;

pub struct PlanetNode {
    id: ID,
    alive: AtomicBool,
    // Not a neighbor set. Just a pointer key so node methods can find the right ConnectionStore.
    map_key: usize,
}