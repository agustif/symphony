# Verus Reference Snapshots

This directory stores locally vendored Verus documentation snapshots for offline use in proof work.

## Files
- `verus-guide-print.md`: printable, single-file snapshot generated from upstream Verus guide sources.
- `verus-upstream-commit.txt`: pinned upstream commit used for reproducible snapshot verification.

## Regenerate
1. Clone upstream Verus docs to a local path (example):
   - `git clone --depth 1 https://github.com/verus-lang/verus.git /tmp/verus-upstream`
2. Regenerate the printable snapshot:
   - `rust/proofs/verus/scripts/sync-verus-guide-print.sh /tmp/verus-upstream`
3. Pin the exact upstream commit used for regeneration:
   - `git -C /tmp/verus-upstream rev-parse HEAD > rust/proofs/verus/reference/verus-upstream-commit.txt`
4. Verify snapshot reproducibility against the pinned commit:
   - `rust/proofs/verus/scripts/verify-verus-guide-print.sh`
