use std::collections::HashSet;

pub struct Galaxy {
    topology: Topology,
    alive_planets: HashSet<PlanetId>,
}
pub struct Topology {
    adj: Vec<Vec<PlanetId>>,
}
pub struct PlanetId(usize);
