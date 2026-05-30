#!/bin/bash
set -euo pipefail

SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
CI_DIR=$(cd "${SCRIPT_DIR}/.." && pwd)
REPO_ROOT=$(cd "${CI_DIR}/.." && pwd)

source "${SCRIPT_DIR}/_env.sh"
load_env_file "${REPO_ROOT}/.env"

CHAIN_ID="${COSMOS_CHAIN_ID:-localosmosis}"
COSMOS_REST="${COSMOS_REST_URL:-http://127.0.0.1:1318}"
API_URL="${STELLAR_API_URL:-http://127.0.0.1:${STELLAR_API_PORT:-8101}}"
GATEWAY_GRPC="${GATEWAY_GRPC:-127.0.0.1:${STELLAR_GATEWAY_GRPC_PORT:-50052}}"
HERMES_CONFIG="${HERMES_CONFIG:-${CI_DIR}/hermes-config.toml}"
WAIT_TIMEOUT_SEC="${WAIT_TIMEOUT_SEC:-300}"
COMPOSE="docker compose --profile local --profile hermes"

cd "${REPO_ROOT}"

if ! command -v docker > /dev/null 2>&1; then
  echo "ERROR: docker not found in PATH — required to bring up the stack."
  exit 1
fi

wait_until () {
  local label="$1" url="$2" logs="$3"
  local deadline=$((SECONDS + WAIT_TIMEOUT_SEC))
  until curl -sf "${url}" > /dev/null 2>&1; do
    if (( SECONDS >= deadline )); then
      echo "  ERROR: ${label} not reachable within ${WAIT_TIMEOUT_SEC}s."
      echo "  Check: ${logs}"
      exit 1
    fi
    sleep 5
  done
}

echo "=== F0: bootstrap (api+gateway+hermes images + chain probes + Soroban contract deploy + lc-wasm upload + hermes config patch + relayer keys) ==="

if [[ "${SKIP_IMAGE_BUILD:-0}" != "1" ]]; then
  echo ""
  echo "Step 0a: Building + pushing stellar-ibc-api docker image..."
  bash "${SCRIPT_DIR}/build-api-image.sh"

  echo ""
  echo "Step 0b: Building + pushing stellar-gateway docker image..."
  bash "${SCRIPT_DIR}/build-gateway-image.sh"

  echo ""
  echo "Step 0c: Building + pushing hermes docker image..."
  bash "${SCRIPT_DIR}/build-hermes-image.sh"
else
  echo ""
  echo "Step 0: SKIP image build (SKIP_IMAGE_BUILD=1)."
fi

echo ""
echo "Step 1: Ensuring Cosmos ${CHAIN_ID} is up at ${COSMOS_REST}..."
if ! curl -sf "${COSMOS_REST}/cosmos/base/tendermint/v1beta1/node_info" > /dev/null 2>&1; then
  echo "  Not reachable — starting the osmosis compose service..."
  ${COMPOSE} up -d osmosis
  echo "  Waiting up to ${WAIT_TIMEOUT_SEC}s for the chain to produce blocks (first run pulls the image)..."
  wait_until "${CHAIN_ID}" "${COSMOS_REST}/cosmos/base/tendermint/v1beta1/node_info" "${COMPOSE} logs osmosis"
fi
echo "  Reachable."

echo ""
echo "Step 2: Ensuring stellar-api is up at ${API_URL}/health (fronts the gateway gRPC)..."
if ! curl -sf "${API_URL}/health" > /dev/null 2>&1; then
  echo "  Not reachable — starting the api + gateway compose services..."
  ${COMPOSE} up -d api gateway
  echo "  Waiting up to ${WAIT_TIMEOUT_SEC}s for the api health endpoint..."
  wait_until "stellar-api" "${API_URL}/health" "${COMPOSE} logs api gateway"
fi
echo "  Reachable. Gateway gRPC expected at ${GATEWAY_GRPC}."

echo ""
echo "================================================================="
echo "Step 3: Soroban contracts — build + upload + deploy + wire router"
echo "================================================================="
if [[ "${SKIP_CONTRACT_DEPLOY:-0}" != "1" ]]; then
  bash "${SCRIPT_DIR}/upload-and-deploy-contracts.sh"
  load_env_file "${REPO_ROOT}/.env"

  echo ""
  echo "  Recreating api + gateway so they pick up the new IBC_CONTRACT_ID..."
  ${COMPOSE} rm -sf api gateway > /dev/null
  ${COMPOSE} up -d api gateway > /dev/null
  wait_until "stellar-api" "${API_URL}/health" "${COMPOSE} logs api gateway"
  echo "  api + gateway recreated."
else
  echo "  SKIP contract deploy (SKIP_CONTRACT_DEPLOY=1)."
fi

echo ""
echo "Step 4: Upload light-client-wasm + patch hermes config..."
if [[ "${SKIP_LC_WASM_UPLOAD:-0}" != "1" ]]; then
  bash "${SCRIPT_DIR}/upload-lc-wasm.sh"
else
  echo "  SKIP lc-wasm upload (SKIP_LC_WASM_UPLOAD=1)."
fi

echo ""
echo "Step 5: Import hermes relayer keys into the hermes-keys volume..."
if [[ "${SKIP_HERMES_KEYS:-0}" != "1" ]]; then
  bash "${SCRIPT_DIR}/hermes-keys.sh"
else
  echo "  SKIP hermes keys import (SKIP_HERMES_KEYS=1)."
fi

load_env_file "${REPO_ROOT}/.env"

echo ""
echo "=== F0 done ==="
echo "  Cosmos chain    : ${CHAIN_ID} (reachable)"
echo "  Stellar api     : ${API_URL} (reachable, gateway gRPC ${GATEWAY_GRPC})"
echo "  Hermes config   : ${HERMES_CONFIG}"
echo "  Deployer addr   : ${DEPLOYER_ADDRESS:-(unset)}"
echo "  IBC router      : ${IBC_CONTRACT_ID:-(unset)}"
echo "  Transfer app    : ${TRANSFER_CONTRACT_ID:-(unset)}"
echo "  Mock LC         : ${MOCK_LC_CONTRACT_ID:-(unset)}"
[[ -n "${ATTESTATION_LC_CONTRACT_ID:-}" ]] && echo "  Attestation LC  : ${ATTESTATION_LC_CONTRACT_ID}"
[[ -n "${TENDERMINT_LC_CONTRACT_ID:-}" ]]  && echo "  Tendermint LC   : ${TENDERMINT_LC_CONTRACT_ID}"
echo ""
echo "Next: create the Cosmos client on Stellar with: make -C ci f1-create-cosmos-client"
