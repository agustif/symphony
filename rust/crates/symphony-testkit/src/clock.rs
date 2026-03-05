use std::{
    sync::{Arc, Mutex},
    time::{Duration, SystemTime},
};

/// Trait for controllable time sources in tests
pub trait Clock: Send + Sync {
    /// Get current time in milliseconds
    fn now_ms(&self) -> u64;

    /// Set time to a specific value (for testing)
    fn set_ms(&self, target_ms: u64) -> u64;

    /// Advance time by a delta (for testing)
    fn advance_ms(&self, delta_ms: u64) -> u64;
}

/// Non-thread-safe deterministic clock for single-threaded tests
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

    pub fn into_thread_safe(self) -> ThreadSafeClock {
        ThreadSafeClock::new(self.now_ms)
    }
}

impl Default for DeterministicClock {
    fn default() -> Self {
        Self::new(0)
    }
}

/// Thread-safe wrapper around deterministic clock for concurrent tests
#[derive(Clone, Debug)]
pub struct ThreadSafeClock {
    inner: Arc<Mutex<DeterministicClock>>,
}

impl ThreadSafeClock {
    pub fn new(start_ms: u64) -> Self {
        Self {
            inner: Arc::new(Mutex::new(DeterministicClock::new(start_ms))),
        }
    }

    pub fn with_clock(clock: DeterministicClock) -> Self {
        Self {
            inner: Arc::new(Mutex::new(clock)),
        }
    }

    /// Get a handle that can be shared across threads
    pub fn handle(&self) -> ClockHandle {
        ClockHandle {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl Clock for ThreadSafeClock {
    fn now_ms(&self) -> u64 {
        let guard = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        guard.now_ms()
    }

    fn set_ms(&self, target_ms: u64) -> u64 {
        let mut guard = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        guard.set_ms(target_ms)
    }

    fn advance_ms(&self, delta_ms: u64) -> u64 {
        let mut guard = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        guard.advance_ms(delta_ms)
    }
}

impl Default for ThreadSafeClock {
    fn default() -> Self {
        Self::new(0)
    }
}

/// Lightweight handle to a ThreadSafeClock for sharing across threads
#[derive(Clone, Debug)]
pub struct ClockHandle {
    inner: Arc<Mutex<DeterministicClock>>,
}

impl ClockHandle {
    pub fn new(start_ms: u64) -> Self {
        Self {
            inner: Arc::new(Mutex::new(DeterministicClock::new(start_ms))),
        }
    }
}

impl Clock for ClockHandle {
    fn now_ms(&self) -> u64 {
        let guard = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        guard.now_ms()
    }

    fn set_ms(&self, target_ms: u64) -> u64 {
        let mut guard = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        guard.set_ms(target_ms)
    }

    fn advance_ms(&self, delta_ms: u64) -> u64 {
        let mut guard = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        guard.advance_ms(delta_ms)
    }
}

impl Default for ClockHandle {
    fn default() -> Self {
        Self::new(0)
    }
}

/// Real-time clock for non-test contexts
#[derive(Clone, Debug, Default)]
pub struct RealClock;

impl Clock for RealClock {
    fn now_ms(&self) -> u64 {
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or(Duration::ZERO)
            .as_millis() as u64
    }

    fn set_ms(&self, _target_ms: u64) -> u64 {
        panic!("Cannot set time on RealClock; use DeterministicClock for tests")
    }

    fn advance_ms(&self, _delta_ms: u64) -> u64 {
        panic!("Cannot advance time on RealClock; use DeterministicClock for tests")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn clock_starts_at_seed_and_advances_deterministically() {
        let mut clock = DeterministicClock::new(1_000);
        assert_eq!(clock.now_ms(), 1_000);
        assert_eq!(clock.advance_ms(250), 1_250);
        assert_eq!(clock.set_ms(500), 500);
    }

    #[test]
    fn thread_safe_clock_can_be_shared() {
        let clock = ThreadSafeClock::new(100);
        let handle = clock.handle();

        assert_eq!(handle.now_ms(), 100);
        assert_eq!(clock.now_ms(), 100);
    }

    #[test]
    fn thread_safe_clock_concurrent_access() {
        let clock = Arc::new(ThreadSafeClock::new(0));
        let handles: Vec<_> = (0..4).map(|_| clock.handle()).collect();

        // Spawn threads to concurrently advance time
        let mut join_handles = Vec::new();
        for handle in handles.into_iter() {
            join_handles.push(thread::spawn(move || {
                for _ in 0..10 {
                    handle.advance_ms(1);
                }
            }));
        }

        // Wait for all threads to complete
        for h in join_handles {
            h.join().unwrap();
        }

        // After all threads complete, clock should be 40ms (4 threads × 10ms each)
        assert_eq!(clock.now_ms(), 40);
    }

    #[test]
    fn clock_handle_is_clone_and_thread_safe() {
        let handle = ClockHandle::new(500);
        let handle2 = handle.clone();

        assert_eq!(handle.now_ms(), 500);
        assert_eq!(handle2.now_ms(), 500);

        handle.advance_ms(100);
        assert_eq!(handle2.now_ms(), 600);
    }

    #[test]
    fn deterministic_clock_converts_to_thread_safe() {
        let mut clock = DeterministicClock::new(1234);
        clock.advance_ms(100);

        let thread_safe = clock.into_thread_safe();
        assert_eq!(thread_safe.now_ms(), 1334);
    }
}
