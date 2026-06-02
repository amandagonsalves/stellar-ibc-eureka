# `stellaribc` — Stellar↔Cosmos IBC orchestrator CLI

`stellar-ibc-cli` (binary **`stellaribc`**) is the single entry point for the
Stellar↔Cosmos IBC v2 bridge. It brings the stack up, builds/pushes images,
deploys the Soroban contracts, uploads the light client, creates clients,
registers counterparties, and reports status — driving **docker**, the
**`stellar`** CLI, and **`stellar-api`** directly. There are no shell scripts.

It lives at the repo root (`stellar-ibc/cli/`) as a workspace member.

## How it works

- **Repo-root discovery** — walks up from the current directory for
  `docker-compose.yml` (override with `STELLAR_IBC_ROOT`). All paths and
  `docker compose` calls are resolved against that root, so the binary works
  from anywhere.
- **Config** — reads `stellar-ibc/.env` (shell env wins, matching the rest of
  the stack). See [Configuration](#configuration).
- **Health probes** — native HTTP (cosmos REST, api `/health`) and TCP (gateway
  gRPC) checks; no external tools required for `check` / `status`.

---

## Install / run

```sh
# run in place (from the repo root)
cargo run -p stellar-ibc-cli -- <command>

# install the `stellaribc` binary (either of these)
cargo run -p stellar-ibc-cli -- install   # self-install to the cargo bin dir
cargo install --path cli

# then, from anywhere:
stellaribc <command>
```

Get help for any command or group:

```sh
stellaribc --help
stellaribc <group> --help
stellaribc <group> <command> --help
```

---

## Command overview

| Group | Commands |
|---|---|
| ops | `install` · `check` · `status` · `up` · `down` · `start` |
| `osmosis` | `start [--fresh]` · `stop` · `status` · `keygen [--force]` |
| `clients` | `cosmos` · `stellar` · `counterparty` · `list` |
| `hermes` | `start` · `stop` · `restart` · `keys-import` |
| `gateway` | `start` · `stop` · `restart` · `query` |
| `api` | `start` · `stop` · `restart` |
| `contracts` | `build` · `upload` · `deploy` · `invoke` · `deploy-all` · `upload-wasm` |
| `tx` | `clients` · `msg` · `query` |

---

## Top-level (ops) commands

### `stellaribc install`
Installs the `stellaribc` binary to the cargo bin dir (`cargo install --path cli
--force`) and reports whether that dir is on your `PATH`.

### `stellaribc check`
Checks prerequisites and configuration, then probes service health. Reports:
toolchain (`docker`, `stellar`, `cargo`), `.env` presence, key config vars
(`STELLAR_SIGNING_KEY`, `ROUTER_CONTRACT_ADDRESS`, …), and the live state of
`localosmosis`, `stellar-api`, and the gateway gRPC port. Always exits 0.

### `stellaribc status`
Probes chains/services, prints the configured endpoints, the deployed contract
ids (from `.env`), and the clients created on the router (`GET /stellar/clients`).

### `stellaribc up [--cosmos | --stellar]`
Brings the stack up via `docker compose up -d`.

| Flag | Effect |
|---|---|
| _(none)_ | start `osmosis` + `api` + `gateway` |
| `--cosmos` | start only `osmosis` |
| `--stellar` | start only `api` + `gateway` |

### `stellaribc down [--volumes]`
Stops the stack via `docker compose down`.

| Flag | Effect |
|---|---|
| `--volumes` | also remove named volumes (wipes chain + hermes-key state) |

### `stellaribc start`
Full start: pull images → start `osmosis` → start `api` + `gateway` → deploy
contracts → upload the light-client wasm → import relayer keys. Each step is
skippable; the chain/service steps are idempotent (probe first, start if down).

| Flag | Effect |
|---|---|
| `--skip-images` | skip building the docker images |
| `--skip-contracts` | skip the Soroban contract deploy |
| `--skip-wasm` | skip the light-client-wasm upload |
| `--skip-keys` | skip importing the hermes relayer keys |
| `--force-redeploy` | redeploy contracts even if `ROUTER_CONTRACT_ADDRESS` is set |

---

## `osmosis` — local Cosmos devnet

Lifecycle for the `localosmosis` chain (the `osmosis` compose service). On
`COSMOS_NETWORK=testnet` the start/stop become reachability checks / no-ops.

| Command | Flags | What it does |
|---|---|---|
| `start` | `--fresh` | `docker compose up -d osmosis` + wait for the first block; `--fresh` wipes `~/.osmosisd-local` and rebuilds genesis |
| `stop` | — | `docker compose stop osmosis` |
| `status` | — | probe RPC + print network/endpoints |
| `keygen` | `--force` | generate the validator + relayer accounts and write **all four** vars to `.env` — the two mnemonics plus the matching hex signer keys (skips when the mnemonic is already set; `--force` regenerates). Uses the `osmolabs/osmosis` image — no local `osmosisd` needed |

The two genesis accounts each map to two `.env` vars:

| Account | Mnemonic var | Hex signer var | Used by |
|---|---|---|---|
| validator | `COSMOS_VALIDATOR_MNEMONIC` | `COSMOS_FUNDER_PRIVATE_KEY` | genesis validator; the api's gov **funder/voter** (votes with stake) |
| relayer | `COSMOS_RELAYER_MNEMONIC` | `COSMOS_PROPOSER_PRIVATE_KEY` | Hermes relayer key; the api's gov **proposer** |

`docker-compose.yml` passes the mnemonics into the osmosis container and
`setup.sh` recovers + funds those accounts at genesis; the api signs gov
messages (store-code proposal during `upload-wasm`) with the hex keys. Because
the hex key is just the mnemonic's private key, `keygen` writes both together so
they never drift — `keygen` is the one-command way to populate all four on a
fresh checkout.

---

## `clients` — client lifecycle

### `stellaribc clients cosmos [--force]`
Create the Cosmos (Tendermint) client on Stellar. Probes the gateway + Cosmos
RPC, runs `hermes create client --host-chain stellar-testnet --reference-chain
localosmosis`, extracts the `07-tendermint-N` id, and writes `COSMOS_CLIENT_ID`
to `.env`. `--force` creates another even if already set.

### `stellaribc clients stellar [--force]`
Create the Stellar (08-wasm) client on Cosmos. Requires `wasm_checksum_hex` to
be set in the hermes config (run `contracts upload-wasm` first). Runs `hermes
create client --host-chain localosmosis --reference-chain stellar-testnet`,
extracts the `08-wasm-N` id, and writes `STELLAR_CLIENT_ID`.

### `stellaribc clients counterparty <stellar | cosmos>`
Register a counterparty on the given side. *Pending* — blocked on migrating the
gateway's `register_counterparty` RPC to prepare→sign→submit; prints a "not
wired yet" notice until then.

### `stellaribc clients list`
Lists the clients created on the Stellar router (`GET /stellar/clients`),
grouped by `client_type`.

---

> The CLI only **pulls and runs** images — it never builds or pushes them.
> Building + pushing images is done via the Makefile
> (`make build SERVICE=<gateway|hermes|api>` / `make push SERVICE=<…>`). Config
> only needs each image's name/tag/registry, which `stellaribc status` shows
> under **Images**.

## `hermes` — relayer

| Command | Flags | What it does |
|---|---|---|
| `start` | `--pull` | `docker compose up -d hermes` (the relayer container); `--pull` fetches the latest image first |
| `stop` | — | `docker compose stop hermes` |
| `restart` | `--pull` | `docker compose restart hermes`, or with `--pull`: pull → `up -d --force-recreate` |
| `keys-import` | — | import the cosmos relayer mnemonic + `STELLAR_SIGNING_KEY` into the `hermes-keys` volume (one-shot `docker compose run`) |

The relayer's Stellar key must equal the router admin key (`STELLAR_SIGNING_KEY`).

---

## `gateway` — gateway service

| Command | Flags | What it does |
|---|---|---|
| `start` | `--pull` | `docker compose up -d gateway` (`--pull` fetches the latest image first) |
| `stop` | — | `docker compose stop gateway` |
| `restart` | `--pull` | `docker compose restart gateway`, or with `--pull`: pull → `up -d --force-recreate` |
| `query` | — | direct gateway gRPC reads — *pending* |

---

## `api` — api service

| Command | Flags | What it does |
|---|---|---|
| `start` | `--pull` | `docker compose up -d api` (`--pull` fetches the latest image first) |
| `stop` | — | `docker compose stop api` |
| `restart` | `--pull` | `docker compose restart api`, or with `--pull`: pull → `up -d --force-recreate` |

---

## `contracts` — Soroban contracts + light-client wasm

Low-level primitives (`build` / `upload` / `deploy` / `invoke`) wrap the
`stellar` CLI directly; `deploy-all` and `upload-wasm` are the full orchestrations.

### `stellaribc contracts build`
`stellar contract build --profile contract` → `contracts/target/wasm32v1-none/contract/`.

### `stellaribc contracts upload --wasm <path>`
`stellar contract upload` a wasm; prints the wasm hash.

### `stellaribc contracts deploy --wasm <path> [-- <ctor args>]`
`stellar contract deploy` a wasm; prints the contract id. Constructor args pass
through verbatim after `--`.

```sh
stellaribc contracts deploy --wasm contracts/target/wasm32v1-none/contract/stellar_ibc_router.wasm -- --admin GABC...
```

### `stellaribc contracts invoke --id <contract> -- <fn> <args>`
`stellar contract invoke` a function on a deployed contract.

```sh
stellaribc contracts invoke --id CB2L... -- register_port --port_id transfer --app_address CASB...
```

### `stellaribc contracts deploy-all [--force] [--attestation] [--tendermint]`
Full deploy orchestration: build → deploy mock + router (`--admin`) + transfer-app
(`--router --admin`) → wire the router (`register_client_type`, `register_port`)
→ write all ids to `.env`. Idempotent (skips when `ROUTER_CONTRACT_ADDRESS` is set unless
`--force`).

| Flag | Effect |
|---|---|
| `--force` | redeploy even if `ROUTER_CONTRACT_ADDRESS` is set |
| `--attestation` | also deploy + register the attestation light client |
| `--tendermint` | also deploy + register the tendermint light client |

### `stellaribc contracts upload-wasm`
Build `light-client-wasm` (`wasm32-unknown-unknown`), `wasm-opt` bulk-memory
lowering, then via the api: fund the proposer → submit the `08-wasm` store-code
gov proposal → vote → verify the checksum on-chain → patch `wasm_checksum_hex`
in the hermes config.

---

## `tx` — low-level tx surface

These mirror the gateway's write/query RPCs and are mostly **pending** (they
print a "not wired yet" notice) — they depend on migrating the gateway's
remaining RPCs to prepare→sign→submit (TASKS.md Task 3) and the packet worker
(Task 5).

