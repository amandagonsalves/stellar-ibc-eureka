# `interstellar` — Stellar↔Cosmos IBC orchestrator CLI

`interstellar` (binary **`interstellar`**) is the single entry point for the
Stellar↔Cosmos IBC v2 bridge. It brings the stack up, builds/pushes images,
deploys the Soroban contracts, uploads the light client, creates clients,
registers counterparties, and reports status — driving **docker**, the
**`stellar`** CLI, and **`stellar-api`** directly. There are no shell scripts.

It lives at the repo root (`stellar-ibc/interstellar/`) as a workspace member.

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
cargo run -p interstellar -- <command>

# install the `interstellar` binary (either of these)
cargo run -p interstellar -- install   # self-install to the cargo bin dir
cargo install --path interstellar

# then, from anywhere:
interstellar <command>
```

Get help for any command or group:

```sh
interstellar --help
interstellar <group> --help
interstellar <group> <command> --help
```

---

## Command overview

| Group | Commands |
|---|---|
| ops | `install` · `check` · `up` · `down` · `start` |
| `cosmos` | `start` · `stop` · `status` · `testnet [--balance <addr>]` |
| `tx clients` | `create [--cosmos\|--stellar] [--force]` · `counterparty [--cosmos\|--stellar]` |
| `tx contracts` | `upload` · `deploy [--stellar\|--cosmos] [--contract <name>] [--force] [--attestation]` · `invoke` |
| `tx transfer` | `--from --to --amount [--denom --timeout-secs --no-mint]` (chain inferred from the addresses) |
| `tx query` | `--clients [--stellar\|--cosmos] [--client-id]` · `--address [--denom]` (balance reads) |
| `services` | `pull` · `up` · `restart` · `down` · `build` · `push` (`[--api\|--gateway\|--hermes\|--cosmos]`) |
| `hermes` | `keys-import` |
| `gateway` | `query` |
| `test` | `[--ics ics02-clients\|ics02-counterparty\|ics20-transfer\|ics02-query]` |

---

## Top-level (ops) commands

### `interstellar install`
Installs the `interstellar` binary to the cargo bin dir (`cargo install --path
interstellar --force`) and reports whether that dir is on your `PATH`.

### `interstellar check`
Checks prerequisites and configuration, then probes service health. Reports:
toolchain (`docker`, `stellar`, `cargo`), `.env` presence, key config vars
(`STELLAR_SIGNING_KEY`, `ROUTER_CONTRACT_ADDRESS`, …), the configured endpoints,
the deployed contract ids (from `.env`), and the live state of the `cosmos`
(`simd-1`) chain, `stellar-api`, and the gateway gRPC port. Always exits 0.

### `interstellar up [--cosmos | --stellar]`
Brings the stack up via `docker compose up -d`.

| Flag | Effect |
|---|---|
| _(none)_ | start `cosmos` + `api` + `gateway` |
| `--cosmos` | start only `cosmos` |
| `--stellar` | start only `api` + `gateway` |

### `interstellar down [--volumes]`
Stops the stack via `docker compose down`.

| Flag | Effect |
|---|---|
| `--volumes` | also remove named volumes (wipes chain + hermes-key state) |

### `interstellar start`
Full start: pull images → start `cosmos` → start `api` + `gateway` → deploy
contracts → upload the light-client wasm → import relayer keys → provision the
sender + receiver accounts. Each step is skippable; the chain/service steps are
idempotent (probe first, start if down).

| Flag | Effect |
|---|---|
| `--skip-images` | skip pulling the docker images |
| `--skip-contracts` | skip the Soroban contract deploy |
| `--skip-wasm` | skip the light-client-wasm upload |
| `--skip-keys` | skip importing the hermes relayer keys |
| `--skip-accounts` | skip provisioning the sender + receiver accounts |
| `--force-redeploy` | redeploy contracts even if `ROUTER_CONTRACT_ADDRESS` is set |

---

## `cosmos` — local Cosmos devnet

Lifecycle for the `simd-1` chain (the `cosmos` compose service —
`ghcr.io/cosmos/ibc-go-wasm-simd:v11.0.0`, ibc-go v11 + `08-wasm`).

| Command | Flags | What it does |
|---|---|---|
| `start` | — | `docker compose up -d cosmos` + wait for the first block |
| `stop` | — | `docker compose stop cosmos` |
| `status` | — | probe RPC + print network/endpoints |
| `testnet` | `--balance <addr>` | probe the public cosmos-testnet (Cosmos Hub `provider`) — health + node/app version; with `--balance` read an account's balance |

The two genesis accounts each map to two `.env` vars, set on a fresh checkout
before `start`:

| Account | Mnemonic var | Hex signer var | Used by |
|---|---|---|---|
| validator | `COSMOS_VALIDATOR_MNEMONIC` | `COSMOS_FUNDER_PRIVATE_KEY` | genesis validator; the api's gov **funder/voter** (votes with stake) |
| relayer | `COSMOS_RELAYER_MNEMONIC` | `COSMOS_PROPOSER_PRIVATE_KEY` | Hermes relayer key; the api's gov **proposer** |

`docker-compose.yml` passes the mnemonics into the cosmos container and
`setup.sh` recovers + funds those accounts at genesis; the api signs gov
messages (store-code proposal during `upload-wasm`) with the hex keys. The hex
key is just the mnemonic's private key, so each account's two vars must stay in
sync.

---

## `tx` — write operations

Mutating commands live under `tx`. For `clients` and `contracts`, a `--cosmos` /
`--stellar` flag selects the side; **omit it to act on both**.

### `interstellar tx clients create [--cosmos | --stellar] [--force]`
Create the Cosmos (`07-tendermint`, on Stellar) and/or Stellar (`08-wasm`, on
Cosmos) client; with neither flag, creates both. `--force` recreates even if the
id is already set. The Stellar client needs `wasm_checksum_hex` in the hermes
config first (`tx contracts deploy --cosmos`).

### `interstellar tx clients counterparty [--cosmos | --stellar]`
Register the counterparty on the chosen side (IBC v2 `registerCounterparty`, no
handshake); with neither flag, registers on both. The Stellar side goes through
the gateway prepare→sign→submit path, the Cosmos side through ibc-go.

### `interstellar tx contracts upload`
Build and upload (install) every Soroban contract wasm to the network, printing
each wasm hash.

### `interstellar tx contracts deploy [--stellar | --cosmos] [--contract <name>] [--force] [--attestation]`
- `--stellar` (default): deploy the Soroban contracts. With `--contract <name>`
  (the wasm file name without `.wasm`, e.g. `stellar_ibc_router`) deploy just
  that one; omit it for the full build + deploy + wire-router + write-`.env`
  orchestration (`--force` redeploys, `--attestation` also deploys the
  attestation LC).
- `--cosmos`: store the light-client code on Cosmos (08-wasm gov store-code).

### `interstellar tx contracts invoke --id <contract> -- <fn> <args>`
Invoke a function on a deployed Soroban contract; the function name and its args
pass through verbatim after `--`.

```sh
interstellar tx contracts invoke --id CB2L... -- register_port --port_id transfer --app_address CASB...
```

### `interstellar tx transfer --from <addr> --to <addr> --amount <n> [--denom --timeout-secs --no-mint]`
Originate an ICS-20 transfer. The source/destination chain is inferred from each
address (`cosmos1…` → Cosmos, `G…`/`C…` → Stellar) and routed accordingly —
Stellar→Cosmos is wired, Cosmos→Stellar is pending (M4). `--denom` defaults to
`stake`.

---

## `tx query` — read client states + balances

```sh
interstellar tx query --clients [--stellar | --cosmos] [--client-id <id>]
interstellar tx query --address <addr> [--denom <denom>]
```

`--clients` reads client states: with `--stellar` / `--cosmos` it scopes to one
network (Stellar via the api `/stellar/clients`, Cosmos via the IBC REST
`client_states`), with neither it reads both, and `--client-id` restricts to a
single client. `--address` reads an account's balances (the chain is inferred
from the address); `--denom` filters to a single denom.

---

## `test` — ICS integration flows

```sh
interstellar test [--ics ics02-clients|ics02-counterparty|ics20-transfer|ics02-query]
```

Runs the happy-path integration flow for each ICS milestone against a **running
stack** (bring it up first with `interstellar start`). With no `--ics`, every
flow runs in dependency order and a summary is printed; the command exits
non-zero if any flow fails. Flows are labelled by IBC standard — client
lifecycle, counterparty registration, and client-state queries are all ICS-02
under Eureka; token transfer is ICS-20.

| ICS | What it asserts |
|---|---|
| `ics02-clients` | creates the Cosmos (`07-tendermint`) and Stellar (`08-wasm`) clients and checks the returned ids |
| `ics02-counterparty` | bootstraps clients + counterparties and checks the Stellar router lists the paired clients |
| `ics20-transfer` | originates a Stellar→Cosmos ICS-20 transfer, waits for the relay round trip to close, and checks the Cosmos voucher increased |
| `ics02-query` | checks the api `/health` + `/stellar/clients` reads and the Cosmos `client_states` read |

The flow bodies live in `interstellar/src/tests/` (one file per ICS). CI runs
them per-milestone via the `interstellar.yml` workflow — each ICS is a separate,
independently re-runnable job.

---

## `services` — service lifecycle + images

```sh
interstellar services <pull|up|restart|down|build|push> [--api|--gateway|--hermes|--cosmos]
```

One namespace for the `api`, `gateway`, `hermes`, and `cosmos` services. A
`--api` / `--gateway` / `--hermes` / `--cosmos` flag selects the target;
**omitting it acts on all**. Image tags resolve from `.env`.

| Command | What it does |
|---|---|
| `pull` | `docker compose pull` the selected image(s) |
| `up` | pull then `docker compose up -d` |
| `restart` | remove the existing container(s) (`rm -s -f`) then `up` |
| `down` | stop + remove the container(s) (`rm -s -f`) |
| `build` | `docker build` the image(s) for the host arch |
| `push` | `docker build` + `docker push` for the host arch |

`build` / `push` cover the three custom images only — `--cosmos` is an upstream
image and is rejected for build/push. **hermes** is built from a separate repo:
the CLI clones `HERMES_REPO_URL` into `target/hermes-relayer` (or updates it),
checks out `HERMES_BRANCH`, then builds from there.

Both build for the **host architecture only** — multi-arch (amd64 + arm64)
manifests are published by the CD workflows (`api-cd` / `gateway-cd` /
`hermes-cd`), which build each arch on a native runner. Building amd64 locally on
an arm64 host would cross-compile under QEMU (slow, and the hermes relayer's
proving-key download times out), so it is intentionally left to CI.

---

## `hermes` — relayer

| Command | What it does |
|---|---|
| `keys-import` | import the cosmos relayer mnemonic + `STELLAR_SIGNING_KEY` into the `hermes-keys` volume (one-shot `docker compose run`) |

The relayer's Stellar key must equal the router admin key (`STELLAR_SIGNING_KEY`).
Container lifecycle (start/stop/restart) lives under `services`.

---

## `gateway` — gateway service

| Command | What it does |
|---|---|
| `query` | direct gateway gRPC reads — *pending* |

Container lifecycle lives under `services`.

---

## Typical workflows

First run from a clean machine:

```sh
cargo install --path interstellar   # install the interstellar binary
interstellar check                # docker/stellar/cargo present? .env filled? everything green?
interstellar start                # images, chains, contracts, wasm, keys, accounts

