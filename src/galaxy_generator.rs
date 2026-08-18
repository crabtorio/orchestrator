use common_game::components::planet::Planet;
use common_game::protocols::{orchestrator_planet, planet_explorer};
use common_game::utils::ID;
use rustrelli::ExplorerRequestLimit;
use serde::ser::SerializeSeq;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Debug, Serialize, Deserialize, Clone, Copy, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PlanetVendor {
    RustyCrab,
    Rustrelli,
    Carbonium,
    Luna4,
    Orbitron,
    PubRustEze,
    Skycartel,
}
impl Distribution<PlanetVendor> for StandardNormal {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> PlanetVendor {
        match rng.random_range(0..7) {
            0 => PlanetVendor::RustyCrab,
            1 => PlanetVendor::Rustrelli,
            2 => PlanetVendor::Carbonium,
            3 => PlanetVendor::Luna4,
            4 => PlanetVendor::Orbitron,
            5 => PlanetVendor::PubRustEze,
            6 => PlanetVendor::Skycartel,
            other => panic!("Planet rng somehow rolled outside of bounds: {}", other),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)] //What even is this, actually? Regardless of that, why would it need be public?
pub struct PlanetData {
    vendor: PlanetVendor,
}

#[derive(Debug, Serialize, Deserialize)] //Same as comment before, but doubly so
pub struct GalaxyDef {
    #[serde(rename = "planet")]
    planets: Vec<PlanetData>,
}

// == Galaxy generation and implementation ==
//implementing the galaxy entirely because a good refactor was needed even if Luca wastes my time
use rand::Rng;
use rand::distr::Distribution;
use rand_distr::StandardNormal;
use std::collections::{HashMap, HashSet};
use std::hash::Hash;
use std::sync::{Arc, Mutex};

pub struct Galaxy {
    pub planets: HashMap<ID, Arc<Mutex<PlanetContainer>>>, //Shifted to a hashmap to ease removal down the line, otherwise acts as a vec for what we need. ID entry is for number-driven access like rands
                                                       //Other galaxy-wide info can go here
}

pub struct PlanetContainer {
    handling_id: ID,
    planet: Planet,
    pub adj: Vec<Arc<Mutex<PlanetContainer>>>,
    vendor: PlanetVendor, //Stored vendor because otherwise unknown
    pub tx_planet: crossbeam_channel::Sender<orchestrator_planet::OrchestratorToPlanet>,
    pub rx_planet: crossbeam_channel::Receiver<orchestrator_planet::PlanetToOrchestrator>,
    pub tx_explorer: crossbeam_channel::Sender<planet_explorer::ExplorerToPlanet>,
}

impl PlanetContainer {
    pub fn run(&mut self) {
        self.planet.run();
    }
    pub fn id(&self) -> ID {
        self.handling_id
    }
    //new is the WIP creation, since the only values accessed is handler_id and adj, either being discardable or initialized at empty
    fn new(id: ID) -> Self {
        let vendor: PlanetVendor = rand::rng().sample(StandardNormal);
        let (tp1, rp1) = crossbeam_channel::unbounded();
        let (tp2, rp2) = crossbeam_channel::unbounded();
        let (te, re) = crossbeam_channel::unbounded();
        PlanetContainer {
            vendor,
            planet: PlanetContainer::get_planet(vendor, rp1, tp2, re, id),
            handling_id: id,
            adj: Vec::new(),
            tx_planet: tp1,
            rx_planet: rp2,
            tx_explorer: te,
        }
    }
    fn get_planet(
        //Moved inside the PlanetContainer struct where it belongs, and removed the pub because no other file should get this
        planet_type: PlanetVendor,
        rx_orchestrator: crossbeam_channel::Receiver<orchestrator_planet::OrchestratorToPlanet>,
        tx_orchestrator: crossbeam_channel::Sender<orchestrator_planet::PlanetToOrchestrator>,
        rx_explorer: crossbeam_channel::Receiver<planet_explorer::ExplorerToPlanet>,
        planet_id: ID,
    ) -> Planet {
        match planet_type {
            PlanetVendor::RustyCrab => rusty_crab_ap2025::planet::create_planet(
                rx_orchestrator,
                tx_orchestrator,
                rx_explorer,
                planet_id,
            ),
            PlanetVendor::Rustrelli => rustrelli::create_planet(
                planet_id,
                rx_orchestrator,
                tx_orchestrator,
                rx_explorer,
                ExplorerRequestLimit::FairShare,
            ),
            PlanetVendor::Carbonium => {
                carbonium::create_planet(planet_id, rx_orchestrator, tx_orchestrator, rx_explorer)
            }
            PlanetVendor::Luna4 => {
                match luna4::create_planet(planet_id, rx_orchestrator, tx_orchestrator, rx_explorer)
                {
                    Ok(planet) => planet,
                    Err(error) => panic!("Error while creating Luna4 planet: \n{}", error),
                }
            }
            PlanetVendor::Orbitron => {
                orbitron::create_planet(rx_orchestrator, tx_orchestrator, rx_explorer, planet_id)
            }
            PlanetVendor::PubRustEze => pub_rust_eze::create_planet(
                planet_id,
                rx_orchestrator,
                tx_orchestrator,
                rx_explorer,
            ),
            PlanetVendor::Skycartel => {
                skycartel::create_planet(planet_id, rx_orchestrator, tx_orchestrator, rx_explorer)
            }
        }
    }
}

impl PartialEq for PlanetContainer {
    fn eq(&self, other: &PlanetContainer) -> bool {
        self.handling_id == other.handling_id
    }
}

impl Eq for PlanetContainer {}

impl Hash for PlanetContainer {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.handling_id.hash(state);
    }
}

