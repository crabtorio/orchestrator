use crate::compat::ExplorerType;
use std::fs;

pub struct StartupState {
    pub explorer_slot_one: Option<ExplorerType>,
    pub explorer_slot_two: Option<ExplorerType>,
    pub game_step_ms: u32,
    pub galaxy_path: String,
    pub galaxy_contents: String,
    pub galaxy_dirty: bool,
    pub last_file_status: Option<String>,
    pub last_file_error: Option<String>,
}

impl StartupState {
    pub fn new() -> Self {
        Self {
            explorer_slot_one: Some(ExplorerType::Explorer),
            explorer_slot_two: None,
            game_step_ms: 2000,
            galaxy_path: "galaxy/test_galaxy.txt".to_owned(),
            galaxy_contents: String::new(),
            galaxy_dirty: false,
            last_file_status: None,
            last_file_error: None,
        }
    }

    pub fn load_galaxy_file(&mut self) -> bool {
        self.last_file_error = None;
        match fs::read_to_string(&self.galaxy_path) {
            Ok(contents) => {
                self.galaxy_contents = contents;
                self.galaxy_dirty = false;
                self.last_file_status = Some("Galaxy file loaded.".to_owned());
                true
            }
            Err(err) => {
                self.last_file_error = Some(format!(
                    "Failed to read galaxy file '{}': {err}",
                    self.galaxy_path
                ));
                false
            }
        }
    }

    pub fn save_galaxy_file(&mut self) -> bool {
        self.last_file_error = None;
        match fs::write(&self.galaxy_path, &self.galaxy_contents) {
            Ok(()) => {
                self.galaxy_dirty = false;
                self.last_file_status = Some("Galaxy file saved.".to_owned());
                true
            }
            Err(err) => {
                self.last_file_error = Some(format!(
                    "Failed to write galaxy file '{}': {err}",
                    self.galaxy_path
                ));
                false
            }
        }
    }
}
