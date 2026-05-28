# stellar-osmosis

Bootstraps and manages a local Osmosis appchain (`localosmosis`) for stellar-ibc
devnets. This is the stellar-ibc port of caribic's `caribic chain start --chain
osmosis --network local`: instead of downloading the Osmosis source and building
`osmosisd` from a Dockerfile, it runs the prebuilt `osmolabs/osmosis:<ver>-alpine`
image as a service inside the repo-root `docker-compose.yml`, initialising genesis
through a mounted entrypoint script.

## Layout

| File | Role |
|---|---|
| `assets/setup.sh` | Container entrypoint. On first boot it runs `osmosisd init`, rewrites genesis/app/config (denoms → `uosmo`/`uion`, CORS, REST + gRPC on `0.0.0.0`, permissionless wasm + CL pools), funds genesis accounts, and `osmosisd start`. Mounted into the `osmosis` service. |
| `src/lifecycle.rs` | Locates the repo `docker-compose.yml` and drives `docker compose --profile osmosis up/down`. Resets `~/.osmosisd-local` for a fresh start unless `--stateful`. |
| `src/health.rs` | Polls `http://127.0.0.1:26658/status` until `latest_block_height > 0`. |
| `src/main.rs` | CLI: `start [--stateful]`, `stop`, `health`. |

The `osmosis` service definition lives in the repo-root `docker-compose.yml`
under the `osmosis` (and `local`) compose profiles.

## Usage

```sh
make start-osmosis            # fresh local chain, wait for first block
make start-osmosis-stateful   # reuse existing ~/.osmosisd-local state
make health-osmosis
make stop-osmosis

# or directly
cargo run -p stellar-osmosis -- start [--stateful]
cargo run -p stellar-osmosis -- stop
cargo run -p stellar-osmosis -- health

# or straight through docker compose
docker compose --profile osmosis up -d osmosis
```

## Endpoints

| Endpoint | Host | Container |
|---|---|---|
| Tendermint RPC / websocket | `http://127.0.0.1:26658` | `26657` |
| REST (LCD) | `http://127.0.0.1:1318` | `1317` |
| gRPC | `127.0.0.1:9094` | `9090` |
| gRPC-web | `127.0.0.1:9091` | `9091` |

Chain id `localosmosis`, account prefix `osmo`, gas denom `uosmo`. These match
`COSMOS_*` in `.env` and the `localosmosis` chain block in `ci/hermes-config.toml`.

The validator (`val`) and `pools` key mnemonics used for the funded genesis
accounts are in `assets/setup.sh`; import them with
`osmosisd keys add <name> --recover` or `hermes keys add` to fund a relayer.

## Config

| Env var | Default | Effect |
|---|---|---|
| `OSMOSIS_VERSION` | `31.0.3` | `osmolabs/osmosis` image tag (the `-alpine` variant is used). |
| `OSMOSIS_LOCAL_GENESIS_TIME` | `2025-12-31T23:59:00Z` | `genesis_time` written into genesis. |
| `STELLAR_IBC_COMPOSE_FILE` | _(auto)_ | Override the compose file; otherwise the nearest `docker-compose.yml` above the cwd is used. |