interstellar tx clients create    # create both clients (Cosmos on Stellar, Stellar on Cosmos)
interstellar tx query --clients   # read client states on both networks
```

Day-to-day:

```sh
interstellar up                              # bring the stack up
interstellar services restart --api          # pull latest + recreate just the api
interstellar tx contracts deploy --stellar --force   # redeploy contracts, rewrite .env
interstellar services restart --gateway      # recreate the gateway to pick up the new ROUTER_CONTRACT_ADDRESS
interstellar down                            # stop the stack
```

---

## Configuration

Read from `stellar-ibc/.env` (shell env overrides). Defaults shown.

| Variable | Default | Used by |
|---|---|---|
| `STELLAR_IBC_ROOT` | _(auto-discovered)_ | repo-root override |
| `COSMOS_CHAIN_ID` | `simd-1` | status, clients, keys |
| `COSMOS_REST_URL` | `http://127.0.0.1:1317` | check/status/start probes |
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
| `API_IMAGE` / `API_TAG` / `API_REGISTRY` | `amandagonsalvesx/stellar-eureka-api` / `latest` / _(none)_ | api image to pull/run |
| `GATEWAY_IMAGE` / `GATEWAY_TAG` / `GATEWAY_REGISTRY` | `amandagonsalvesx/stellar-eureka-gateway` / `latest` / _(none)_ | gateway image to pull/run |
| `HERMES_IMAGE` / `HERMES_TAG` / `HERMES_REGISTRY` | `amandagonsalvesx/stellar-hermes-cardano` / `latest` / _(none)_ | hermes image to pull/run |

