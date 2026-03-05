# Symphony Rust Architecture Overview

The Rust runtime is split into a deterministic domain/reducer core plus pluggable adapter crates.
This keeps failure-prone IO concerns out of orchestration invariants.

## Layering
1. Domain + reducer (`symphony-domain`)
2. Runtime scheduler (`symphony-runtime`)
3. Adapter boundaries (`symphony-tracker*`, `symphony-workspace`, `symphony-agent-protocol`)
4. Operator surfaces (`symphony-http`, `symphony-observability`, `symphony-cli`)