| Command | Status |
|---|---|
| `tx clients create` · `tx clients update` | pending |
| `tx msg register-counterparty <stellar\|cosmos>` | delegates to `clients counterparty` (pending) |
| `tx msg recv` · `tx msg ack` · `tx msg timeout` | pending |
| `tx query commitment` · `receipt` · `ack` · `header` | pending |

---

## Typical workflows

First run from a clean machine:

```sh
cargo install --path cli        # install the stellaribc binary
stellaribc check               # docker/stellar/cargo present? .env filled?
stellaribc start                # images, chains, contracts, wasm, keys
stellaribc status               # everything green?

stellaribc clients cosmos       # Cosmos client on Stellar
stellaribc clients stellar      # Stellar client on Cosmos
stellaribc clients list
```

Day-to-day:

```sh
stellaribc up                          # bring the stack up
stellaribc api restart --pull          # pull latest + recreate just the api
stellaribc contracts deploy-all --force   # redeploy contracts, rewrite .env
stellaribc gateway restart --pull      # pull latest + pick up the new ROUTER_CONTRACT_ADDRESS
stellaribc down                        # stop the stack
```

---

## Configuration

Read from `stellar-ibc/.env` (shell env overrides). Defaults shown.

| Variable | Default | Used by |
|---|---|---|
| `STELLAR_IBC_ROOT` | _(auto-discovered)_ | repo-root override |
| `COSMOS_CHAIN_ID` | `localosmosis` | status, clients, keys |
| `COSMOS_REST_URL` | `http://127.0.0.1:1318` | check/status/start probes |
| `COSMOS_RPC_URL` | `http://127.0.0.1:26657` | clients (RPC probe) |
| `STELLAR_API_URL` | `http://127.0.0.1:8101` | status, clients list, upload-wasm |
| `STELLAR_GATEWAY_GRPC_PORT` | `50052` | gateway gRPC probe |
| `HERMES_CONFIG` | `<root>/hermes-config.toml` | clients stellar (checksum check) |
| `STELLAR_SIGNING_KEY` | _(required)_ | deploy + the stellar relayer key |
| `STELLAR_RPC_URL` | `https://soroban-testnet.stellar.org` | contracts (stellar CLI) |
| `NETWORK_PASSPHRASE` | `Test SDF Network ; September 2015` | contracts (stellar CLI) |
| `DEPLOYER_IDENTITY` | `admin` | contracts deploy/invoke `--source` |
| `ROUTER_CONTRACT_ADDRESS` / `TRANSFER_CONTRACT_ADDRESS` / `DEPLOYER_ADDRESS` | _(set by deploy-all)_ | status, deploy-all idempotency |
| `COSMOS_CLIENT_ID` / `STELLAR_CLIENT_ID` | _(set by clients cmds)_ | clients idempotency |
| `TRANSFER_PORT` / `MOCK_CLIENT_TYPE` / `ATTESTATION_CLIENT_TYPE` / `TENDERMINT_CLIENT_TYPE` | `transfer` / `mock` / `attestation` / `07-tendermint` | router wiring |
| `API_IMAGE` / `API_TAG` / `API_REGISTRY` | `amandagonsalvesx/stellar-ibc-api` / `latest` / _(none)_ | api image to pull/run |
| `GATEWAY_IMAGE` / `GATEWAY_TAG` / `GATEWAY_REGISTRY` | `amandagonsalvesx/stellar-gateway` / `latest` / _(none)_ | gateway image to pull/run |
| `HERMES_IMAGE` / `HERMES_TAG` / `HERMES_REGISTRY` | `amandagonsalvesx/stellar-hermes-cardano` / `latest` / _(none)_ | hermes image to pull/run |