impl Galaxy {
    pub fn iter(&self) -> impl Iterator<Item = Arc<Mutex<PlanetContainer>>> {
        self.planets.values().cloned()
    }
    pub fn from_random_distribution(planet_count: i32, expected_percentage: f64) -> Self {
        //Of course starting a galaxy should be done as it's method
        struct Coords {
            x: f64,
            y: f64,
            z: f64,
        } //Internal struct where you heathens can't be tempted to touch it

        impl Coords {
            fn new() -> Self {
                Coords {
                    x: rand::rng().sample(StandardNormal),
                    y: rand::rng().sample(StandardNormal),
                    z: rand::rng().sample(StandardNormal),
                }
            }

            fn clone(&self) -> Self {
                Coords {
                    x: self.x,
                    y: self.y,
                    z: self.z,
                }
            }

            fn get_distance(&self, other: &Self) -> f64 {
                ((self.x - other.x).powi(2)
                    + (self.y - other.y).powi(2)
                    + (self.z - other.z).powi(2))
                .sqrt()
            }
        }
        struct CoordContainer {
            id: ID,
            adj: Vec<ID>,
            c: Coords,
        } //Encapsulation step, comes with a few useful methods
        impl CoordContainer {
            fn new(id: ID) -> Self {
                CoordContainer {
                    id,
                    adj: Vec::new(),
                    c: Coords::new(),
                }
            }

            fn get_dist(&self, other: &Self) -> f64 {
                self.c.get_distance(&other.c)
            }
        }
        impl PartialEq for CoordContainer {
            fn eq(&self, other: &Self) -> bool {
                self.id == other.id
            }
        }

        impl Eq for CoordContainer {}

        impl Clone for CoordContainer {
            fn clone(&self) -> Self {
                CoordContainer {
                    id: self.id,
                    adj: self.adj.clone(),
                    c: self.c.clone(),
                }
            }
        }

        impl Hash for CoordContainer {
            fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                self.id.hash(state);
            }
        }

        struct LateBinding {
            p1: ID,
            p2: ID,
        }

        impl LateBinding {
            fn new(p1: ID, p2: ID) -> Self {
                LateBinding { p1, p2}
            }

            fn resolve(self, containers: &mut Vec<CoordContainer>) {
                containers[self.p1 as usize].adj.push(self.p2);
                containers[self.p2 as usize].adj.push(self.p1);
            }
        }

        let adj_size = match expected_percentage {
            //Simple piecewise linear interpolation
            ..=0.0 => 0.0, //Negative values are normalized to zero
            0.0..=68.0 => expected_percentage / 68.0, //First line, between 0 and 1
            68.0..=95.0 => (expected_percentage - 68.0) / (95.0 - 68.0) + 1.0, //Second line between 1 and 2
            _ => (expected_percentage - 95.0) / (99.0 - 95.0) + 2.0, //Second line, from 2 and passes through 3 but with no upper bound
        };

