#!/bin/bash
#
# Task 2: Upload `stellar-lc-wasm` to the cardano-entrypoint Cosmos chain via
# 08-wasm. Runs the full lifecycle:
#
#   1. Build `stellar-lc-wasm` for `wasm32-unknown-unknown` (release).
#   2. Lower bulk-memory ops via `wasm-opt` (the cardano-entrypoint wasmvm
#      validator rejects them; matches the workaround in entrypoint.sh).
#   3. `docker cp` the wasm into the running cardano-entrypoint container.
#   4. Submit a gov proposal `tx ibc-wasm store-code` from the `relayer` key.
#   5. Vote yes from `alice` (the genesis validator).
#   6. Wait out the voting period (must exceed `gov.params.voting_period`
#      — set to 15s in cardano-ibc-incubator's devnet config; we use 20s).
#   7. Verify the wasm checksum is registered under
#      `query ibc-wasm checksums` and echo it for use in
#      `hermes create client --wasm-checksum <hex>`.
#
# This script does NOT create or update a client — those follow-up steps need
# the stellar-testnet block added to `~/.hermes/config.toml` and a running
# gateway. Once this script prints the checksum, the next manual step is:
#
#   hermes create client \
#     --host-chain cardanoentrypoint \
#     --reference-chain stellar-testnet \
#     --client-type 08-wasm \
#     --wasm-checksum <hex from this script>
#
# Behavior:
#   - Skips (exit 0) when the chain or container isn't reachable, matching
#     the pattern in ci/tests/upload-wasm.sh, so this script can run in CI
#     without a chain available.
#
set -euo pipefail

SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
REPO_ROOT=$(cd "${SCRIPT_DIR}/.." && pwd)
CRATE="stellar-lc-wasm"
WASM_FILE="${REPO_ROOT}/target/wasm32-unknown-unknown/release/stellar_lc_wasm.wasm"
CONTAINER="cardano-entrypoint-node-prod"
CHAIN_BIN="/go/bin/cardano-entrypointd"
CHAIN_HOME="/root/.cardano-entrypoint-data/node"
CHAIN_ID="cardanoentrypoint"
NODE="tcp://localhost:26657"
REST="http://localhost:1317"
VOTING_PERIOD="${VOTING_PERIOD:-20}"

echo "=== Task 2: upload ${CRATE} to ${CHAIN_ID} ==="

# ── Phase A: Build wasm ───────────────────────────────────────────────────────

echo ""
echo "Step 1: Building ${CRATE} for wasm32-unknown-unknown (release)..."
cd "${REPO_ROOT}"
cargo build \
  --target wasm32-unknown-unknown \
  -p "${CRATE}" \
  --release

if [[ ! -f "${WASM_FILE}" ]]; then
  echo "ERROR: expected wasm artifact not found at ${WASM_FILE}"
  exit 1
fi

if command -v wasm-opt &>/dev/null; then
  echo "Step 2: Lowering bulk-memory ops via wasm-opt..."
  wasm-opt \
    --enable-bulk-memory \
    --llvm-memory-copy-fill-lowering \
    -O1 --strip-debug \
    "${WASM_FILE}" -o "${WASM_FILE}"
else
  echo "Step 2: SKIP (wasm-opt not installed — install binaryen if upload is rejected)"
fi

WASM_SIZE=$(wc -c < "${WASM_FILE}")
LOCAL_SHA=$(shasum -a 256 "${WASM_FILE}" | awk '{print $1}')
echo "  ${WASM_FILE}"
echo "  ${WASM_SIZE} bytes, sha256=${LOCAL_SHA}"

# ── Phase B: Chain + container preconditions ──────────────────────────────────

echo ""
echo "Step 3: Checking ${CHAIN_ID} is reachable at ${REST}..."
if ! curl -sf "${REST}/cosmos/base/tendermint/v1beta1/node_info" > /dev/null 2>&1; then
  echo "  SKIP: ${CHAIN_ID} REST not reachable. Start it with: caribic start --clean"
  exit 0
fi
echo "  Chain is up."

echo ""
echo "Step 4: Checking Docker container ${CONTAINER}..."
if ! docker inspect "${CONTAINER}" > /dev/null 2>&1; then
  echo "  SKIP: container '${CONTAINER}' not found. Is caribic running?"
  exit 0
