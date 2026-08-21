use common_game::utils::ID;
use dashmap::DashMap;

//Orchestrator
    //Explorertype
#[derive(Clone, Copy, Debug)]
pub enum ExplorerType {
    Vojager,
    Explorer,
    Nomad,
}
    //ExplorersLocationRef
    pub(crate) type ExplorersLocationRef = DashMap<ID, ID>;

    //logging::LogTarget
    pub mod logging;
    //Payload
    //TODO find way to for the payload module to work properly
    #[macro_export]
    macro_rules! payload {
    ($($key:ident : $val:expr),* $(,)?) => {{
        let mut p = common_game::logging::Payload::new();
        $(
            p.insert(stringify!($key).to_string(), format!{"{}", $val});
        )*
        p
    }};
}

    //Ui::{OrchestratorToUiUpdate, UiToOrchestratorCommand}
pub mod ui;

    //Planet::PlanetMap
pub mod planet;

    //Id
        //IdManager
pub mod id;

//PlanetKind
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanetKind {
    Trip,
    Rustrelli,
    Luna4,
    RustyCrab,
    Enterprise,
    Orbitron,
    Houston,
}

pub mod common_explorer;