use serde::{Deserialize, Serialize};
use common_game::components::planet::{Planet as BasePlanet, Planet};
use common_game::protocols::{orchestrator_planet, planet_explorer};
use common_game::utils::ID;
use rustrelli::ExplorerRequestLimit;
use rusty_crab_ap2025::planet::create_planet;
use toml::de::Error;  //Clippy don't like them
use toml::toml;

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
    pub fn from_random_distribution(planet_count: i32, adj_size: f64) -> Self { //Of course starting a galaxy should be done as it's method
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

        let mut in_set = HashSet::new();
        let mut out_set = HashMap::new();

        for _ in 0..planet_count {
            let new_planet = PlanetContainer::new();
            out_set.insert(new_planet.Handling_ID, CoordContainer::new(Rc::new(RefCell::new(new_planet)))); //Birth of the planets, from random
        }

        for i in out_set.values() {
            for ii in out_set.values() {
                let dist = i.get_dist(ii);
                if dist <= adj_size && i != ii {
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
                let minDist = f64::INFINITY; //Since this is infinity all values should be smaller, effectively guaranteeing at least one true later
                let mut A = None; //Options to make Rust not cry
                let mut B = None;
                let mut C = None;
                for i in in_set.iter() {
                    for ii in out_set.values() {
                        let dist = i.get_dist(ii);
                        if dist < minDist {
                            A = Some(i.plnt.clone());
                            B = Some(ii.plnt.clone());
                            C = Some(ii.clone()); //Third copy just for confort, could be optimized away by unwrapping and extracting data earlier, but this is nicer
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

    /* I'll just comment out all the code that relies on coordinates, which again we are not going to neither save to file or use outside of the fancy generation
    pub fn deserialize_galaxy(path : &str) -> Result<GalaxyDef,Error> {
        toml::from_str(r#"
            [[planet]]
            vendor="rustrelli"
            [planet.location]
            x=42
            y=42
            z=42
        "#)
    }
    *///Also, serializing the galaxy should be a method of that galaxy for that sweet sweet &self

}