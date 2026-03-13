use serde::ser::SerializeSeq;
use serde::{Deserialize, Serialize, Serializer};
use common_game::components::planet::{Planet as BasePlanet, Planet};
use common_game::protocols::{orchestrator_planet, planet_explorer};
use common_game::utils::ID;
use rustrelli::ExplorerRequestLimit;
use rusty_crab_ap2025::planet::create_planet;
use toml::de::Error;  //Clippy don't like them

#[derive(Debug,Serialize,Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanetVendor {
    RustyCrab,
    Rustrelli,
    Carbonium,
    Luna4,
    Orbitron,
    PubRustEze,
    Skycartel
}

#[derive(Debug,Serialize,Deserialize)]  //What even is this, actually? Regardless of that, why would it need be public?
pub struct PlanetData {
    vendor: PlanetVendor,
    //location : Coord TODO luca fix it
}

#[derive(Debug,Serialize,Deserialize)] //Same as comment before, but doubly so
pub struct GalaxyDef {
    #[serde(rename="planet")]
    planets : Vec<PlanetData>
}

// == Galaxy generation and implementation ==
//implementing the galaxy entirely because a good refactor was needed even if Luca wastes my time
use std::{rc::Rc, cell::RefCell, collections::{HashMap, HashSet}, sync::atomic::{AtomicUsize, Ordering}};
use std::hash::Hash;
use rand::Rng;
use rand_distr::StandardNormal;

pub struct Galaxy {
    planets: HashMap<usize, Rc<RefCell<PlanetContainer>>>  //Shifted to a hashmap to ease removal down the line, otherwise acts as a vec for what we need. Usize entry is for number-driven acess like rands
    //Other, galaxy-wide info can go here
}

struct PlanetContainer {
    Handling_ID: usize,
//    planet: Planet    //Uncommented when the planet selection is complete. for one, this one is actually on me
    adj: Vec<Rc<RefCell<PlanetContainer>>>
}

const IDCOUNTER_START: usize = 1;
static IDCOUNTER: AtomicUsize = AtomicUsize::new(IDCOUNTER_START); //ID counter and AtomicUsize imports are temporary until i understand how the ID field on the actual planets work

static MAXID: AtomicUsize = AtomicUsize::new(IDCOUNTER_START);  //Pub for simplification of random functions

impl PlanetContainer {  //new is the WIP creation, since the only values accessed is handler_id and adj, either being discardable or initialized at empty
    fn new() -> Self {
        PlanetContainer{
            Handling_ID: IDCOUNTER.fetch_add(1, Ordering::Relaxed),
            adj: Vec::new()
        }
    }
    fn get_planet(  //Moved inside the PlanetContainer struct where it belongs, and removed the pub because no other file should get this
        planet_type: PlanetVendor,
        rx_orchestrator: crossbeam_channel::Receiver<orchestrator_planet::OrchestratorToPlanet>,
        tx_orchestrator: crossbeam_channel::Sender<orchestrator_planet::PlanetToOrchestrator>,
        rx_explorer: crossbeam_channel::Receiver<planet_explorer::ExplorerToPlanet>,
        planet_id: ID,
    ) -> Planet {
        match planet_type {
            PlanetVendor::RustyCrab => rusty_crab_ap2025::planet::create_planet(rx_orchestrator, tx_orchestrator, rx_explorer, planet_id),
            PlanetVendor::Rustrelli => rustrelli::create_planet(planet_id, rx_orchestrator, tx_orchestrator, rx_explorer, ExplorerRequestLimit::FairShare),
            PlanetVendor::Carbonium => todo!(),
            PlanetVendor::Luna4 => todo!(),
            PlanetVendor::Orbitron => todo!(),
            PlanetVendor::PubRustEze => todo!(),
            PlanetVendor::Skycartel => todo!(),
        }
    }
}

impl PartialEq for PlanetContainer {
    fn eq(&self, other: &PlanetContainer) -> bool {
        self.Handling_ID == other.Handling_ID
    }
}

impl Eq for PlanetContainer {}

impl Hash for PlanetContainer {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.Handling_ID.hash(state);
    }
}

