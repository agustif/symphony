# Verus Script Plan

These scripts define CI entrypoints for proof checks and reference-snapshot guards.

| Script | Scope | Current behavior | Exit code policy |
| --- | --- | --- | --- |
| `install-verus.sh` | Toolchain bootstrap | Installs pinned Verus release binary and links `verus`/`cargo-verus` into `~/.local/bin`. | `0` on success, `2` on unsupported platform. |
| `run-proof-checks.sh` | Verus proof profiles (`quick`, `full`) | Runs real Verus checks when available, otherwise performs specification formatting validation. The full profile now includes `agent_update_safety.rs`. | `0` on success, `1` on proof failure, `2` on invalid inputs. |
| `run-long-suite.sh` | Long suites (`interleavings`, `soak`) | Executes the real `symphony-testkit` long-running validation suites. | `0` on success, non-zero on cargo failure, `2` on invalid inputs. |
| `sync-verus-guide-print.sh` | Verus guide snapshot sync | Rebuilds `reference/verus-guide-print.md` from local Verus upstream docs clone. | `0` on successful generation, `2` on missing inputs. |
| `verify-verus-guide-print.sh` | Snapshot reproducibility guard | Rebuilds printable guide from pinned commit and checks byte-for-byte parity. | `0` on match, `1` on stale snapshot, `2` on missing prerequisites. |

## Implementation roadmap

1. Replace the remaining auto-discharged proof bodies with more explicit lemma structure where the model is non-trivial.
2. Upload logs and summaries as workflow artifacts.
3. Keep long-suite target names aligned with the actual `symphony-testkit` suites.
4. Split soak suite into shards for bounded runtime.

## Local usage

```bash
cd rust
proofs/verus/scripts/install-verus.sh
proofs/verus/scripts/run-proof-checks.sh --profile quick
proofs/verus/scripts/run-proof-checks.sh --profile full
proofs/verus/scripts/run-long-suite.sh interleavings
proofs/verus/scripts/run-long-suite.sh soak
proofs/verus/scripts/sync-verus-guide-print.sh /tmp/verus-upstream
proofs/verus/scripts/sync-verus-guide-print.sh /tmp/verus-upstream --output /tmp/verus-guide-print.md
proofs/verus/scripts/verify-verus-guide-print.sh
```

Set a custom Verus binary if needed:

```bash
VERUS_BIN=/path/to/verus proofs/verus/scripts/run-proof-checks.sh --profile quick
```
