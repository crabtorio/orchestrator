use crate::orchestrator::Command;
use common_game::utils::ID;
use rand::Rng;
use rand_distr::StandardNormal;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::Relaxed;
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    thread, time,
};

pub trait Ai: Send {
    fn run(
        &self,
        ai_queue: Arc<Mutex<VecDeque<Command>>>,
        runflag: Arc<AtomicBool>,
        ai_args: AiArgs,
    );
}
pub enum AiType {
    RichardRandom,
}

pub enum AiArgs {
    RichardRandom(ID, ID, Arc<Mutex<Vec<ID>>>),
}
pub struct RichardRandom;

impl Ai for RichardRandom {
    fn run(
        &self,
        ai_queue: Arc<Mutex<VecDeque<Command>>>,
        runflag: Arc<AtomicBool>,
        ai_args: AiArgs,
    ) {
        match ai_args {
            AiArgs::RichardRandom(start, end, deads) => {
                log::debug!("AI: RichardRandom is born with range: {} - {}", start, end);
                //Parameter-constants, allow control for how the chances chance

                const MAX_TARGET_TRIES: usize = 100;
                const NOTHING_WEIGHT: i32 = 10;
                const SUN_WEIGHT: i32 = 19;
                const ASTEROID_WEIGHT: i32 = 1;

                const RAY_BATCH_SIZE: usize = 4;

                while runflag.load(Relaxed) {

                    let deads_lock = deads.lock().expect("Failed to get lock");

                    let action: i32 =
                        rand::random_range(1..=(NOTHING_WEIGHT + SUN_WEIGHT + ASTEROID_WEIGHT));
                    if action > NOTHING_WEIGHT && action <= NOTHING_WEIGHT + SUN_WEIGHT {
                        //Send sunray to targets
                        let mut targets: Vec<ID> = Vec::new();
                        let mut misses = 0;

                        while targets.len() < RAY_BATCH_SIZE &&  misses < MAX_TARGET_TRIES {
                            let target = rand::random_range(start..=end) as ID;
                            if deads_lock.contains(&target) {
                                misses += 1;
                            } else {
                                targets.push(target);
                            }
                        }

                        let mut commlock = ai_queue.lock().expect("Failed to get lock");

                        for i in targets {
                            commlock.push_back(Command::SendSunray(i));
                            log::trace!("AI: RichardRandom Sent Sunray to planet ID: {}", i);
                        }
                        log::debug!("AI: RichardRandom Sent Sunrays to planets");
                    } else if !(action <= NOTHING_WEIGHT) {
                        //Send asteroid to target
                        let mut counter = 0;

                        let mut target: ID = rand::random_range(start..=end) as ID;

                        while counter < MAX_TARGET_TRIES && deads_lock.contains(&target) {
                            target = rand::random_range(start..=end) as ID;
                            counter += 1;
                        }

                        if counter < MAX_TARGET_TRIES {
                            let mut commslock = ai_queue.lock().expect("Failed to get lock");
                            commslock.push_back(Command::SendAsteroid(target));
                            log::debug!("AI: RichardRandom Sent Asteroid to planet ID: {}", target);
                        } else {
                            log::debug!("AI: RichardRandom ran out of targets");
                            runflag.store(false, Relaxed);
                        }
                    }

                    //Sleep time is 3 + offset(clamped between +1 and -1)
                    let sleep_offset: f32 = rand::rng().sample(StandardNormal);
                    let sleep_time = match sleep_offset {
                        ..=-1.0 => 2000,
                        1.0.. => 4000,
                        _ => (3000 + (sleep_offset * 1000.0).round() as i32) as u64,
                    };
                    log::trace!(
                        "AI: RichardRandom Sleeping for: {} milliseconds",
                        sleep_time
                    );
                    let sleep_millis = time::Duration::from_millis(sleep_time);
                    thread::sleep(sleep_millis);
                }
                log::debug!("AI: RichardRandom says bye bye");
            }
            _ => {
                log::error!("AI: Wrong arguments supplied to RichardRandom");
            }
        }
    }
}
