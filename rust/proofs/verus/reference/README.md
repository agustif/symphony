# Verus Reference Snapshots

This directory stores locally vendored Verus documentation snapshots for offline use in proof work.

## Files
- `verus-guide-print.md`: printable, single-file snapshot generated from upstream Verus guide sources.

## Regenerate
1. Clone upstream Verus docs to a local path (example):
   - `git clone --depth 1 https://github.com/verus-lang/verus.git /tmp/verus-upstream`
2. Regenerate the printable snapshot:
   - `rust/proofs/verus/scripts/sync-verus-guide-print.sh /tmp/verus-upstream`