impl Galaxy {
    pub fn from_random_distribution(planet_count: i32, expected_percentage: f64) -> Self { //Of course starting a galaxy should be done as it's method
        struct Coords {
            x: f64,
            y: f64,
            z: f64
        }   //Internal struct where you heathens can't be tempted to touch it

        impl Coords {
            fn new() -> Self {
                Coords {
                    x: rand::rng().sample(StandardNormal),
                    y: rand::rng().sample(StandardNormal),
                    z: rand::rng().sample(StandardNormal)
                }
            }

            fn clone(&self) -> Self {
                Coords { x: self.x, y: self.y, z: self.z }
            }

            fn get_distance (&self, other: &Self) -> f64 {
                ((self.x-other.x).powi(2) + (self.y-other.y).powi(2) + (self.z-other.z).powi(2)).sqrt()
            }
        }
        struct CoordContainer {
            plnt: Rc<RefCell<PlanetContainer>>,
            c: Coords
        } //Encapsulation step, comes with a few useful methods

        impl CoordContainer {

            fn new (plnt: Rc<RefCell<PlanetContainer>>) -> Self {
                CoordContainer{plnt, c: Coords::new()}
            }

            fn get_dist (&self, other: &Self) -> f64 {
                self.c.get_distance(&other.c)
            }
        }
        impl PartialEq for CoordContainer {
            fn eq(&self, other: &Self) -> bool {
                self.plnt.borrow().Handling_ID == other.plnt.borrow().Handling_ID
            }
        }

        impl Eq for CoordContainer {}

        impl Clone for CoordContainer {
            fn clone(&self) -> Self {
                CoordContainer{ plnt: self.plnt.clone(), c: self.c.clone()}
            }
        }

        impl Hash for CoordContainer {
            fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                self.plnt.borrow().Handling_ID.hash(state);
            }
        }

        struct late_binding {
            p1: Rc<RefCell<PlanetContainer>>,
            p2: Rc<RefCell<PlanetContainer>>,
        }

        impl late_binding {
            fn new(p1: Rc<RefCell<PlanetContainer>>, p2: Rc<RefCell<PlanetContainer>>) -> Self {
                late_binding{p1, p2}
            }

            fn resolve(self) {
                self.p1.borrow_mut().adj.push(self.p2.clone());
                self.p2.borrow_mut().adj.push(self.p1);
            }
        }

        let adj_size= match expected_percentage {   //Simple piecewise linear interpolation
            ..=0.0 => {0.0},    //Negative values are normalized to zero
            0.0..=68.0 => {expected_percentage/68.0},   //First line, between 0 and 1
            68.0..=95.0 => {(expected_percentage-68.0)/(95.0-68.0)+1.0},    //Second line between 1 and 2
            _ => {(expected_percentage-95.0)/(99.0-95.0)+2.0},  //Second line, from 2 and passes through 3 but with no upper bound
        };

        let mut in_set = HashSet::new();
        let mut out_set = HashMap::new();

        if planet_count == 0 {return Galaxy{ planets: HashMap::new() }} //Early out for no-planet galaxy

        for _ in 0..planet_count {
            let new_planet = PlanetContainer::new();
            out_set.insert(new_planet.Handling_ID, CoordContainer::new(Rc::new(RefCell::new(new_planet)))); //Birth of the planets, from random
        }

        for i in out_set.values() {
            for ii in out_set.values() {
                let dist = i.get_dist(ii);
                if dist <= adj_size && i != ii {    //Makes sure the planet is not adjacent to itself
                    i.plnt.borrow_mut().adj.push(ii.plnt.clone());  //Simple adjacency, the prim algorithm later will sort out
                }
            }
        }

