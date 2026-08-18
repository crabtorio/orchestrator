use crate::orchestrator::Command;
use common_game::utils::ID;
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    thread,
    time,
};
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::Relaxed;
use rand::{Rng};
use rand_distr::StandardNormal;

pub trait Ai: Send {
    fn run(&self, ai_queue: Arc<Mutex<VecDeque<Command>>>, runflag: Arc<AtomicBool>, start: ID, end: ID);
}

pub struct RichardRandom;

impl Ai for RichardRandom {
    fn run(&self, ai_queue: Arc<Mutex<VecDeque<Command>>>, runflag: Arc<AtomicBool>, start: ID, end: ID) {

        //Parameter-constants, allow control for how the chances chance
        const NOTHING_WEIGHT: i32 = 10;
        const SUN_WEIGHT: i32 = 5;
        const ASTEROID_WEIGHT: i32 = 1;

        while runflag.load(Relaxed) {
            let target: ID = rand::random_range(start..end);
            let action: i32 = rand::random_range(1..(NOTHING_WEIGHT + SUN_WEIGHT + ASTEROID_WEIGHT));
            
            let mut lock = ai_queue.lock().unwrap();
            if action> NOTHING_WEIGHT && action<= NOTHING_WEIGHT + SUN_WEIGHT {
                //Send sunray to target
                lock.push_back(Command::SendSunray(target));

            } else if !(action<= NOTHING_WEIGHT) {
                //Send asteroid to target
                lock.push_back(Command::SendSunray(target));
            }
            
            //Sleep time is 1 + offset(clamped between +1 and -1)
            let sleep_offset: f32 = rand::rng().sample(StandardNormal);
            let sleep_time = match sleep_offset { ..=-1.0 => 0, 1.0.. => 2000, _ => (1000 + (sleep_offset *1000.0).round() as i32) as u64};
            let sleep_millis = time::Duration::from_millis(sleep_time);

            thread::sleep(sleep_millis);
        }
    }
}
