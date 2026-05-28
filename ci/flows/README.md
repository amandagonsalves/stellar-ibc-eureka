# `ci/flows/` — script reference

The scripts in this directory are the building blocks for bringing the Stellar↔Cosmos bridge from a clean machine to a running e2e flow. Each script is callable standalone, each is exposed as a `make` target in `../Makefile`, and each is idempotent (re-running is safe).

All scripts source [`_env.sh`](./_env.sh) and read configuration from `stellar-ibc/.env`. Variables set in your shell take precedence over `.env`.

## Run order for a clean machine

```bash
make -C ci cosmos-only         # 1. start the Cosmos counterparty (Osmosis localnet)
cargo run -p stellar-hermes-gateway  # 2. start the Stellar gateway (separate terminal)
make -C ci f0                  # 3. build/push images + deploy contracts + upload lc-wasm
```

`make -C ci f0` orchestrates [`build-gateway-image.sh`](#build-gateway-imagesh), [`build-hermes-image.sh`](#build-hermes-imagesh), [`upload-and-deploy-contracts.sh`](#upload-and-deploy-contractssh), and [`upload-lc-wasm.sh`](#upload-lc-wasmsh) in order. Each can also be run standalone (see below).

End-to-end recipe with verification at every stage: [`docs/stellar-cosmos-e2e.md`](../../../docs/stellar-cosmos-e2e.md).

---

## Make-target index

| Make target | Script | One-liner |
|---|---|---|
| `make -C ci cosmos-only` | [cosmos-only.sh](#cosmos-onlysh) | Start ONLY the Osmosis Cosmos localnet (skips the broken Cardano gateway) |
| `make -C ci caribic-preflight` | [caribic-preflight.sh](#caribic-preflightsh) | Pre-build the cardano-node-local-clock image so `caribic start` doesn't choke |
| `make -C ci gateway-image` | [build-gateway-image.sh](#build-gateway-imagesh) | Build + push `${GATEWAY_IMAGE}:${GATEWAY_TAG}` |
| `make -C ci hermes-image` | [build-hermes-image.sh](#build-hermes-imagesh) | Build + push `${HERMES_IMAGE}:${HERMES_TAG}` |
| `make -C ci deploy-contracts` | [upload-and-deploy-contracts.sh](#upload-and-deploy-contractssh) | Build + upload + deploy router/transfer-app/mock-LC on Stellar; write IDs to `.env` |
| `make -C ci upload-lc-wasm` | [upload-lc-wasm.sh](#upload-lc-wasmsh) | Build `light-client-wasm`, gov-upload to Cosmos, patch hermes config |
| `make -C ci f0` | [f0-bootstrap.sh](#f0-bootstrapsh) | Full F0 — orchestrates all of the above with skip flags |

---

## `_env.sh`

Helper. Sourced by every other script — **not run directly**.

Exposes two functions:

- `load_env_file <path>` — reads `KEY=VALUE` lines from `<path>` and exports them, **only if the env var isn't already set**. Strips surrounding `"` or `'`. Shell-set env wins.
- `docker_login_if_creds <push-flag>` — if `push-flag` is `1`/`true` and both `DOCKER_USERNAME` + `DOCKER_TOKEN` are set in env, runs `docker login --password-stdin`. Otherwise no-ops with a warning.

---

## `cosmos-only.sh`

**Purpose.** Bring up **only** the Osmosis localnet (`localosmosis`). This bypasses caribic's full `start` command, which currently fails on the Cardano-side gateway-app (HostState UTxO schema drift) — a Cardano-side bug we don't need to wait on for Stellar↔Cosmos work.

### How to run

```bash
make -C ci cosmos-only
# or:
bash stellar-ibc/ci/flows/cosmos-only.sh
```

### What it does

1. Sources `_env.sh` and loads `.env`.
2. Verifies `${INCUBATOR_REPO}` exists and the `caribic` binary is on PATH.
3. Runs `caribic chain start --chain ${COSMOS_CHAIN_NAME} --network ${COSMOS_NETWORK}` from inside the incubator repo. With the default `.env` values this is `caribic chain start --chain osmosis --network local`, which calls `caribic/src/chains/osmosis/lifecycle.rs::start_local` — `docker compose -f chains/osmosis/configuration/docker-compose.yml up -d --build`.
4. Polls `${COSMOS_RPC_URL}/status` until `latest_block_height > 0` or `WAIT_TIMEOUT_SEC` elapses.
5. Probes `${COSMOS_REST_URL}/cosmos/base/tendermint/v1beta1/node_info` and logs success or a warning if REST is slow to come up.

### Env vars (with defaults)

| Var | Default | What it does |
|---|---|---|
| `INCUBATOR_REPO` | `../cardano-ibc-incubator` | Path to your cardano-ibc-incubator checkout |
| `COSMOS_CHAIN_NAME` | `osmosis` | Caribic chain adapter to start |
| `COSMOS_NETWORK` | `local` | Caribic network profile |
| `COSMOS_RPC_URL` | `http://127.0.0.1:26658` | Where to poll for chain readiness |
| `COSMOS_REST_URL` | `http://127.0.0.1:1318` | REST probe target |
| `WAIT_TIMEOUT_SEC` | `180` | How long to wait for first block |

### Outputs

Long-running Docker containers (`configuration-osmosisd-1` + `configuration-redis-1`). No state changes to `.env`.

### Idempotency

Safe to re-run. `caribic chain start` is itself idempotent — if the container is already up, it'll attach to it.

### Common failures

| Symptom | Fix |
|---|---|
| `caribic: command not found` | Install from `${INCUBATOR_REPO}/caribic`: `cd $INCUBATOR_REPO/caribic && cargo install --path .` |
| `Unable to find image local:osmosis` | First run builds the Osmosis image (~5–10 min). Just wait it out. |
| Polls timeout, container exists | `docker logs configuration-osmosisd-1` for the underlying error |

---

## `caribic-preflight.sh`

**Purpose.** Workaround for an upstream caribic startup bug: `caribic/src/setup.rs:817` runs `docker run cardano-node-local-clock:10.1.4-3` **before** `caribic/src/start.rs:1817` builds that image. On a clean machine `caribic start` therefore aborts with `Unable to find image cardano-node-local-clock:10.1.4-3 locally`. This script builds the image up-front.

**Only needed if you run the full `caribic start --clean`** (Cardano-side work). For our Stellar↔Cosmos flow we use [`cosmos-only.sh`](#cosmos-onlysh) instead, which skips the Cardano devnet entirely and therefore doesn't hit this bug.

### How to run

```bash
make -C ci caribic-preflight
# or:
bash stellar-ibc/ci/flows/caribic-preflight.sh
```

### What it does

1. Verifies `docker` is on PATH and `${INCUBATOR_REPO}` exists.
2. Verifies `${INCUBATOR_REPO}/chains/cardano/Dockerfile.local-clock` exists.
3. If `${LOCAL_CLOCK_IMAGE}` is already in `docker images`, exits 0.
4. Otherwise: `docker build -t ${LOCAL_CLOCK_IMAGE} -f ${LOCAL_CLOCK_DOCKERFILE} ${LOCAL_CLOCK_CONTEXT}`.
5. Verifies the image exists post-build.

### Env vars

| Var | Default | What it does |
|---|---|---|
| `INCUBATOR_REPO` | `../cardano-ibc-incubator` | Path to your cardano-ibc-incubator checkout |
| `LOCAL_CLOCK_IMAGE` | `cardano-node-local-clock:10.1.4-3` | Image tag caribic looks for |

### Outputs

A locally-tagged Docker image. No `.env` changes.

### Idempotency

Safe — image-present check skips re-build.

---

## `build-gateway-image.sh`

**Purpose.** Build the Stellar gateway image from `stellar-ibc/Dockerfile`, smoke-test it, and (optionally) push to DockerHub.

### How to run

```bash
make -C ci gateway-image
# or:
bash stellar-ibc/ci/flows/build-gateway-image.sh
```

### What it does

1. Validates `docker` is present and the Dockerfile looks like a gateway Dockerfile (greps for `stellar-hermes-gateway`).
2. `docker build -t ${GATEWAY_IMAGE}:${GATEWAY_TAG} -f stellar-ibc/Dockerfile stellar-ibc/`.
3. Smoke test: runs the image detached for 3 seconds with `.env`, asserts the container is still running, prints the last 5 log lines, then stops it. Catches binaries that crash on startup (wrong port, missing dep, etc.).
4. If `${GATEWAY_REGISTRY}` is set, tags as `${GATEWAY_REGISTRY}/${GATEWAY_IMAGE}:${GATEWAY_TAG}`.
5. If `PUSH=1`: `docker login` via `${DOCKER_USERNAME}` + `${DOCKER_TOKEN}` (from `.env`), then `docker push`.

### Env vars

| Var | Default | What it does |
|---|---|---|
| `GATEWAY_IMAGE` | `amandagonsalvesx/stellar-gateway` | DockerHub repo |
| `GATEWAY_TAG` | `latest` | Image tag |
| `GATEWAY_REGISTRY` | _(unset)_ | Optional registry prefix (e.g. `ghcr.io/me`) |
| `PUSH` | `1` | Set to `0` to skip the push |
| `DOCKER_USERNAME` / `DOCKER_TOKEN` | _(unset)_ | DockerHub creds; if absent the script warns and tries the push anyway |

### Outputs

- Local Docker image `${GATEWAY_IMAGE}:${GATEWAY_TAG}`.
- (If `PUSH=1`) Pushed to DockerHub at the same ref.

### Idempotency

Safe to re-run. Docker layer cache makes rebuilds fast when nothing changed.

### Common failures

| Symptom | Fix |
|---|---|
| `denied: requested access to the resource is denied` on push | `GATEWAY_IMAGE` doesn't match your DockerHub namespace, or wrong `DOCKER_USERNAME`/`DOCKER_TOKEN` |
| `requires rustc 1.88` during build | Bump the `FROM rust:X.Y-slim-bookworm` line in `stellar-ibc/Dockerfile` |
| Smoke test reports "container exited within 3s" | Inspect the printed `docker logs` — usually a config/env issue. Try `docker run --rm --env-file .env ${GATEWAY_IMAGE}:${GATEWAY_TAG}` interactively to debug |

---

## `build-hermes-image.sh`

**Purpose.** Build the hermes-relayer image (cardano-foundation fork with our Stellar endpoint) from `${HERMES_REPO}/Dockerfile`, smoke-test with `hermes version`, and push.

### How to run

```bash
make -C ci hermes-image
# or:
bash stellar-ibc/ci/flows/build-hermes-image.sh
```

### What it does

1. Validates `docker` present, `${HERMES_REPO}/Cargo.toml` exists and references `ibc-relayer-cli` (catches the case where `HERMES_REPO` points at the wrong checkout).
2. `docker build -t ${HERMES_IMAGE}:${HERMES_TAG} -f ${HERMES_REPO}/Dockerfile ${HERMES_REPO}`.
3. Smoke test: `docker run --rm ... ${LOCAL_REF} version` and prints the first 5 lines of output.
4. Optional re-tag to `${HERMES_REGISTRY}/${HERMES_IMAGE}:${HERMES_TAG}`.
5. If `PUSH=1`: `docker login` then `docker push`.

### Env vars

| Var | Default | What it does |
|---|---|---|
| `HERMES_REPO` | `../hermes-relayer` | Path to the hermes-relayer fork |
| `HERMES_IMAGE` | `amandagonsalvesx/stellar-hermes-cardano` | DockerHub repo |
| `HERMES_TAG` | `latest` | Image tag |
| `HERMES_REGISTRY` | _(unset)_ | Optional registry prefix |
| `PUSH` | `1` | Set to `0` to skip the push |
| `DOCKER_USERNAME` / `DOCKER_TOKEN` | _(unset)_ | DockerHub creds |

### Outputs

- Local Docker image `${HERMES_IMAGE}:${HERMES_TAG}`.
- (If `PUSH=1`) Pushed to DockerHub.

### Idempotency

Safe to re-run.

### Common failures

| Symptom | Fix |
|---|---|
| `HERMES_REPO does not look like a hermes-relayer checkout` | Set `HERMES_REPO=/absolute/path/to/hermes-relayer` in `.env` |
| `Cargo.toml does not reference ibc-relayer-cli` | `HERMES_REPO` is pointing at a subdirectory; point at the fork root |
| `protoc failed: google/protobuf/timestamp.proto: File not found` | Add `libprotobuf-dev` to the hermes Dockerfile's apt install (already fixed in `hermes-relayer/Dockerfile`) |
| `Unable to find libclang` | Add `clang` + `libclang-dev` (already fixed in `hermes-relayer/Dockerfile`) |

---

## `upload-and-deploy-contracts.sh`

**Purpose.** Build all Soroban contracts, upload + deploy router, transfer-app, mock LC (plus optional attestation/tendermint LCs), wire the router with `register_client_type` + `register_port`, and write the deployed contract IDs back into `.env`.

### How to run

```bash
make -C ci deploy-contracts
# or:
bash stellar-ibc/ci/flows/upload-and-deploy-contracts.sh
```

### What it does

1. Checks `stellar` CLI is on PATH and `${STELLAR_SIGNING_KEY}` is set.
2. **Idempotency guard.** If `IBC_CONTRACT_ID` is already set in `.env` and `FORCE_REDEPLOY != 1`, exits 0 with a "nothing to do" message.
3. Registers `${DEPLOYER_IDENTITY}` (default `stellar-ibc-deployer`) from `STELLAR_SIGNING_KEY` in the stellar CLI keystore.
4. `stellar keys fund` against `${STELLAR_RPC_URL}` (friendbot — warns if already funded).
5. `stellar contract build --profile contract` (compiles every workspace contract to `target/wasm32v1-none/release/`).
6. Uploads `mock_light_client.wasm`, `stellar_ibc_router.wasm`, `stellar_transfer_app.wasm`.
7. Deploys:
   - `MOCK_LC_CONTRACT_ID = stellar contract deploy --wasm mock_light_client.wasm`
   - `IBC_CONTRACT_ID = stellar contract deploy --wasm stellar_ibc_router.wasm -- --admin <DEPLOYER_ADDRESS>`
   - `TRANSFER_CONTRACT_ID = stellar contract deploy --wasm stellar_transfer_app.wasm -- --router <IBC_CONTRACT_ID> --admin <DEPLOYER_ADDRESS>`
8. If `DEPLOY_ATTESTATION_LC=1`: uploads + deploys `stellar_attestation_light_client.wasm`.
9. If `DEPLOY_TENDERMINT_LC=1`: uploads + deploys `stellar_tendermint_light_client.wasm`.
10. Wires the router:
    - `register_client_type --client_type ${MOCK_CLIENT_TYPE} --lc_address <MOCK_LC_CONTRACT_ID>` (default `mock`)
    - Same for attestation/tendermint if deployed
    - `register_port --port_id ${TRANSFER_PORT} --app_address <TRANSFER_CONTRACT_ID>` (default `transfer`)
11. Patches `.env` in place — adds/updates `IBC_CONTRACT_ID`, `TRANSFER_CONTRACT_ID`, `MOCK_LC_CONTRACT_ID`, `ATTESTATION_LC_CONTRACT_ID`, `TENDERMINT_LC_CONTRACT_ID`, `DEPLOYER_ADDRESS`.

### Env vars

| Var | Default | What it does |
|---|---|---|
| `STELLAR_RPC_URL` | `https://soroban-testnet.stellar.org` | Soroban RPC endpoint |
| `NETWORK_PASSPHRASE` | `Test SDF Network ; September 2015` | Network passphrase |
| `STELLAR_SIGNING_KEY` | _(required)_ | Funded Stellar testnet secret (S…) |
| `DEPLOYER_IDENTITY` | `stellar-ibc-deployer` | Local keystore label |
| `FORCE_REDEPLOY` | `0` | Set to `1` to redeploy even when `IBC_CONTRACT_ID` is already in `.env` |
| `DEPLOY_ATTESTATION_LC` | `0` | Set to `1` to also deploy the attestation LC |
| `DEPLOY_TENDERMINT_LC` | `0` | Set to `1` to also deploy the tendermint LC |
| `TRANSFER_PORT` | `transfer` | Port name registered for the transfer app |
| `MOCK_CLIENT_TYPE` / `ATTESTATION_CLIENT_TYPE` / `TENDERMINT_CLIENT_TYPE` | `mock` / `attestation` / `07-tendermint` | client_type strings registered in the router |

### Outputs

- Six new Soroban contracts on Stellar testnet (or three if you skip the optional LCs).
- `.env` updated in place with the resulting contract IDs.
- **Restart your running `stellar-hermes-gateway`** afterward so it picks up the new `IBC_CONTRACT_ID` + `TRANSFER_CONTRACT_ID`.

### Idempotency

Safe to re-run. By default it short-circuits when `IBC_CONTRACT_ID` is already populated. Set `FORCE_REDEPLOY=1` for a clean slate.

### Common failures

| Symptom | Fix |
|---|---|
| `STELLAR_SIGNING_KEY is empty` | `stellar keys generate ci-deployer --network testnet --fund && stellar keys show ci-deployer` → paste secret into `.env` |
| `friendbot fund failed` | Network usually has no friendbot, or account already funded. The warning is benign if the address has XLM. |
| Build error compiling contracts | Run `cd contracts && stellar contract build --profile contract` interactively to see the cargo error |

---

## `upload-lc-wasm.sh`

**Purpose.** Build `light-client-wasm` for `wasm32-unknown-unknown`, post-process with `wasm-opt`, copy into the running Cosmos chain container, submit + pass a 08-wasm governance proposal to register the wasm checksum, verify on-chain, and patch the `wasm_checksum_hex` field of `ci/hermes-config.toml`.

### How to run

```bash
make -C ci upload-lc-wasm
# or:
bash stellar-ibc/ci/flows/upload-lc-wasm.sh
```

Cosmos chain must be running first (`make -C ci cosmos-only`).

### What it does

1. **Step 1 — Build.** `cargo build --target wasm32-unknown-unknown -p light-client-wasm --release`. Asserts the resulting `.wasm` exists. If `wasm-opt` is installed, runs `wasm-opt --enable-bulk-memory --llvm-memory-copy-fill-lowering -O1 --strip-debug` (lowers bulk-memory ops so the cardano-entrypoint wasmvm can interpret them).
2. **Step 2 — Probe REST** at `${COSMOS_REST_URL}/cosmos/base/tendermint/v1beta1/node_info`. Exits 0 (SKIP) if not reachable.
3. **Step 3 — Find Cosmos container** via `docker ps -qf name=osmosisd`. Exits 0 (SKIP) if not found.
4. **Step 4 — `docker cp`** the wasm into `/tmp/stellar_lc_wasm.wasm` inside the container.
5. **Step 5 — Gov proposal.** `osmosisd tx ibc-wasm store-code /tmp/stellar_lc_wasm.wasm --from ${COSMOS_PROPOSER_KEY} --deposit 1${COSMOS_GAS_DENOM} ...`. Waits 4s for the tx to land.
6. **Step 6 — Find the new proposal** in voting period: queries `cosmos/gov/v1/proposals?proposal_status=PROPOSAL_STATUS_VOTING_PERIOD` and takes the last one.
7. **Step 7 — Vote YES** from `${COSMOS_VOTER_KEY}`, then sleeps `${VOTING_PERIOD}` seconds (default 20s).
8. **Step 8 — Verify checksum.** `osmosisd query ibc-wasm checksums`; asserts the local sha256 of the wasm appears in the list.
9. **Step 9 — Patch hermes config** (skip if `PATCH_HERMES_CONFIG=0`). In-place substitutes `wasm_checksum_hex = '…'` in `ci/hermes-config.toml`.

### Env vars

| Var | Default | What it does |
|---|---|---|
| `COSMOS_CHAIN_ID` | `localosmosis` | For logging; passed via `--chain-id` |
| `COSMOS_REST_URL` | `http://127.0.0.1:1318` | REST probe + proposal-query endpoint |
| `CONTAINER` | _auto from `docker ps -qf name=osmosisd`_ | Override to point at a different container |
| `CHAIN_BIN` | `osmosisd` | Binary inside the container |
| `CHAIN_HOME` | `/osmosis/.osmosisd` | Keyring home inside the container |
| `NODE` | `tcp://localhost:26657` | Tendermint RPC inside the container |
| `COSMOS_PROPOSER_KEY` | `val` | Key used to submit `store-code` |
| `COSMOS_VOTER_KEY` | `val` | Key used to vote yes |
| `COSMOS_GAS_DENOM` | `uosmo` | Deposit denomination |
| `VOTING_PERIOD` | `20` | Seconds to sleep after voting |
| `PATCH_HERMES_CONFIG` | `1` | Set to `0` to skip the in-place config edit |

### Outputs

- A registered wasm checksum in the Cosmos chain's 08-wasm keeper.
- `ci/hermes-config.toml` patched with the matching `wasm_checksum_hex`.

### Idempotency

Re-runnable. If you re-upload the same wasm, you'll get a duplicate proposal — it'll pass, and the checksum already on-chain remains valid.

### Common failures

| Symptom | Fix |
|---|---|
| `SKIP: localosmosis REST not reachable` | `make -C ci cosmos-only` first |
| `SKIP: container '…' not found` | The Cosmos chain isn't running. Check `docker ps`. |
| `unknown key: val` | `osmosisd keys list --keyring-backend test` and set `COSMOS_PROPOSER_KEY`/`COSMOS_VOTER_KEY` accordingly |
| `no proposal currently in voting period` | The `tx ibc-wasm store-code` tx failed before reaching the gov module — usually wrong key or insufficient deposit |

---

## `f0-bootstrap.sh`

**Purpose.** End-to-end orchestrator. Calls the other scripts in the right order with explicit skip flags, then prints a summary. This is the script you'd run on a fresh laptop after setting up `.env` + bringing up the Cosmos chain + Stellar gateway.

### How to run

```bash
make -C ci f0
# or:
bash stellar-ibc/ci/flows/f0-bootstrap.sh
```

### What it does (per step — each can SKIP without failing)

| Step | Calls | Skips when |
|---|---|---|
| 0a — gateway image | `build-gateway-image.sh` | `SKIP_IMAGE_BUILD=1` |
| 0b — hermes image | `build-hermes-image.sh` | `SKIP_IMAGE_BUILD=1` |
| 1 — probe Cosmos | `curl ${COSMOS_REST}/cosmos/base/tendermint/v1beta1/node_info` | Cosmos REST unreachable (exits 0 with hint to run `cosmos-only`) |
| 2 — probe Stellar gateway | `curl ${GATEWAY_HTTP}/health` | Gateway HTTP unreachable (exits 0 with hint to `cargo run -p stellar-hermes-gateway`) |
| 3 — deploy contracts | `upload-and-deploy-contracts.sh` (then reloads `.env`) | `SKIP_CONTRACT_DEPLOY=1` |
| 4 — upload lc-wasm + patch hermes config | `upload-lc-wasm.sh` | Internally skips if Cosmos REST/container unreachable |

### Env vars (orchestration-specific)

| Var | Default | What it does |
|---|---|---|
| `SKIP_IMAGE_BUILD` | `0` | Set to `1` to skip both Docker image builds |
| `SKIP_CONTRACT_DEPLOY` | `0` | Set to `1` to skip contract deploy (re-reads `.env` regardless) |
| `COSMOS_CHAIN_ID` | `localosmosis` | Used for the probe message |
| `COSMOS_REST_URL` | `http://127.0.0.1:1318` | Step 1 probe target |
| `STELLAR_GATEWAY_HTTP_PORT` | `8101` | Step 2 probe target |
| `HERMES_CONFIG` | `ci/hermes-config.toml` | Passed through to `upload-lc-wasm.sh` for the patch step |

Plus everything read by the underlying scripts.

### Outputs

Cumulative — see the per-script "Outputs" sections.

### Idempotency

Fully idempotent across both axes: each sub-script is itself idempotent, and the orchestrator's probes only proceed when prerequisites are met.

### Common bootstrap recipes

```bash
make -C ci f0                                              # First-time bootstrap
SKIP_IMAGE_BUILD=1 make -C ci f0                           # Re-use already-pushed images
SKIP_IMAGE_BUILD=1 SKIP_CONTRACT_DEPLOY=1 make -C ci f0    # Just refresh the lc-wasm + config patch
FORCE_REDEPLOY=1 make -C ci deploy-contracts               # Redeploy contracts only (clean slate)
PATCH_HERMES_CONFIG=0 make -C ci upload-lc-wasm            # Re-upload wasm without touching hermes-config.toml
```

---

## Common env vars (reference)

These appear across multiple scripts. Source of truth is `stellar-ibc/.env` (override per-call via shell env).

| Var | Used by | Purpose |
|---|---|---|
| `STELLAR_RPC_URL` | gateway, deploy-contracts | Soroban RPC endpoint (testnet by default) |
| `NETWORK_PASSPHRASE` | gateway, deploy-contracts | Stellar network identifier |
| `STELLAR_SIGNING_KEY` | deploy-contracts | Funded testnet secret used to deploy contracts |
| `STELLAR_GATEWAY_GRPC_PORT` / `STELLAR_GATEWAY_HTTP_PORT` | gateway, f0 | Where the gateway listens (default 50052 / 8101) |
| `INCUBATOR_REPO` | cosmos-only, caribic-preflight | Path to your cardano-ibc-incubator checkout |
| `HERMES_REPO` | build-hermes-image | Path to the hermes-relayer fork checkout |
| `COSMOS_CHAIN_NAME` / `COSMOS_NETWORK` | cosmos-only | Caribic chain adapter + network profile |
| `COSMOS_CHAIN_ID` | f0, upload-lc-wasm | Chain id for tx/log lines (`localosmosis`) |
| `COSMOS_RPC_URL` / `COSMOS_REST_URL` / `COSMOS_GRPC_URL` | cosmos-only, f0, upload-lc-wasm | Cosmos chain endpoints |
| `COSMOS_PROPOSER_KEY` / `COSMOS_VOTER_KEY` | upload-lc-wasm | Cosmos keys for gov proposal + vote |
| `COSMOS_GAS_DENOM` | upload-lc-wasm | Gas denom (`uosmo`) |
| `DOCKER_USERNAME` / `DOCKER_TOKEN` | build-*-image | DockerHub creds for auto-login |
| `GATEWAY_IMAGE` / `GATEWAY_TAG` | build-gateway-image | DockerHub ref for the gateway image |
| `HERMES_IMAGE` / `HERMES_TAG` | build-hermes-image | DockerHub ref for the hermes image |
| `PUSH` | build-*-image | `1` to push, `0` to keep image local-only |
| `SKIP_IMAGE_BUILD` / `SKIP_CONTRACT_DEPLOY` | f0 | Orchestrator skips |
| `FORCE_REDEPLOY` | deploy-contracts | Override the "already deployed" guard |
| `DEPLOY_ATTESTATION_LC` / `DEPLOY_TENDERMINT_LC` | deploy-contracts | Opt-in for the alternative LCs |
| `IBC_CONTRACT_ID` / `TRANSFER_CONTRACT_ID` / `MOCK_LC_CONTRACT_ID` / `ATTESTATION_LC_CONTRACT_ID` / `TENDERMINT_LC_CONTRACT_ID` / `DEPLOYER_ADDRESS` | (written by deploy-contracts; read by gateway + later scripts) | Contract addresses populated after deploy |
