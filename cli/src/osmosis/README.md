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
| `assets/default-config.json` | Declarative chain config: chain id, moniker, genesis time, the `val`/`relayer` funded balances, the gentx, and the `genesis`/`app`/`config` override lists (each entry a `{path, type, value}` applied with `dasel`). Holds **no secrets** — the account mnemonics come from env. Edit this, not the script. |
| `assets/setup.sh` | Container entrypoint. On first boot it `apk add jq dasel`, runs `osmosisd init`, applies every override from `default-config.json`, recovers each account from its env mnemonic (`COSMOS_VALIDATOR_MNEMONIC` / `COSMOS_RELAYER_MNEMONIC`) and funds it, builds the gentx, then `osmosisd start`. |
| `config.rs` | `OsmosisConfig::from_env()` — local/testnet presets overridable via `COSMOS_*`. |
| `mod.rs` | `start`/`stop`/`status`/`keygen` — drive `docker compose --profile osmosis up/down`; `start --fresh` wipes `~/.osmosisd-local` first; `keygen` generates the validator + relayer accounts and writes their mnemonics **and** matching hex signer keys to `.env`. |

## Usage

```sh
stellaribc osmosis keygen           # generate validator + relayer mnemonics → .env (skips ones already set; --force to regenerate)
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
Two accounts are recovered into genesis from env mnemonics: `val` (validator,
`COSMOS_VALIDATOR_MNEMONIC`) and `relayer` (a funded account for Hermes,
`COSMOS_RELAYER_MNEMONIC`). Each account also has a hex signer key the api uses
for the gov store-code flow — `COSMOS_FUNDER_PRIVATE_KEY` (= validator) and
`COSMOS_PROPOSER_PRIVATE_KEY` (= relayer). `stellaribc osmosis keygen` generates
all four together so the mnemonic↔hex pairs never drift; don't set them by hand.

