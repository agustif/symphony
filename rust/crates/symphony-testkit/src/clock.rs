#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct DeterministicClock {
    now_ms: u64,
}

impl DeterministicClock {
    pub fn new(start_ms: u64) -> Self {
        Self { now_ms: start_ms }
    }

    pub fn now_ms(&self) -> u64 {
        self.now_ms
    }

    pub fn set_ms(&mut self, target_ms: u64) -> u64 {
        self.now_ms = target_ms;
        self.now_ms
    }

    pub fn advance_ms(&mut self, delta_ms: u64) -> u64 {
        self.now_ms = self.now_ms.saturating_add(delta_ms);
        self.now_ms
    }
}

impl Default for DeterministicClock {
    fn default() -> Self {
        Self::new(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clock_starts_at_seed_and_advances_deterministically() {
        let mut clock = DeterministicClock::new(1_000);
        assert_eq!(clock.now_ms(), 1_000);
        assert_eq!(clock.advance_ms(250), 1_250);
        assert_eq!(clock.set_ms(500), 500);
    }
}