        in_set.insert(out_set.remove(&IDCOUNTER_START).unwrap());   //Again, under any sane circumstance this should be Some
        let mut late_queue = Vec::new();    //Deferring the de-orphaning to  the last step saves some calculations by reducing the size of the expansion necessary of a temp set
        while !out_set.is_empty() { //By running the out_set to the ground we are sure there are no orphans left
            let mut has_updated = true;
            while has_updated {
                has_updated = false;
                let temp_set = in_set.clone();  //The temp set is necessary because the iterator doesn't like to have its set touched
                for i in temp_set.iter() {
                    for ii in i.plnt.borrow().adj.iter() {
                        let maybe_entry = out_set.remove(&ii.borrow().Handling_ID);
                        match maybe_entry {
                            None => {}
                            Some(entry) => {has_updated = has_updated || in_set.insert(entry);}   //The OR preserves true results, and the Option allows us to remove everything willy nilly
                        }
                    }
                }
            }
            if !out_set.is_empty() { //Double check the last expansion hasn't depleted the out_sets
                let min_dist = f64::INFINITY; //Since this is infinity all values should be smaller, effectively guaranteeing at least one true later
                let mut A = None; //Options to make Rust not cry
                let mut B = None;
                let mut C = None;
                for i in in_set.iter() {
                    for ii in out_set.values() {
                        let dist = i.get_dist(ii);
                        if dist < min_dist {
                            A = Some(i.plnt.clone());
                            B = Some(ii.plnt.clone());
                            C = Some(ii.clone()); //Third copy just for comfort, could be optimized away by unwrapping and extracting data earlier, but this is nicer
                        }
                    }
                }
                late_queue.push(late_binding::new(A.unwrap(), B.unwrap().clone())); //I am once again reminding you that it would be very concerning for either to be None
                let temp = C.unwrap();
                out_set.remove(&temp.plnt.borrow().Handling_ID);
                in_set.insert(temp);
            }
        }
        while !late_queue.is_empty() {
            late_queue.pop().unwrap().resolve();    //Resolves all the outstanding late bindings, using the nice method I made earlier
        }
        let mut ending_set = HashMap::new();    //Return to hashmap because the extra bits actually matter now
        for i in in_set.drain() {
            let temp = i.plnt.borrow().Handling_ID;
            ending_set.insert(temp, i.plnt);
        }

        Galaxy{planets: ending_set}
    }

}

#[derive(Serialize,Deserialize)]
struct PlanetEntry {
    planet_id : usize,
    adjacencies : Vec<usize>
}

impl Serialize for Galaxy {

    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where S: Serializer 
    {   
        let mut seq = serializer.serialize_seq(Some(self.planets.len()))?;
        for (planet_id, planet) in &self.planets {
            let adjacencies = planet.borrow().adj.iter()
                .map( |adj_planet| {
                        self.planets.iter().find_map(|(pid,p)| 
                            if p == adj_planet {
                                Some(*pid)
                            } else {
                                None
                            }
                        ).expect("Planet must be in Galaxy")
                    }
                ).collect();

            seq.serialize_element(
                &PlanetEntry{
                    planet_id: *planet_id,
                    adjacencies
                }
            )?;
        }
        seq.end()
    }

}

//Tests from here
#[cfg(test)]
mod tests {
    use rand::SeedableRng;
    use super::*;

    #[test]
    fn test_rand_gen_count_and_spanning() {
        //Verify the number of planets is actually the one requested
        let mut rng = rand::rng();
        let count = rng.random_range(0..=10000);
        let SOMEADJ: f64 = 25.0;
        let mut galaxy = Galaxy::from_random_distribution(count, SOMEADJ);

        assert!(count == galaxy.planets.iter().count().try_into().unwrap()); //Also technically cheks if the i32 used to make planets somehow made more than an i32 can fit

        let connected_punchcard:Rc<RefCell<HashSet<usize>>> = Rc::new(RefCell::new(HashSet::new()));
        let starting = galaxy.planets.get(&IDCOUNTER_START).unwrap().clone();
        fn recursive_expl (punch:Rc<RefCell<HashSet<usize>>>, cur: Rc<RefCell<PlanetContainer>>) {  //Function loads into a container all the nodes connected to one
            if punch.borrow_mut().insert(cur.borrow().Handling_ID) {
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
    }

    #[test]
    fn test_rand_gen_simmetry() {
        //For any two planets A, B: A->B => B->A
    }


}