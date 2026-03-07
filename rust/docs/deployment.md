# Symphony Container Deployment Guide

This repository now ships the container assets below:

| File | Purpose |
| --- | --- |
| `/Users/af/symphony/.dockerignore` | Keeps build context small and excludes local build outputs. |
| `/Users/af/symphony/Dockerfile.rust` | Builds the Rust CLI image and exposes the JSON API on port `8080`. |
| `/Users/af/symphony/Dockerfile.elixir` | Builds the Elixir escript image and exposes the JSON API on port `8080`. |
| `/Users/af/symphony/docker-compose.yml` | Runs either implementation through explicit Compose profiles. |

## What The Images Include

Both images are conservative runtime images.

- They include `bash`, `git`, `openssh-client`, `curl`, `ca-certificates`, and `tini`.
- They do not bundle the Codex CLI for you.
- They expect a `WORKFLOW.md` bind mount at `/srv/symphony/WORKFLOW.md`.
- They ship an internal healthcheck against `GET /api/v1/state` on port `8080`.

## Workflow Requirements

The containers only provide Symphony itself. Your mounted workflow still controls tracker access, hooks, and Codex startup.

### Rust workflow requirements

- The Compose profile passes `--workspace-root /var/lib/symphony/workspaces`.
- The Compose profile passes `--logs-root /var/log/symphony`.
- The Compose profile assumes the Rust CLI supports `--bind 0.0.0.0`.
- `--port 8080` is passed explicitly, so the healthcheck and port mapping stay aligned.

### Elixir workflow requirements

The Elixir CLI has no `--bind` flag, so the workflow must open the HTTP listener itself.

Use these values in the mounted `WORKFLOW.md`:

```yaml
server:
  host: 0.0.0.0
  port: 8080
workspace:
  root: /var/lib/symphony/workspaces
```

If `server.host` stays at the default loopback value, the container healthcheck still passes, but host port publishing will not be reachable from outside the container.

## Build The Images

Build the Rust image:

```bash
docker build --file Dockerfile.rust --tag symphony-rust:local .
```

Build the Elixir image:

```bash
docker build --file Dockerfile.elixir --tag symphony-elixir:local .
```

## Run With Docker Compose

The Compose file is profile-based so you choose one implementation at a time.

Export the required variables:

```bash
export LINEAR_API_KEY=lin_api_xxxxxxxxxxxx
export SYMPHONY_WORKFLOW_PATH="$PWD/WORKFLOW.md"
```

Start the Rust implementation:

```bash
docker compose --profile rust up --build -d
```

Start the Elixir implementation:

```bash
docker compose --profile elixir up --build -d
```

Tail logs:

```bash
docker compose logs -f symphony-rust
docker compose logs -f symphony-elixir
```

Stop and remove containers:

```bash
docker compose down
```

### Compose Defaults

The shipped Compose file uses these defaults:

| Setting | Rust profile | Elixir profile |
| --- | --- | --- |
| Host port | `8080` via `SYMPHONY_RUST_PORT` | `8081` via `SYMPHONY_ELIXIR_PORT` |
| Workflow mount | `${SYMPHONY_WORKFLOW_PATH}` | `${SYMPHONY_WORKFLOW_PATH}` |
| State volume | `symphony-rust-state` | `symphony-elixir-state` |
| Log volume | `symphony-rust-logs` | `symphony-elixir-logs` |
| Healthcheck | `GET http://127.0.0.1:8080/api/v1/state` | `GET http://127.0.0.1:8080/api/v1/state` |

Override the exposed host ports if needed:

```bash
export SYMPHONY_RUST_PORT=18080
export SYMPHONY_ELIXIR_PORT=18081
```

## Run The Images Directly

Rust:

```bash
docker run \
  --detach \
  --name symphony-rust \
  --publish 8080:8080 \
  --env LINEAR_API_KEY="$LINEAR_API_KEY" \
  --volume "$PWD/WORKFLOW.md:/srv/symphony/WORKFLOW.md:ro" \
  --volume symphony-rust-state:/var/lib/symphony \
  --volume symphony-rust-logs:/var/log/symphony \
  symphony-rust:local
```

Elixir:

```bash
docker run \
  --detach \
  --name symphony-elixir \
  --publish 8081:8080 \
  --env LINEAR_API_KEY="$LINEAR_API_KEY" \
  --volume "$PWD/WORKFLOW.md:/srv/symphony/WORKFLOW.md:ro" \
  --volume symphony-elixir-state:/var/lib/symphony \
  --volume symphony-elixir-logs:/var/log/symphony \
  symphony-elixir:local
```

## Healthchecks

Both Dockerfiles and the Compose file use the same probe:

```bash
curl --fail --silent --show-error http://127.0.0.1:8080/api/v1/state >/dev/null
```

Probe settings:

| Setting | Value |
| --- | --- |
| Interval | `30s` |
| Timeout | `5s` |
| Start period | `20s` |
| Retries | `5` |

## Operational Notes

- The Rust image renames the built `symphony-cli` binary to `/usr/local/bin/symphony`.
- The Elixir image runs the checked-in escript built from `mix escript.build`.
- The default container user is `symphony`.
- The working directory is `/srv/symphony`.
- Logs are written under `/var/log/symphony` when you use the shipped commands.
- The images are intentionally explicit. If you need Codex or additional hook tooling inside the container, extend the image in a downstream Dockerfile.
