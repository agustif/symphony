use symphony_domain::{Event, IssueId};

pub fn interleave_preserving_order(
    left: &[Event],
    right: &[Event],
    limit: usize,
) -> Vec<Vec<Event>> {
    if limit == 0 {
        return Vec::new();
    }
    let mut results = Vec::new();
    let mut prefix = Vec::with_capacity(left.len() + right.len());
    collect_interleavings(left, right, 0, 0, &mut prefix, &mut results, limit);
    results
}

fn collect_interleavings(
    left: &[Event],
    right: &[Event],
    left_index: usize,
    right_index: usize,
    prefix: &mut Vec<Event>,
    results: &mut Vec<Vec<Event>>,
    limit: usize,
) {
    if results.len() >= limit {
        return;
    }
    if left_index == left.len() && right_index == right.len() {
        results.push(prefix.clone());
        return;
    }
    if left_index < left.len() {
        prefix.push(left[left_index].clone());
        collect_interleavings(
            left,
            right,
            left_index + 1,
            right_index,
            prefix,
            results,
            limit,
        );
        prefix.pop();
    }
    if right_index < right.len() {
        prefix.push(right[right_index].clone());
        collect_interleavings(
            left,
            right,
            left_index,
            right_index + 1,
            prefix,
            results,
            limit,
        );
        prefix.pop();
    }
}

pub fn deterministic_event_stream(issue_ids: &[IssueId], steps: usize, seed: u64) -> Vec<Event> {
    if issue_ids.is_empty() || steps == 0 {
        return Vec::new();
    }

    let mut generator_state = seed;
    let mut events = Vec::with_capacity(steps);
    for _ in 0..steps {
        generator_state = next_lcg(generator_state);
        let issue_index = (generator_state as usize) % issue_ids.len();
        let issue = issue_ids[issue_index].clone();

        generator_state = next_lcg(generator_state);
        let event = match generator_state % 3 {
            0 => Event::Claim(issue),
            1 => Event::MarkRunning(issue),
            _ => Event::Release(issue),
        };
        events.push(event);
    }
    events
}

fn next_lcg(value: u64) -> u64 {
    value
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407)
}

pub fn release_events(issue_ids: &[IssueId]) -> Vec<Event> {
    issue_ids.iter().cloned().map(Event::Release).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::issue_id;

    #[test]
    fn interleaving_respects_limit() {
        let issue = issue_id("SYM-1");
        let left = vec![Event::Claim(issue.clone()), Event::MarkRunning(issue)];
        let right = vec![Event::Release(issue_id("SYM-1"))];
        let schedules = interleave_preserving_order(&left, &right, 2);
        assert_eq!(schedules.len(), 2);
    }

    #[test]
    fn deterministic_stream_is_stable() {
        let pool = vec![issue_id("SYM-1"), issue_id("SYM-2"), issue_id("SYM-3")];
        assert_eq!(
            deterministic_event_stream(&pool, 12, 7),
            deterministic_event_stream(&pool, 12, 7)
        );
    }
}
