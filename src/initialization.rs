use std::cell::{RefCell};
use std::ops::Deref;
use std::rc::Rc;

pub struct Galaxy{
    planets: Vec<Rc<RefCell<PlanetContainer>>>,
    //Other significant galaxy-wide information can be stored here
    //Could include a "dead planets" vector if they need to be conserved after destruction
}

impl Galaxy{
    fn new_by_space_distribution(planet_count: i32, adj_size: f64) -> Self {
        let mut seed_vec: Vec<Rc<RefCell<PlanetContainer>>> = Vec::new();
        for _ in 0..planet_count {
            seed_vec.push(Rc::new(RefCell::new(PlanetContainer::from_rand())));
        }
        for i in &seed_vec {
            let mut pass_flag = false;
            let mut closest = i;
            let mut closest_dist = f64::INFINITY;
            for ii in &seed_vec {
                let distance = i.borrow().get_dist(ii.borrow().deref()); //black magik Rust wizardry
                if (i.borrow().id != ii.borrow().id) {
                    if (!pass_flag && distance < closest_dist) {
                        closest_dist = distance;
                        closest = ii
                    }
                    if (distance < adj_size) {
                        pass_flag = true;
                        i.borrow_mut().adj.push(ii.clone());
                    }
                }
            }
            if !pass_flag {
                i.borrow_mut().adj.push(closest.clone());
            }
        }

        Galaxy{planets: seed_vec}
    }
}

use common_game::components::planet::Planet as planet;

struct tempPlanet {} //temporary planet to make the compiler not mad while Luca figures out the planet importing
struct PlanetContainer { //Unifies the planet, its galaxy ID and encodes the adjacencies
    id: usize,
    planet: tempPlanet,
    adj: Vec<Rc<RefCell<PlanetContainer>>>,
    c: Coords
}

struct Coords {
    x: f64,
    y: f64,
    z: f64
}

use std::sync::atomic::{AtomicUsize, Ordering};
use rand::Rng;
use rand_distr::StandardNormal;



static IDCOUNTER: AtomicUsize = AtomicUsize::new(1);
impl PlanetContainer {

    fn get_dist (&self, other: &Self) -> f64 {
        self.c.get_distance(&other.c)
    }
    fn from_rand() -> Self {
        //TODO Luca has to figure this shite

        PlanetContainer{
            id: IDCOUNTER.fetch_add(1, Ordering::Relaxed),
            planet: tempPlanet {},
            adj: Vec::new(),
            c: Coords::from_rand()
        }
    }
}

impl Coords{
    fn from_rand() -> Self {
        Coords {
            x: rand::rng().sample(StandardNormal),
            y: rand::rng().sample(StandardNormal),
            z: rand::rng().sample(StandardNormal)
        }
    }

    fn get_distance (&self, other: &Self) -> f64 {
        ((self.x-other.x).powi(2) + (self.y-other.y).powi(2) + (self.z-other.z).powi(2)).sqrt()
    }
}