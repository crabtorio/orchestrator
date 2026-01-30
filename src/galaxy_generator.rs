use serde::{Deserialize, Serialize};
use common_game::components::planet::{Planet as BasePlanet, Planet};
use common_game::protocols::{orchestrator_planet, planet_explorer};
use common_game::utils::ID;
use rustrelli::ExplorerRequestLimit;
use rusty_crab_ap2025::planet::create_planet;
use toml::de::Error;
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

#[derive(Debug,Serialize,Deserialize)]
pub struct Coord {
    x: f64,
    y: f64,
    z: f64
}

#[derive(Debug,Serialize,Deserialize)]
pub struct PlanetData {
    vendor: PlanetVendor,
    location : Coord
}

#[derive(Debug,Serialize,Deserialize)]
pub struct GalaxyDef {
    #[serde(rename="planet")]
    planets : Vec<PlanetData>
}

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

pub fn get_planet(
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