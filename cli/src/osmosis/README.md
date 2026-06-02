# osmosis (stellaribc module)

Bootstraps and manages a local Osmosis appchain (`localosmosis`) for stellar-ibc
devnets. It runs the prebuilt `osmolabs/osmosis:<ver>-alpine` image as the
`osmosis` service in the repo-root `docker-compose.yml` and builds genesis from
scratch through a mounted entrypoint script — no Dockerfile, no source build.

Driven by `stellaribc osmosis` (start/stop/status); the chain config is minimal
and IBC-tailored: it starts from `osmosisd init` defaults and overrides only what
relaying and the 08-wasm light client need — `uosmo` as bond/mint/fee denom, a
funded validator + relayer account, and a short gov voting period + tiny deposit
so the `stellaribc contracts upload-wasm` proposal lands deterministically.

## Layout

| File | Role |
|---|---|
| `assets/default-config.json` | Declarative chain config: chain id, moniker, genesis time, the `val`/`relayer` key mnemonics + funded balances, the gentx, and the `genesis`/`app`/`config` override lists (each entry a `{path, type, value}` applied with `dasel`). Edit this, not the script. |
| `assets/setup.sh` | Container entrypoint. On first boot it `apk add jq dasel`, runs `osmosisd init`, applies every override from `default-config.json`, recovers + funds each key, builds the gentx, then `osmosisd start`. Data-driven — holds no hardcoded chain values. |
| `config.rs` | `OsmosisConfig::from_env()` — local/testnet presets overridable via `COSMOS_*`. |
| `mod.rs` | `start`/`stop`/`status` — drive `docker compose --profile osmosis up/down`; `start --fresh` wipes `~/.osmosisd-local` first. |

## Usage

```sh
stellaribc osmosis start            # start the devnet, wait for first block
stellaribc osmosis start --fresh    # wipe ~/.osmosisd-local, then start clean
stellaribc osmosis status
stellaribc osmosis stop

# or straight through docker compose
docker compose --profile osmosis up -d osmosis
```

## Endpoints

| Endpoint | Host | Container |
|---|---|---|
| Tendermint RPC / websocket | `http://127.0.0.1:26657` | `26657` |
| REST (LCD) | `http://127.0.0.1:1318` | `1317` |
| gRPC | `127.0.0.1:9094` | `9090` |

Chain id `localosmosis`, account prefix `osmo`, gas denom `uosmo` — these match
`COSMOS_*` in `.env` and the `localosmosis` chain block in `hermes-config.toml`.
Two keys are recovered into genesis: `val` (validator) and `relayer` (a funded
account for Hermes); both mnemonics live in `assets/default-config.json`.
