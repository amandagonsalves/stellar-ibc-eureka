#!/bin/bash
set -euo pipefail

SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
CI_DIR=$(cd "${SCRIPT_DIR}/.." && pwd)
REPO_ROOT=$(cd "${CI_DIR}/.." && pwd)
ENV_FILE="${REPO_ROOT}/.env"

source "${SCRIPT_DIR}/_env.sh"
load_env_file "${ENV_FILE}"

STELLAR_RPC_URL="${STELLAR_RPC_URL:-https://soroban-testnet.stellar.org}"
NETWORK_PASSPHRASE="${NETWORK_PASSPHRASE:-Test SDF Network ; September 2015}"
STELLAR_SIGNING_KEY="${STELLAR_SIGNING_KEY:-}"

WASM_DIR="${REPO_ROOT}/target/wasm32v1-none/release"
ROUTER_WASM="${WASM_DIR}/stellar_ibc_router.wasm"
TRANSFER_WASM="${WASM_DIR}/stellar_transfer_app.wasm"
MOCK_LC_WASM="${WASM_DIR}/mock_light_client.wasm"
ATTESTATION_LC_WASM="${WASM_DIR}/stellar_attestation_light_client.wasm"
TENDERMINT_LC_WASM="${WASM_DIR}/stellar_tendermint_light_client.wasm"

DEPLOY_ATTESTATION_LC="${DEPLOY_ATTESTATION_LC:-0}"
DEPLOY_TENDERMINT_LC="${DEPLOY_TENDERMINT_LC:-0}"
FORCE_REDEPLOY="${FORCE_REDEPLOY:-0}"
DEPLOYER_IDENTITY="${DEPLOYER_IDENTITY:-stellar-ibc-deployer}"
TRANSFER_PORT="${TRANSFER_PORT:-transfer}"
MOCK_CLIENT_TYPE="${MOCK_CLIENT_TYPE:-mock}"
ATTESTATION_CLIENT_TYPE="${ATTESTATION_CLIENT_TYPE:-attestation}"
TENDERMINT_CLIENT_TYPE="${TENDERMINT_CLIENT_TYPE:-07-tendermint}"

echo "=== upload-and-deploy-contracts ==="
echo "  RPC          : ${STELLAR_RPC_URL}"
echo "  Network      : ${NETWORK_PASSPHRASE}"
echo "  Deployer     : ${DEPLOYER_IDENTITY}"
echo "  Force redep  : ${FORCE_REDEPLOY}"
echo "  Attestation  : ${DEPLOY_ATTESTATION_LC}"
echo "  Tendermint   : ${DEPLOY_TENDERMINT_LC}"
echo ""

if ! command -v stellar > /dev/null 2>&1; then
  echo "ERROR: 'stellar' CLI not found in PATH."
  echo "  Install: brew install stellar-cli  (or 'cargo install --locked stellar-cli')"
  exit 1
fi

if [[ -z "${STELLAR_SIGNING_KEY}" ]]; then
  echo "ERROR: STELLAR_SIGNING_KEY is empty in .env."
  echo "  Generate one with 'stellar keys generate --network testnet --fund <name>'"
  echo "  then copy the secret (stellar keys show <name>) into .env."
  exit 1
fi

if [[ -n "${IBC_CONTRACT_ID:-}" && "${FORCE_REDEPLOY}" != "1" ]]; then
  echo "  IBC_CONTRACT_ID already set in .env: ${IBC_CONTRACT_ID}"
  echo "  Set FORCE_REDEPLOY=1 to redeploy from scratch."
  exit 0
fi

echo "Step 1: registering deployer identity '${DEPLOYER_IDENTITY}'..."
stellar keys remove "${DEPLOYER_IDENTITY}" > /dev/null 2>&1 || true
echo "${STELLAR_SIGNING_KEY}" | stellar keys add "${DEPLOYER_IDENTITY}" --secret-key > /dev/null
DEPLOYER_ADDRESS=$(stellar keys address "${DEPLOYER_IDENTITY}")
echo "  Address: ${DEPLOYER_ADDRESS}"