        let mut hold_vec = Vec::new();

        if planet_count == 0 {
            return Galaxy {
                planets: HashMap::new(),
            };
        } //Early out for no-planet galaxy

        for i in 0..planet_count {
            hold_vec.push(CoordContainer::new(i as ID));
        }

        for i in 0..planet_count as usize {
            for ii in 0..planet_count as usize {
                let dist = hold_vec[i].get_dist(&hold_vec[ii]);
                if i != ii && dist <= adj_size { //Makes sure the planet is not adjacent to self
                    hold_vec[i].adj.push(ii as ID); //Simple adjacency, the following algorithm will guarantee connectedness
                }
            }
        }

        let mut in_set = HashSet::new();
        let mut out_set = HashSet::new();
        let mut lag_set = HashSet::new();

        in_set.insert(0usize);
        for i in 1..planet_count as usize { //preload non-cleared set
            out_set.insert(i);
        }

        let mut late_queue = Vec::new(); //Deferring the de-orphaning to  the last step saves some calculations by reducing the size of the expansion necessary of a temp set

        while !out_set.is_empty() { //By running the out_set to the ground we are sure there are no orphans left

            let mut has_updated = true;

            while has_updated {
                has_updated = false;
                let temp_set = in_set.clone(); //The temp set is necessary because the iterator doesn't like to have its set touched
                for i in temp_set.difference(&lag_set) {
                    for ii in hold_vec[i.clone()].adj.iter() {
                        let pos = ii.clone() as usize;
                        in_set.insert(pos);
                        has_updated =  out_set.remove(&pos) || has_updated; //The OR preserves true results
                    }
                }
                lag_set = temp_set;
            }

            if !out_set.is_empty() { //Double-check the last expansion hasn't depleted the out_sets

                let min_dist = f64::INFINITY; //Since this is infinity all values should be smaller, effectively guaranteeing at least one true later
                let mut a = None; //Options to make Rust not cry
                let mut b = None;
                let mut c = None;
                for i in in_set.iter() {
                    for ii in out_set.iter() {
                        let dist = hold_vec[*i].get_dist(&hold_vec[*ii]);
                        if dist < min_dist {
                            a = Some(i.clone());
                            b = Some(ii.clone());
                            c = Some(ii.clone()); //Third copy just for comfort, could be optimized away by unwrapping and extracting data earlier, but this is nicer
                        }
                    }
                }


                late_queue.push(LateBinding::new(a.unwrap() as ID, b.unwrap() as ID)); //I am once again reminding you that it would be very concerning for either to be None
                out_set.remove(&c.unwrap());
                in_set.insert(c.unwrap());
            }
        }

        while !late_queue.is_empty() {
            late_queue.pop().unwrap().resolve(&mut hold_vec); //Resolves all the outstanding late bindings, using the nice method I made earlier
        }

        let mut conversion_array = Vec::new();
        let mut ending_set = HashMap::new(); //Return to hashmap because the extra bits actually matter now

        for i in 0..planet_count as usize {
            conversion_array.push(Arc::new(Mutex::new(PlanetContainer::new(hold_vec[i].id))));
        }

        for i in 0..planet_count as usize {
            let mut planet = conversion_array[i].lock().unwrap(); //get the mutexGuard of the element to prepare adjacencies to
            for ii in hold_vec[i].adj.iter() { //Iterate over all the pre-prepared adjacencies
                let pos = ii.clone() as usize;  //Indexing things is just so nice
                planet.adj.push(conversion_array[pos].clone()); //Clone the arc to put in the ajd vector
            }
        } //Lock lost here?

        for i in 0..planet_count as usize {
            ending_set.insert(hold_vec[i].id, conversion_array[i].clone()); //Insert all the arcs in the ending set with respective ids
        }

        Galaxy {
            planets: ending_set,
        }
    }
    pub fn drop_planet(&mut self, id: ID) {
        let planet_arc = self.planets[&id].clone(); //First isolate planet to remove all references to it
        let planet = planet_arc.lock().unwrap();
        //Is some AI shutdown needed? Goes here
        for i in planet.adj.iter() { //Isolate planet from all neighbors
            let mut other = i.lock().unwrap();
            for ii in 0..other.adj.len() {
                if Arc::ptr_eq(&planet_arc, &other.adj[ii]) {
                    other.adj.remove(ii);
                }
            }
        }
        self.planets.remove(&id); //Then remove it from the hashmap
        //If this is called from some asteroid-management section, that function should cover explorer handling/killing
    }
}

