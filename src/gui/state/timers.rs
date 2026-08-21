use std::time::{Duration, Instant};

pub struct PollingTimers {
    pub planet_snapshot_timer: Instant,
    pub planet_snapshot_interval: Duration,
    pub explorer_snapshot_timer: Instant,
    pub explorer_snapshot_interval: Duration,
    pub explorer_position_timer: Instant,
    pub explorer_position_interval: Duration,
}

impl PollingTimers {
    pub fn new(game_step: u64) -> Self {
        Self {
            planet_snapshot_timer: Instant::now(),
            planet_snapshot_interval: Duration::from_millis(game_step),
            explorer_snapshot_timer: Instant::now(),
            explorer_snapshot_interval: Duration::from_millis(game_step),
            explorer_position_timer: Instant::now(),
            explorer_position_interval: Duration::from_millis(game_step),
        }
    }

    /// Returns `true` (and resets the timer) when it is time to poll planet snapshots.
    pub fn should_poll_planet_snapshots(&mut self) -> bool {
        if self.planet_snapshot_timer.elapsed() >= self.planet_snapshot_interval {
            self.planet_snapshot_timer = Instant::now();
            true
        } else {
            false
        }
    }

    /// Returns `true` (and resets the timer) when it is time to poll explorer snapshots.
    pub fn should_poll_explorer_snapshots(&mut self) -> bool {
        if self.explorer_snapshot_timer.elapsed() >= self.explorer_snapshot_interval {
            self.explorer_snapshot_timer = Instant::now();
            true
        } else {
            false
        }
    }

    /// Returns `true` (and resets the timer) when it is time to poll explorer positions.
    pub fn should_poll_explorer_positions(&mut self) -> bool {
        if self.explorer_position_timer.elapsed() >= self.explorer_position_interval {
            self.explorer_position_timer = Instant::now();
            true
        } else {
            false
        }
    }

    /// How long until the next timer fires (minimum across all three).
    pub fn time_until_next_poll(&self) -> Duration {
        let remaining = |timer: &Instant, interval: &Duration| -> Duration {
            interval.saturating_sub(timer.elapsed())
        };
        remaining(&self.planet_snapshot_timer, &self.planet_snapshot_interval)
            .min(remaining(
                &self.explorer_snapshot_timer,
                &self.explorer_snapshot_interval,
            ))
            .min(remaining(
                &self.explorer_position_timer,
                &self.explorer_position_interval,
            ))
    }
}
