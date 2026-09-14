//! Presentation-only load control. Never changes engine time or canvas size.
use std::time::Duration;

const LEVELS: [(usize, u32); 5] = [
    (160_000, 30),
    (80_000, 24),
    (40_000, 15),
    (20_000, 10),
    (10_000, 5),
];

#[derive(Default)]
pub struct ImagePacer {
    level: usize,
    average_seconds: Option<f64>,
    stable_frames: u32,
}

impl ImagePacer {
    pub fn pixel_budget(&self) -> usize {
        LEVELS[self.level].0
    }

    pub fn interval(&self) -> Duration {
        let nominal = 1.0 / f64::from(LEVELS[self.level].1);
        let observed = self.average_seconds.unwrap_or(0.0) * 2.0;
        Duration::from_secs_f64(nominal.max(observed).clamp(1.0 / 30.0, 1.0))
    }

    pub fn observe(&mut self, preparation: Duration, transmission: Duration) {
        let cost = preparation
            .saturating_add(transmission)
            .as_secs_f64()
            .min(60.0);
        let average = self
            .average_seconds
            .map_or(cost, |old| old * 0.75 + cost * 0.25);
        self.average_seconds = Some(average);
        let nominal = 1.0 / f64::from(LEVELS[self.level].1);
        if average > nominal * 0.5 {
            self.level = (self.level + 1).min(LEVELS.len() - 1);
            self.stable_frames = 0;
        } else if average < nominal * 0.2 {
            self.stable_frames += 1;
            if self.stable_frames >= 60 {
                self.level = self.level.saturating_sub(1);
                self.stable_frames = 0;
            }
        } else {
            self.stable_frames = 0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overload_reduces_pixels_and_rate_with_bounded_recovery() {
        let mut pacer = ImagePacer::default();
        assert_eq!(pacer.pixel_budget(), 160_000);
        assert!(pacer.interval() >= Duration::from_secs_f64(1.0 / 30.0));
        for _ in 0..10 {
            pacer.observe(Duration::from_millis(100), Duration::from_millis(100));
        }
        assert_eq!(pacer.pixel_budget(), 10_000);
        assert!(pacer.interval() >= Duration::from_millis(399));
        for _ in 0..59 {
            pacer.observe(Duration::from_millis(1), Duration::ZERO);
        }
        assert_eq!(pacer.pixel_budget(), 10_000);
        for _ in 0..400 {
            pacer.observe(Duration::from_millis(1), Duration::ZERO);
        }
        assert_eq!(pacer.pixel_budget(), 160_000);
        assert!(pacer.interval() < Duration::from_millis(34));
    }

    #[test]
    fn extreme_costs_cannot_produce_unbounded_intervals() {
        let mut pacer = ImagePacer::default();
        pacer.observe(Duration::MAX, Duration::MAX);
        assert_eq!(pacer.interval(), Duration::from_secs(1));
    }
}