#[derive(Serialize, Deserialize)]
struct PlanetEntry {
    planet_id: ID,
    adjacencies: Vec<ID>,
    vendor: PlanetVendor,
}

impl Serialize for Galaxy {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut seq = serializer.serialize_seq(Some(self.planets.len()))?;
        for (planet_id, planet) in &self.planets {
            let lock = planet.lock().unwrap();

            let adjacencies = lock.adj.iter().map(|adj_planet| {adj_planet.lock().unwrap().handling_id}).collect();

            seq.serialize_element(&PlanetEntry {
                planet_id: *planet_id,
                adjacencies,
                vendor: lock.vendor,
            })?;
        }
        seq.end()
    }
}

impl<'de> Deserialize<'de> for Galaxy {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let entries = Vec::<PlanetEntry>::deserialize(deserializer)?;

        let planets: HashMap<ID, Arc<Mutex<PlanetContainer>>> = entries
            .iter()
            .map(|entry| {
                let ((tp1, rp1), (tp2, rp2), (te, re)) = (
                    crossbeam_channel::unbounded(),
                    crossbeam_channel::unbounded(),
                    crossbeam_channel::unbounded(),
                );
                let container = Arc::new(Mutex::new(PlanetContainer {
                    handling_id: entry.planet_id,
                    planet: PlanetContainer::get_planet(
                        entry.vendor.clone(),
                        rp1,
                        tp2,
                        re,
                        entry.planet_id.try_into().unwrap(),
                    ),
                    adj: Vec::new(),
                    vendor: entry.vendor,
                    tx_planet: tp1,
                    rx_planet: rp2,
                    tx_explorer: te,
                }));
                (entry.planet_id, container)
            })
            .collect();

        for entry in &entries {
            let planet = planets[&entry.planet_id].clone();
            for adj_id in &entry.adjacencies {
                let adj = planets
                    .get(adj_id)
                    .ok_or_else(|| {
                        serde::de::Error::custom(format!(
                            "Adjacency references unknown planet id {adj_id}"
                        ))
                    })?
                    .clone();
                planet.lock().unwrap().adj.push(adj);
            }
        }

        Ok(Galaxy { planets })
    }
}

//Tests from here
#[cfg(test)]
mod tests {
    use super::*;
    use rand_distr::num_traits::ToPrimitive;

    #[test]
    fn test_rand_gen_count_and_spanning() {
        //Verify the number of planets is actually the one requested
        let mut rng = rand::rng();
        let count = rng.random_range(0..=1000);
        let some_adj: f64 = 80.0;
        let galaxy = Galaxy::from_random_distribution(count, some_adj);

        assert_eq!(count, galaxy.planets.iter().count().to_i32().unwrap()); //Also technically checks if the i32 used to make planets somehow made more than an i32 can fit

        let mut punch_card = HashSet::new();
        fn recursive_search(current: ID, galaxy: &Galaxy, punch_card: &mut HashSet<ID>) {
            let unvisited = punch_card.insert(current); //See if already visited
            if unvisited {

                let mut candidates = Vec::new();

                {
                    let lock = galaxy.planets[&current].lock().unwrap(); //Lock to look inside
                    for i in lock.adj.iter() {
                        let adj_lock = i.lock().unwrap(); //Look into neighbors
                        candidates.push(adj_lock.handling_id); //Put their IDs in the next list
                    }
                }//Extra scope to make sure the locks die in the meantime

                for i in candidates{
                    recursive_search(i, galaxy, punch_card); //Ideally since the locks are already out-of-scope there should be no eternal spins
                }
            }
        }

        recursive_search(1, &galaxy, &mut punch_card);

        assert_eq!(punch_card.len(), count as usize, "All planets connected");
    }

