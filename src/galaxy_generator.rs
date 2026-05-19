use common_game::components::planet::Planet;
use common_game::protocols::{orchestrator_planet, planet_explorer};
use common_game::utils::ID;
use rustrelli::ExplorerRequestLimit;
use serde::ser::SerializeSeq;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
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
use std::hash::Hash;
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    rc::Rc,
};
pub struct Galaxy {
    planets: HashMap<ID, Rc<RefCell<PlanetContainer>>>, //Shifted to a hashmap to ease removal down the line, otherwise acts as a vec for what we need. i32 entry is for number-driven acess like rands
                                                        //Other, galaxy-wide info can go here
}

pub struct PlanetContainer {
    handling_id: ID,
    planet: Planet,
    adj: Vec<Rc<RefCell<PlanetContainer>>>,
    vendor: PlanetVendor, //Stored vendor because otherwise unknown
    tx_planet: crossbeam_channel::Sender<orchestrator_planet::OrchestratorToPlanet>,
    rx_planet: crossbeam_channel::Receiver<orchestrator_planet::PlanetToOrchestrator>,
    tx_explorer: crossbeam_channel::Sender<planet_explorer::ExplorerToPlanet>,
}

impl PlanetContainer {
    pub fn run(&mut self) {
        self.planet.run();
    }
    pub fn id(&self) -> ID {
        self.handling_id
    }
    //new is the WIP creation, since the only values accessed is handler_id and adj, either being discardable or initialized at empty
    fn new(id: &mut ID) -> Self {
        *id += 1;
        let vendor: PlanetVendor = rand::rng().sample(StandardNormal);
        let (tp1, rp1) = crossbeam_channel::unbounded();
        let (tp2, rp2) = crossbeam_channel::unbounded();
        let (te, re) = crossbeam_channel::unbounded();
        PlanetContainer {
            vendor: vendor,
            planet: PlanetContainer::get_planet(vendor, rp1, tp2, re, *id - 1),
            handling_id: *id - 1,
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
    pub fn iter(&self) -> impl Iterator<Item = Rc<RefCell<PlanetContainer>>> {
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
            plnt: Rc<RefCell<PlanetContainer>>,
            c: Coords,
        } //Encapsulation step, comes with a few useful methods

        impl CoordContainer {
            fn new(plnt: Rc<RefCell<PlanetContainer>>) -> Self {
                CoordContainer {
                    plnt,
                    c: Coords::new(),
                }
            }

            fn get_dist(&self, other: &Self) -> f64 {
                self.c.get_distance(&other.c)
            }
        }
        impl PartialEq for CoordContainer {
            fn eq(&self, other: &Self) -> bool {
                self.plnt.borrow().handling_id == other.plnt.borrow().handling_id
            }
        }

        impl Eq for CoordContainer {}

        impl Clone for CoordContainer {
            fn clone(&self) -> Self {
                CoordContainer {
                    plnt: self.plnt.clone(),
                    c: self.c.clone(),
                }
            }
        }

        impl Hash for CoordContainer {
            fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                self.plnt.borrow().handling_id.hash(state);
            }
        }

        struct LateBinding {
            p1: Rc<RefCell<PlanetContainer>>,
            p2: Rc<RefCell<PlanetContainer>>,
        }

        impl LateBinding {
            fn new(p1: Rc<RefCell<PlanetContainer>>, p2: Rc<RefCell<PlanetContainer>>) -> Self {
                LateBinding { p1, p2 }
            }

            fn resolve(self) {
                self.p1.borrow_mut().adj.push(self.p2.clone());
                self.p2.borrow_mut().adj.push(self.p1);
            }
        }

        let adj_size = match expected_percentage {
            //Simple piecewise linear interpolation
            ..=0.0 => 0.0, //Negative values are normalized to zero
            0.0..=68.0 => expected_percentage / 68.0, //First line, between 0 and 1
            68.0..=95.0 => (expected_percentage - 68.0) / (95.0 - 68.0) + 1.0, //Second line between 1 and 2
            _ => (expected_percentage - 95.0) / (99.0 - 95.0) + 2.0, //Second line, from 2 and passes through 3 but with no upper bound
        };

        let mut in_set = HashSet::new();
        let mut out_set = HashMap::new();
        let mut lag_set = HashSet::new();

        const IDSTART: ID = 1;
        let mut id_count = IDSTART;

        if planet_count == 0 {
            return Galaxy {
                planets: HashMap::new(),
            };
        } //Early out for no-planet galaxy

        for _ in 0..planet_count {
            let new_planet = PlanetContainer::new(&mut id_count);
            out_set.insert(
                new_planet.handling_id,
                CoordContainer::new(Rc::new(RefCell::new(new_planet))),
            ); //Birth of the planets, from random
        }

        for i in out_set.values() {
            for ii in out_set.values() {
                let dist = i.get_dist(ii);
                if dist <= adj_size && i != ii {
                    //Makes sure the planet is not adjacent to itself
                    i.plnt.borrow_mut().adj.push(ii.plnt.clone()); //Simple adjacency, the prim algorithm later will sort out
                }
            }
        }

        in_set.insert(out_set.remove(&IDSTART).unwrap()); //Again, under any sane circumstance this should be Some
        let mut late_queue = Vec::new(); //Deferring the de-orphaning to  the last step saves some calculations by reducing the size of the expansion necessary of a temp set
        while !out_set.is_empty() {
            //By running the out_set to the ground we are sure there are no orphans left
            let mut has_updated = true;
            while has_updated {
                has_updated = false;
                let temp_set = in_set.clone(); //The temp set is necessary because the iterator doesn't like to have its set touched
                for i in temp_set.difference(&lag_set) {
                    for ii in i.plnt.borrow().adj.iter() {
                        let maybe_entry = out_set.remove(&ii.borrow().handling_id);
                        match maybe_entry {
                            None => {}
                            Some(entry) => {
                                has_updated = in_set.insert(entry) || has_updated;
                            } //The OR preserves true results, and the Option allows us to remove everything willy nilly
                        }
                    }
                }
                lag_set = temp_set;
            }
            if !out_set.is_empty() {
                //Double check the last expansion hasn't depleted the out_sets
                let min_dist = f64::INFINITY; //Since this is infinity all values should be smaller, effectively guaranteeing at least one true later
                let mut a = None; //Options to make Rust not cry
                let mut b = None;
                let mut c = None;
                for i in in_set.iter() {
                    for ii in out_set.values() {
                        let dist = i.get_dist(ii);
                        if dist < min_dist {
                            a = Some(i.plnt.clone());
                            b = Some(ii.plnt.clone());
                            c = Some(ii.clone()); //Third copy just for comfort, could be optimized away by unwrapping and extracting data earlier, but this is nicer
                        }
                    }
                }
                late_queue.push(LateBinding::new(a.unwrap(), b.unwrap().clone())); //I am once again reminding you that it would be very concerning for either to be None
                let temp = c.unwrap();
                out_set.remove(&temp.plnt.borrow().handling_id);
                in_set.insert(temp);
            }
        }
        while !late_queue.is_empty() {
            late_queue.pop().unwrap().resolve(); //Resolves all the outstanding late bindings, using the nice method I made earlier
        }
        let mut ending_set = HashMap::new(); //Return to hashmap because the extra bits actually matter now
        for i in in_set.drain() {
            let temp = i.plnt.borrow().handling_id;
            ending_set.insert(temp, i.plnt);
        }

        Galaxy {
            planets: ending_set,
        }
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
            let adjacencies = planet
                .borrow()
                .adj
                .iter()
                .map(|adj_planet| {
                    self.planets
                        .iter()
                        .find_map(|(pid, p)| if p == adj_planet { Some(*pid) } else { None })
                        .expect("Planet must be in Galaxy")
                })
                .collect();

            seq.serialize_element(&PlanetEntry {
                planet_id: *planet_id,
                adjacencies,
                vendor: planet.borrow().vendor,
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

        let planets: HashMap<ID, Rc<RefCell<PlanetContainer>>> = entries
            .iter()
            .map(|entry| {
                let ((tp1, rp1), (tp2, rp2), (te, re)) = (
                    crossbeam_channel::unbounded(),
                    crossbeam_channel::unbounded(),
                    crossbeam_channel::unbounded(),
                );
                let container = Rc::new(RefCell::new(PlanetContainer {
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
                planet.borrow_mut().adj.push(adj);
            }
        }

        Ok(Galaxy { planets })
    }
}

//Tests from here
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rand_gen_count_and_spanning() {
        //Verify the number of planets is actually the one requested
        let mut rng = rand::rng();
        let count = rng.random_range(0..=1000);
        let some_adj: f64 = 80.0;
        let mut galaxy = Galaxy::from_random_distribution(count, some_adj);

        assert_eq!(count, galaxy.planets.iter().count().to_i32().unwrap()); //Also technically checks if the i32 used to make planets somehow made more than an i32 can fit

        let connected_punchcard: Rc<RefCell<HashSet<ID>>> = Rc::new(RefCell::new(HashSet::new()));
        let starting = galaxy.planets.get(&1).unwrap().clone();
        fn recursive_expl(punch: Rc<RefCell<HashSet<ID>>>, cur: Rc<RefCell<PlanetContainer>>) {
            //Function loads into a container all the nodes connected to one
            if punch.borrow_mut().insert(cur.borrow().handling_id) {
                for i in cur.borrow().adj.iter() {
                    recursive_expl(punch.clone(), i.clone());
                }
            }
        }

        recursive_expl(connected_punchcard.clone(), starting);
        for i in galaxy.planets.keys() {
            assert!(connected_punchcard.borrow().contains(i)); //If there's an ID in the galaxy and not the punchcard, something's missing
        }
    }

    #[test]
    fn test_rand_gen_sanity_adj() {
        //No planet should have itself in its adjacencies, or duplicates
        let mut rng = rand::rng();
        let count = rng.random_range(0..=1000);
        let some_adj: f64 = 80.0;
        let mut galaxy = Galaxy::from_random_distribution(count, some_adj);

        for i in galaxy.planets.values() {
            for ii in 0..i.borrow().adj.len() {
                assert_ne!(
                    i.borrow().handling_id,
                    i.borrow().adj[ii].borrow().handling_id
                ); //No self reference
                for iii in ii + 1..i.borrow().adj.len() {
                    assert_ne!(
                        i.borrow().adj[ii].borrow().handling_id,
                        i.borrow().adj[iii].borrow().handling_id
                    ); //No duplicates
                }
            }
        }
    }

    #[test]
    fn test_rand_gen_simmetry() {
        //For any two planets A, B: A->B => B->A

        let mut rng = rand::rng();
        let count = rng.random_range(0..=1000);
        let some_adj: f64 = 80.0;
        let mut galaxy = Galaxy::from_random_distribution(count, some_adj);

        for i in galaxy.planets.values() {
            //For every planet A
            for ii in i.borrow().adj.iter() {
                //For every planet B: A->B
                let mut has_self = false;
                for iii in ii.borrow().adj.iter() {
                    //For every planet C: B->C
                    has_self = iii.borrow().handling_id == i.borrow().handling_id || has_self; //Check A == C
                }
                assert!(has_self);
            }
        }
    }
}
