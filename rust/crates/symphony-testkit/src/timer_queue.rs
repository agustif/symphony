use crate::DeterministicClock;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeterministicTimer<T> {
    pub deadline_ms: u64,
    pub sequence: u64,
    pub payload: T,
}

#[derive(Clone, Debug)]
pub struct DeterministicTimerQueue<T> {
    scheduled: Vec<DeterministicTimer<T>>,
    next_sequence: u64,
}

impl<T> DeterministicTimerQueue<T> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.scheduled.len()
    }

    pub fn is_empty(&self) -> bool {
        self.scheduled.is_empty()
    }

    pub fn schedule_at(&mut self, deadline_ms: u64, payload: T) {
        let event = DeterministicTimer {
            deadline_ms,
            sequence: self.next_sequence,
            payload,
        };
        self.next_sequence = self.next_sequence.saturating_add(1);
        self.scheduled.push(event);
    }

    pub fn schedule_after(&mut self, clock: &DeterministicClock, delay_ms: u64, payload: T) {
        let deadline_ms = clock.now_ms().saturating_add(delay_ms);
        self.schedule_at(deadline_ms, payload);
    }

    pub fn next_deadline_ms(&self) -> Option<u64> {
        self.scheduled.iter().map(|event| event.deadline_ms).min()
    }

    pub fn pop_due(&mut self, now_ms: u64) -> Vec<T> {
        let mut due = Vec::new();
        let mut pending = Vec::new();

        for event in self.scheduled.drain(..) {
            if event.deadline_ms <= now_ms {
                due.push(event);
            } else {
                pending.push(event);
            }
        }

        due.sort_by_key(|event| (event.deadline_ms, event.sequence));
        self.scheduled = pending;

        due.into_iter().map(|event| event.payload).collect()
    }
}

impl<T> Default for DeterministicTimerQueue<T> {
    fn default() -> Self {
        Self {
            scheduled: Vec::new(),
            next_sequence: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pops_due_in_deadline_then_sequence_order() {
        let clock = DeterministicClock::new(10);
        let mut queue = DeterministicTimerQueue::new();
        queue.schedule_after(&clock, 5, "late");
        queue.schedule_at(12, "first");
        queue.schedule_at(12, "second");

        let due = queue.pop_due(12);
        assert_eq!(due, vec!["first", "second"]);
        assert_eq!(queue.len(), 1);
        assert_eq!(queue.next_deadline_ms(), Some(15));
    }
}
