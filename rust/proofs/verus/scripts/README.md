# Verus Script Plan

These scripts define CI entrypoints for proof checks and long-running suites.

| Script | Scope | Current behavior | Exit code policy |
| --- | --- | --- | --- |
| `run-proof-checks.sh` | Verus proof profiles (`quick`, `full`) | Runs real Verus checks when available, otherwise performs specification formatting validation. | `0` on success, `1` on proof failure, `2` on invalid inputs. |
| `run-long-suite.sh` | Long suites (`interleavings`, `soak`) | Validates target suite and prints planned cargo command. | `0` for valid placeholder runs, `2` for invalid inputs. |
| `sync-verus-guide-print.sh` | Verus guide snapshot sync | Rebuilds `reference/verus-guide-print.md` from local Verus upstream docs clone. | `0` on successful generation, `2` on missing upstream docs. |

## Implementation roadmap

1. Replace proof stubs in `specs/*.rs` with complete Verus model definitions.
2. Upload logs and summaries as workflow artifacts.
3. Replace long-suite placeholder command dispatch with concrete suites.
4. Split soak suite into shards for bounded runtime.

## Local usage

```bash
cd rust
proofs/verus/scripts/run-proof-checks.sh --profile quick
proofs/verus/scripts/run-proof-checks.sh --profile full
proofs/verus/scripts/run-long-suite.sh interleavings
proofs/verus/scripts/run-long-suite.sh soak
proofs/verus/scripts/sync-verus-guide-print.sh /tmp/verus-upstream
```

Set a custom Verus binary if needed:

```bash
VERUS_BIN=/path/to/verus proofs/verus/scripts/run-proof-checks.sh --profile quick
```
