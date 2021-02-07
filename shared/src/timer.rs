use serde::{Deserialize, Serialize};

use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct Timer {
    duration: Duration,
    start: Instant,
}

impl Timer {
    pub fn new(duration: Duration) -> Self {
        Timer {
            start: Instant::now(),
            duration,
        }
    }

    pub fn reset(&mut self) {
        self.start = Instant::now();
    }

    pub fn is_finished(&self) -> bool {
        Instant::now().saturating_duration_since(self.start) > self.duration
    }

    pub fn percentage_done(&self) -> f32 {
        if self.is_finished() {
            return 1.;
        }
        
        Instant::now().duration_since(self.start).as_secs_f32() / self.duration.as_secs_f32()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimerSimple {
    duration: f32,
    current_duration: f32,
}

impl TimerSimple {
    pub fn new(duration: f32) -> Self {
        Self {
            duration,
            current_duration: 0.,
        }
    }

    pub fn finish(&mut self) {
        self.current_duration = self.duration;
    }

    pub fn is_finished(&self) -> bool {
        self.current_duration >= self.duration
    }

    pub fn reset(&mut self) {
        self.current_duration = 0.;
    }

    pub fn update(&mut self, time: f32) {
        self.current_duration += time;
    }

    pub fn percentage_done(&self) -> f32 {
        (self.current_duration / self.duration).min(1.)
    }
}