> `HERMES_REPO_URL` / `HERMES_BRANCH` locate the relayer source for
> `services build/push --hermes` (cloned into `target/hermes-relayer`);
> `DOCKER_USERNAME` / `DOCKER_TOKEN` authenticate the push.

---

## Source layout

```
interstellar/src/
  main.rs            clap command tree + dispatch
  config.rs          base Config: cosmos · stellar · hermes · api · gateway · deployment
  repo.rs            repo-root discovery
  run.rs             process helpers (command / capture / compose / piped)
  tools.rs           typed wrappers over run (stellar · gaiad · docker)
  probe.rs           http / tcp health probes
  logger.rs          TTY-aware status logger
  shared.rs          chain_of · print_clients · env_upsert · check helpers
  install.rs         install command (self-install to the cargo bin dir)
  check.rs           prerequisites + config + service-health check
  stack.rs           up / down (docker compose)
  start.rs           full bring-up orchestration
  service.rs         shared docker-compose start wrapper
  accounts/          sender + receiver account provisioning (stellar + cosmos)
  tx/                write operations grouped under `tx`
    mod.rs           tx command tree (clients · contracts · transfer · query) + side routing
    clients/         create (cosmos/stellar) · counterparty · config
    contracts/       upload · deploy_all · wasm · build · config (+ deploy_one/invoke)
    transfer/        ICS-20 transfer origination (stellar → cosmos)
    query/           client-state reads (clients.rs) + address-routed balances (balances.rs)
  services/          service lifecycle + images grouped under `services`
    mod.rs           pull/up/restart/down/build/push (api · gateway · hermes · cosmos)
    cosmos/          cosmos (simd-1) chain config + lifecycle (start/stop/status/testnet)
    stellar/         stellar chain config
    gateway/         gRPC query · config
    hermes/          container (start + exec) · keys · config
  tests/             ICS integration flows (interstellar test)
```

## Makefile

The root `Makefile` carries a single convenience target — `make install`, which
runs `cargo run -p interstellar -- install`. Everything else runs through the CLI
directly: image build + push is `interstellar services build` / `services push`
(which resolve image refs from `.env` and clone the hermes source per
`HERMES_REPO_URL` / `HERMES_BRANCH`).
