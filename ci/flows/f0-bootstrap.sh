#!/bin/bash
set -euo pipefail

SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
CI_DIR=$(cd "${SCRIPT_DIR}/.." && pwd)
REPO_ROOT=$(cd "${CI_DIR}/.." && pwd)

source "${SCRIPT_DIR}/_env.sh"
load_env_file "${REPO_ROOT}/.env"

CHAIN_ID="${COSMOS_CHAIN_ID:-localosmosis}"
COSMOS_REST="${COSMOS_REST_URL:-http://127.0.0.1:1318}"
GATEWAY_HTTP="${GATEWAY_HTTP:-http://127.0.0.1:${STELLAR_GATEWAY_HTTP_PORT:-8101}}"
GATEWAY_GRPC="${GATEWAY_GRPC:-127.0.0.1:${STELLAR_GATEWAY_GRPC_PORT:-50052}}"
HERMES_CONFIG="${HERMES_CONFIG:-${CI_DIR}/hermes-config.toml}"

echo "=== F0: bootstrap (images + chain probes + lc-wasm upload + hermes config patch) ==="

if [[ "${SKIP_IMAGE_BUILD:-0}" != "1" ]]; then
  echo ""
  echo "Step 0a: Building + pushing stellar-gateway docker image..."
  bash "${SCRIPT_DIR}/build-gateway-image.sh"

  echo ""
  echo "Step 0b: Building + pushing hermes docker image..."
  bash "${SCRIPT_DIR}/build-hermes-image.sh"
else
  echo ""
  echo "Step 0: SKIP image build (SKIP_IMAGE_BUILD=1)."
fi

echo ""
echo "Step 1: Probing Cosmos ${CHAIN_ID} REST at ${COSMOS_REST}..."
if ! curl -sf "${COSMOS_REST}/cosmos/base/tendermint/v1beta1/node_info" > /dev/null 2>&1; then
  echo "  SKIP: ${CHAIN_ID} not reachable."
  echo "  Start it with: make -C ci cosmos-only"
  exit 0
fi
echo "  Reachable."

echo ""
echo "Step 2: Probing Stellar gateway at ${GATEWAY_HTTP}/health..."
if ! curl -sf "${GATEWAY_HTTP}/health" > /dev/null 2>&1; then
  echo "  SKIP: stellar-hermes-gateway not reachable at ${GATEWAY_HTTP}."
  echo "  Start it with: cargo run -p stellar-hermes-gateway"
  exit 0
fi
echo "  Reachable. gRPC expected at ${GATEWAY_GRPC}."

if [[ "${SKIP_CONTRACT_DEPLOY:-0}" != "1" ]]; then
  echo ""
  echo "Step 3: Build + upload + deploy + wire Soroban contracts..."
  bash "${SCRIPT_DIR}/upload-and-deploy-contracts.sh"
  load_env_file "${REPO_ROOT}/.env"
else
  echo ""
  echo "Step 3: SKIP contract deploy (SKIP_CONTRACT_DEPLOY=1)."
fi

echo ""
echo "Step 4: Upload light-client-wasm + patch hermes config..."
bash "${SCRIPT_DIR}/upload-lc-wasm.sh"

echo ""
echo "=== F0 done ==="
echo "  Cosmos chain  : ${CHAIN_ID} (reachable)"
echo "  Stellar GW    : ${GATEWAY_HTTP} (reachable, gRPC ${GATEWAY_GRPC})"
echo "  Hermes config : ${HERMES_CONFIG}"
echo ""
echo "Next flow: F1 (initial setup — create clients + register counterparties)."