fi
echo "  Container present."

# ── Phase C: Copy + submit gov proposal + vote ────────────────────────────────

echo ""
echo "Step 5: Copying wasm into container..."
docker cp "${WASM_FILE}" "${CONTAINER}:/tmp/stellar_lc_wasm.wasm"
echo "  Copied to /tmp/stellar_lc_wasm.wasm"

TX_FLAGS="--keyring-backend test --home ${CHAIN_HOME} --chain-id ${CHAIN_ID} --node ${NODE} --gas auto --gas-adjustment 1.4 -y -o json"

echo ""
echo "Step 6: Submitting governance proposal to store wasm..."
PROPOSAL_OUTPUT=$(docker exec "${CONTAINER}" \
  "${CHAIN_BIN}" tx ibc-wasm store-code /tmp/stellar_lc_wasm.wasm \
  --from relayer \
  --title "Upload Stellar light client (stellar-lc-wasm)" \
  --summary "Registers stellar_lc_wasm.wasm as the 10-stellar client type for 08-wasm" \
  --deposit "1stake" \
  ${TX_FLAGS} 2>&1) || {
  echo "ERROR: gov proposal submission failed:"
  echo "${PROPOSAL_OUTPUT}"
  exit 1
}
echo "${PROPOSAL_OUTPUT}"
echo "  Waiting 4s for proposal tx to land..."
sleep 4

echo ""
echo "Step 7: Locating the new proposal in voting period..."
PROPOSAL_ID=$(curl -sf "${REST}/cosmos/gov/v1/proposals?proposal_status=PROPOSAL_STATUS_VOTING_PERIOD" 2>/dev/null \
  | python3 -c "import sys,json; ps=json.load(sys.stdin).get('proposals',[]); print(ps[-1]['id'] if ps else '')" 2>/dev/null || true)

if [[ -z "${PROPOSAL_ID}" ]]; then
  echo "ERROR: no proposal currently in voting period."
  exit 1
fi
echo "  Proposal ID: ${PROPOSAL_ID}"

echo ""
echo "Step 8: Voting YES from alice (genesis validator)..."
docker exec "${CONTAINER}" \
  "${CHAIN_BIN}" tx gov vote "${PROPOSAL_ID}" yes \
  --from alice \
  ${TX_FLAGS} > /dev/null 2>&1

echo "  Voted YES. Waiting ${VOTING_PERIOD}s for the voting period to elapse..."
sleep "${VOTING_PERIOD}"

# ── Phase D: Verify checksum + print it ───────────────────────────────────────

echo ""
echo "Step 9: Querying registered checksums on-chain..."
CHECKSUMS=$(docker exec "${CONTAINER}" \
  "${CHAIN_BIN}" query ibc-wasm checksums \
  --node "${NODE}" -o json 2>&1) || {
  echo "ERROR: query ibc-wasm checksums failed:"
  echo "${CHECKSUMS}"
  exit 1
}
echo "${CHECKSUMS}"

ON_CHAIN_HEX=$(echo "${CHECKSUMS}" \
  | python3 -c "
import sys, json
cs = json.load(sys.stdin).get('checksums', []) or []
print('\n'.join(cs))
" 2>/dev/null | tr '[:upper:]' '[:lower:]')

if [[ -z "${ON_CHAIN_HEX}" ]]; then
  echo "ERROR: no checksums registered. Proposal probably failed to pass."
  echo "  Check: ${REST}/cosmos/gov/v1/proposals/${PROPOSAL_ID}"
  exit 1
fi

if ! echo "${ON_CHAIN_HEX}" | grep -q "${LOCAL_SHA}"; then
  echo "ERROR: local sha256 ${LOCAL_SHA} does not appear in on-chain checksums:"
  echo "${ON_CHAIN_HEX}"
  exit 1
fi

echo ""
echo "SUCCESS: ${CRATE} registered on ${CHAIN_ID}."
echo "  wasm checksum (hex): ${LOCAL_SHA}"
echo ""
echo "Next step (Task 2 phase 2):"
echo "  hermes create client \\"
echo "    --host-chain ${CHAIN_ID} \\"
echo "    --reference-chain stellar-testnet \\"
echo "    --client-type 08-wasm \\"
echo "    --wasm-checksum ${LOCAL_SHA}"
