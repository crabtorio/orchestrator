use crate::orchestrator::Command;
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

pub trait Ai: Send {
    fn run(&self, ai_queue: Arc<Mutex<VecDeque<Command>>>);
}

pub struct Auto;

impl Ai for Auto {
    fn run(&self, ai_queue: Arc<Mutex<VecDeque<Command>>>) {
        //Todo
    }
}