> `HERMES_REPO` / `DOCKER_USERNAME` / `DOCKER_TOKEN` are only used by the
> Makefile `push-*` targets (building + pushing images), not by the CLI.

---

## Source layout

```
cli/src/
  main.rs            clap command tree + dispatch
  config.rs          base Config: osmosis · stellar · hermes · api · gateway · deployment
  repo.rs            repo-root discovery
  run.rs             process helpers (command / capture / compose / piped)
  probe.rs           http / tcp health probes
  logger.rs          TTY-aware status logger
  shared.rs          print_clients / env_upsert / pending / check helpers
  ops/               install · check · status · stack (up/down) · start · config
  osmosis/           osmosis chain config + lifecycle (start/stop/status)
  stellar/           stellar chain config + lifecycle
  clients/           cosmos · stellar · counterparty · list · config
  hermes/            container (start/stop/restart) · keys · config
  gateway/           container · query · config
  api/               container
  contracts/         build · upload · deploy · invoke · deploy_all · wasm · config
  tx/                clients · msg · query
```

## Makefile

The root `Makefile` is only for **image build + push** (everything else runs
through the CLI directly). Both targets take a `SERVICE=<gateway|hermes|api>`:

```sh
make build SERVICE=gateway   # docker build the image for that service
make push  SERVICE=gateway   # build + docker push (login via DOCKER_USERNAME/DOCKER_TOKEN)
```

Image refs come from `.env` (`<SERVICE>_IMAGE`/`_TAG`); hermes builds from
`HERMES_REPO/ci/release/hermes.Dockerfile`. The Makefile also keeps `make fmt`
(`cargo fmt --all`), `make test` (`cargo test --locked`), and `make cargo-build`
(`cargo build`).
