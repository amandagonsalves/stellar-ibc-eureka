#!/bin/bash
set -euo pipefail

SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
REPO_ROOT=$(cd "${SCRIPT_DIR}/../.." && pwd)
WASM_FILE="${REPO_ROOT}/target/wasm32-unknown-unknown/release/stellar_ibc_light_client.wasm"
CONTAINER="cardano-entrypoint-node-prod"
CHAIN_BIN="/go/bin/cardano-entrypointd"
CHAIN_HOME="/root/.cardano-entrypoint-data/node"
CHAIN_ID="cardano-entrypoint"
NODE="http://localhost:26657"
REST="http://localhost:1317"

echo "=== 08-wasm upload test ==="

if [[ ! -f "${WASM_FILE}" ]]; then
  echo "ERROR: WASM not found at ${WASM_FILE}"
  echo "  Run: bash ci/entrypoint.sh"
  exit 1
fi

echo "WASM: ${WASM_FILE} ($(wc -c < "${WASM_FILE}") bytes)"

echo ""
echo "Step 1: Checking ${CHAIN_ID} is reachable..."
if ! curl -sf "${NODE}/status" > /dev/null 2>&1; then
  echo "SKIP: ${CHAIN_ID} is not reachable at ${NODE}."
  echo "  Start it with: caribic start --clean"
  exit 0
fi
echo "  Chain is up."

echo ""
echo "Step 2: Checking Docker container..."
if ! docker inspect "${CONTAINER}" > /dev/null 2>&1; then
  echo "SKIP: Container '${CONTAINER}' not found. Is caribic running?"
  exit 0
fi
echo "  Container: ${CONTAINER}"

echo ""
echo "Step 3: Copying WASM into container..."
docker cp "${WASM_FILE}" "${CONTAINER}:/tmp/stellar_ibc_light_client.wasm"
echo "  Copied to /tmp/stellar_ibc_light_client.wasm"

echo ""
echo "Step 4: Uploading WASM via ibc-wasm store-code..."
TX_OUTPUT=$(docker exec "${CONTAINER}" \
  "${CHAIN_BIN}" tx ibc-wasm store-code /tmp/stellar_ibc_light_client.wasm \
  --from relayer \
  --keyring-backend test \
  --home "${CHAIN_HOME}" \
  --chain-id "${CHAIN_ID}" \
  --node "${NODE}" \
  --gas auto \
  --gas-adjustment 1.4 \
  -y 2>&1) || {
  echo "ERROR: store-code transaction failed:"
  echo "${TX_OUTPUT}"
  exit 1
}

echo "${TX_OUTPUT}"

TXHASH=$(echo "${TX_OUTPUT}" | grep -oE 'txhash: [A-F0-9]+' | awk '{print $2}' || true)
if [[ -z "${TXHASH}" ]]; then
  echo "ERROR: Could not extract txhash from output."
  exit 1
fi

echo ""
echo "Tx hash: ${TXHASH}"
echo "Waiting for tx to be included in a block..."
sleep 6

echo ""
echo "Step 5: Verifying checksum is registered on-chain..."
RESPONSE=$(curl -sf "${REST}/ibc/lightclients/wasm/v1/checksums" 2>&1)
echo "${RESPONSE}" | jq . 2>/dev/null || echo "${RESPONSE}"

if echo "${RESPONSE}" | grep -q '"checksums"'; then
  echo ""
  echo "SUCCESS: Stellar light client WASM uploaded and registered."
  echo "  The chain can now create 10-stellar clients via 08-wasm."
else
  echo "ERROR: Unexpected response from ${REST}/ibc/lightclients/wasm/v1/checksums"
  exit 1
fi
