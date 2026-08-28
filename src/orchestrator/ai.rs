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
                const SUN_WEIGHT: i32 = 5;
                const ASTEROID_WEIGHT: i32 = 1;

                while runflag.load(Relaxed) {
                    let mut target: ID = rand::random_range(start..=end) as ID;

                    let lock = deads.lock().unwrap();

                    let mut counter = 0;

                    while lock.contains(&target) && counter < MAX_TARGET_TRIES {
                        target = rand::random_range(start..=end) as ID;
                        counter += 1;
                    }

                    if counter < MAX_TARGET_TRIES {
                        let action: i32 =
                            rand::random_range(1..=(NOTHING_WEIGHT + SUN_WEIGHT + ASTEROID_WEIGHT));
                        log::trace!("AI: RichardRandom rolled {} on {}", action, target);

                        let mut lock = ai_queue.lock().unwrap();
                        if action > NOTHING_WEIGHT && action <= NOTHING_WEIGHT + SUN_WEIGHT {
                            //Send sunray to target
                            lock.push_back(Command::SendSunray(target));
                            log::debug!("AI: RichardRandom Sent Sunray to planet ID: {}", target);
                        } else if !(action <= NOTHING_WEIGHT) {
                            //Send asteroid to target
                            lock.push_back(Command::SendAsteroid(target));
                            log::debug!("AI: RichardRandom Sent Asteroid to planet ID: {}", target);
                        }
                    } else {
                        log::debug!("AI: RichardRandom ran out of targets");
                        runflag.store(false, Relaxed);
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