    #[test]
    fn test_rand_gen_sanity_adj() {
        //No planet should have itself in its adjacencies
        let mut rng = rand::rng();
        let count = rng.random_range(0..=1000);
        let some_adj: f64 = 80.0;
        let galaxy = Galaxy::from_random_distribution(count, some_adj);

        for i in galaxy.planets.values() {
            let planet = i.lock().unwrap();
            for ii in 0..planet.adj.len() {
                let adj = planet.adj[ii].lock().unwrap();
                assert_ne!(
                    planet.handling_id,
                    adj.handling_id,
                    "No planet should be self adjacent"
                );
            }
        }
    }
    #[test]
    fn test_rand_gen_no_cloning() {
        //No planet should have duplicate adjacencies
        let mut rng = rand::rng();
        let count = rng.random_range(0..=1000);
        let some_adj: f64 = 80.0;
        let galaxy = Galaxy::from_random_distribution(count, some_adj);

        for i in galaxy.planets.values() {
            let planet = i.lock().unwrap();
            for ii in 0..planet.adj.len() {
                for iii in ii+1..planet.adj.len() {
                    if(Arc::ptr_eq(&planet.adj[ii], &planet.adj[iii])) {
                        println!("{}", count);
                        println!("{}, {:?}", planet.handling_id, planet.vendor);
                        for d in 0..planet.adj.len() {
                            if d == ii || d == iii {
                                println!("\t!{}: {}!", d, planet.adj[d].lock().unwrap().handling_id);
                            } else {
                                println!("\t{}: {}", d, planet.adj[d].lock().unwrap().handling_id);
                            }
                        }

                        println!(";");
                        let lock = planet.adj[ii].lock().unwrap();
                        for d in 0..lock.adj.len() {
                            if Arc::ptr_eq(&lock.adj[d], &i) {
                                println!("\t!{}: {}!", d, planet.handling_id);
                            } else {
                                println!{"\t{}: {}", d, lock.adj[d].lock().unwrap().handling_id};
                            }
                        }
                    }
                    assert!(!Arc::ptr_eq(&planet.adj[ii], &planet.adj[iii]), "All adjacencies should be unique")
                }
            }
        }
    }

    #[test]
    fn test_rand_gen_simmetry() {
        //For any two planets A, B: A->B => B->A

        let mut rng = rand::rng();
        let count = rng.random_range(1..=1000);
        let some_adj: f64 = 80.0;
        let galaxy = Galaxy::from_random_distribution(count, some_adj);

        for i in galaxy.planets.values() { //For every planet A
            for ii in i.lock().unwrap().adj.iter() { //For every planet B: A->B
                let mut has_self = false;
                let mut planet = ii.lock().unwrap();
                for iii in planet.adj.iter() {//For every planet C: B->C
                    has_self = Arc::ptr_eq(iii, i) || has_self; //Check A == C
                }
                assert!(has_self, "Edge is symmetric");
            }
        }
    }

    #[test]
    fn test_serialize_deserialize() {

        let pre_galaxy = Galaxy::from_random_distribution(1000, 80.0);

        let jstr = serde_json::to_string(&pre_galaxy)
            .expect("A galaxy should always be json-serializeable");
        let post_galaxy: Galaxy = serde_json::from_str::<Galaxy>(&jstr.as_str())
            .expect("A valid json galaxy should always be deserializeable");

        fn eq_planets(pl1: &Arc<Mutex<PlanetContainer>>, pl2: &Arc<Mutex<PlanetContainer>>) -> bool {
            fn gather_data(pl:&Arc<Mutex<PlanetContainer>>) -> (PlanetVendor, ID, HashSet<ID>) {
                let lock = pl.lock().unwrap();
                let vendor = lock.vendor.clone();
                let id = lock.handling_id.clone();
                let mut adj_ids = HashSet::new();

                for i in lock.adj.iter() {
                    adj_ids.insert(i.lock().unwrap().handling_id);
                }

                (vendor, id, adj_ids)

            }

            gather_data(pl1) == gather_data(pl2)
        }

        for i in pre_galaxy.planets.keys() {
            assert!(eq_planets(&pre_galaxy.planets[i], &post_galaxy.planets[i]), "All planets in pre are equal in post unchanged");
        }

        for i in post_galaxy.planets.keys() {
            assert!(eq_planets(&pre_galaxy.planets[i], &post_galaxy.planets[i]), "All planets in post are equal in pre unchanged");
        }
    }
    
/*  //Enable this test if tou want to iterate something until it breaks
    #[test]
    fn brute_inconsisten_test() {
        loop {
            tests::test_rand_gen_no_cloning();
        }
    }
 */
}
