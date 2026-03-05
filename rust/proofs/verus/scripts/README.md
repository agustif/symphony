# Verus Script Plan

These scripts define CI entrypoints for proof checks and long-running suites.

| Script | Scope | Current behavior | Exit code policy |
| --- | --- | --- | --- |
| `run-proof-checks.sh` | Verus proof profiles (`quick`, `full`) | Validates arguments and prints planned Verus invocations. | `0` for valid placeholder runs, `2` for invalid inputs. |
| `run-long-suite.sh` | Long suites (`interleavings`, `soak`) | Validates target suite and prints planned cargo command. | `0` for valid placeholder runs, `2` for invalid inputs. |

## Implementation roadmap

1. Add concrete Verus spec files under `proofs/verus/specs/`.
2. Replace planned command echoes with real invocations.
3. Upload logs and summaries as workflow artifacts.
4. Split soak suite into shards for bounded runtime.

## Local usage

```bash
cd rust
proofs/verus/scripts/run-proof-checks.sh --profile quick
proofs/verus/scripts/run-proof-checks.sh --profile full
proofs/verus/scripts/run-long-suite.sh interleavings
proofs/verus/scripts/run-long-suite.sh soak
```