echo ""
echo "Step 2: ensuring deployer is funded on the target network..."
if ! stellar keys fund "${DEPLOYER_IDENTITY}" \
      --rpc-url "${STELLAR_RPC_URL}" \
      --network-passphrase "${NETWORK_PASSPHRASE}" > /dev/null 2>&1; then
  echo "  WARN: friendbot fund failed — account may already be funded, or the network has no friendbot."
fi

echo ""
echo "Step 3: building all Soroban contracts (stellar contract build)..."
cd "${REPO_ROOT}/contracts"
stellar contract build --profile contract
cd "${REPO_ROOT}"

for wasm in "${ROUTER_WASM}" "${TRANSFER_WASM}" "${MOCK_LC_WASM}"; do
  if [[ ! -f "${wasm}" ]]; then
    echo "ERROR: expected wasm not found at ${wasm}"
    exit 1
  fi
done

STELLAR_NET_FLAGS=(
  --rpc-url "${STELLAR_RPC_URL}"
  --network-passphrase "${NETWORK_PASSPHRASE}"
)

upload_wasm() {
  local label="$1"
  local wasm="$2"
  echo "  upload ${label} (${wasm})..."
  stellar contract upload \
    --source "${DEPLOYER_IDENTITY}" \
    "${STELLAR_NET_FLAGS[@]}" \
    --wasm "${wasm}" 2>&1 | tail -1
}

deploy_no_args() {
  local label="$1"
  local wasm="$2"
  echo "  deploy ${label} (${wasm})..."
  stellar contract deploy \
    --source "${DEPLOYER_IDENTITY}" \
    "${STELLAR_NET_FLAGS[@]}" \
    --wasm "${wasm}" 2>&1 | tail -1
}

deploy_with_args() {
  local label="$1"
  local wasm="$2"
  shift 2
  echo "  deploy ${label} with constructor args..."
  stellar contract deploy \
    --source "${DEPLOYER_IDENTITY}" \
    "${STELLAR_NET_FLAGS[@]}" \
    --wasm "${wasm}" \
    -- "$@" 2>&1 | tail -1
}

invoke() {
  local label="$1"
  local contract="$2"
  shift 2
  echo "  invoke ${label} on ${contract}..."
  stellar contract invoke \
    --source "${DEPLOYER_IDENTITY}" \
    "${STELLAR_NET_FLAGS[@]}" \
    --id "${contract}" \
    -- "$@"
}

echo ""
echo "Step 4: uploading + deploying contracts..."

upload_wasm "mock-light-client"  "${MOCK_LC_WASM}"        > /dev/null
upload_wasm "router"             "${ROUTER_WASM}"         > /dev/null
upload_wasm "transfer-app"       "${TRANSFER_WASM}"       > /dev/null

MOCK_LC_CONTRACT_ID=$(deploy_no_args "mock-light-client" "${MOCK_LC_WASM}")
echo "    MOCK_LC_CONTRACT_ID=${MOCK_LC_CONTRACT_ID}"

IBC_CONTRACT_ID=$(deploy_with_args "router" "${ROUTER_WASM}" \
  --admin "${DEPLOYER_ADDRESS}")
echo "    IBC_CONTRACT_ID=${IBC_CONTRACT_ID}"

TRANSFER_CONTRACT_ID=$(deploy_with_args "transfer-app" "${TRANSFER_WASM}" \
  --router "${IBC_CONTRACT_ID}" \
  --admin "${DEPLOYER_ADDRESS}")
echo "    TRANSFER_CONTRACT_ID=${TRANSFER_CONTRACT_ID}"

ATTESTATION_LC_CONTRACT_ID=""
if [[ "${DEPLOY_ATTESTATION_LC}" == "1" || "${DEPLOY_ATTESTATION_LC}" == "true" ]]; then
  upload_wasm "attestation-lc" "${ATTESTATION_LC_WASM}" > /dev/null
  ATTESTATION_LC_CONTRACT_ID=$(deploy_no_args "attestation-lc" "${ATTESTATION_LC_WASM}")
  echo "    ATTESTATION_LC_CONTRACT_ID=${ATTESTATION_LC_CONTRACT_ID}"
fi

TENDERMINT_LC_CONTRACT_ID=""
if [[ "${DEPLOY_TENDERMINT_LC}" == "1" || "${DEPLOY_TENDERMINT_LC}" == "true" ]]; then
  upload_wasm "tendermint-lc" "${TENDERMINT_LC_WASM}" > /dev/null
  TENDERMINT_LC_CONTRACT_ID=$(deploy_no_args "tendermint-lc" "${TENDERMINT_LC_WASM}")
  echo "    TENDERMINT_LC_CONTRACT_ID=${TENDERMINT_LC_CONTRACT_ID}"
fi

echo ""
echo "Step 5: wiring router (register_client_type + register_port)..."

invoke "register_client_type mock" "${IBC_CONTRACT_ID}" \
  register_client_type \
  --client_type "${MOCK_CLIENT_TYPE}" \
  --lc_address "${MOCK_LC_CONTRACT_ID}"

if [[ -n "${ATTESTATION_LC_CONTRACT_ID}" ]]; then
  invoke "register_client_type attestation" "${IBC_CONTRACT_ID}" \
    register_client_type \
    --client_type "${ATTESTATION_CLIENT_TYPE}" \
    --lc_address "${ATTESTATION_LC_CONTRACT_ID}"
fi

if [[ -n "${TENDERMINT_LC_CONTRACT_ID}" ]]; then
  invoke "register_client_type tendermint" "${IBC_CONTRACT_ID}" \
    register_client_type \
    --client_type "${TENDERMINT_CLIENT_TYPE}" \
    --lc_address "${TENDERMINT_LC_CONTRACT_ID}"
fi

invoke "register_port transfer" "${IBC_CONTRACT_ID}" \
  register_port \
  --port_id "${TRANSFER_PORT}" \
  --app_address "${TRANSFER_CONTRACT_ID}"

echo ""
echo "Step 6: writing contract IDs into ${ENV_FILE}..."
python3 - "${ENV_FILE}" \
  "IBC_CONTRACT_ID=${IBC_CONTRACT_ID}" \
  "TRANSFER_CONTRACT_ID=${TRANSFER_CONTRACT_ID}" \
  "MOCK_LC_CONTRACT_ID=${MOCK_LC_CONTRACT_ID}" \
  "ATTESTATION_LC_CONTRACT_ID=${ATTESTATION_LC_CONTRACT_ID}" \
  "TENDERMINT_LC_CONTRACT_ID=${TENDERMINT_LC_CONTRACT_ID}" \
  "DEPLOYER_ADDRESS=${DEPLOYER_ADDRESS}" <<'PY'
import sys, re, pathlib
path = pathlib.Path(sys.argv[1])
updates = {}
for arg in sys.argv[2:]:
    if "=" not in arg:
        continue
    k, v = arg.split("=", 1)
    updates[k] = v
text = path.read_text()
for key, value in updates.items():
    pattern = re.compile(rf"^{re.escape(key)}\s*=.*$", re.MULTILINE)
    if pattern.search(text):
        text = pattern.sub(f"{key}={value}", text)
    else:
        if not text.endswith("\n"):
            text += "\n"
        text += f"{key}={value}\n"
path.write_text(text)
for k, v in updates.items():
    if v:
        print(f"  {k}={v}")
PY

echo ""
echo "=== upload-and-deploy-contracts done ==="
echo "  Deployer        : ${DEPLOYER_ADDRESS}"
echo "  Router          : ${IBC_CONTRACT_ID}"
echo "  Transfer app    : ${TRANSFER_CONTRACT_ID}"
echo "  Mock LC         : ${MOCK_LC_CONTRACT_ID}"
[[ -n "${ATTESTATION_LC_CONTRACT_ID}" ]] && echo "  Attestation LC  : ${ATTESTATION_LC_CONTRACT_ID}"
[[ -n "${TENDERMINT_LC_CONTRACT_ID}" ]]  && echo "  Tendermint LC   : ${TENDERMINT_LC_CONTRACT_ID}"
echo ""
echo ".env updated. Restart any running stellar-gateway so it picks up the new IBC_CONTRACT_ID + TRANSFER_CONTRACT_ID."
