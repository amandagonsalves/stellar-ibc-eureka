#!/bin/bash
set -euo pipefail

SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
CI_DIR=$(cd "${SCRIPT_DIR}/.." && pwd)
REPO_ROOT=$(cd "${CI_DIR}/.." && pwd)

source "${SCRIPT_DIR}/_env.sh"
load_env_file "${REPO_ROOT}/.env"

CRATE="light-client-wasm"
WASM_FILE="${REPO_ROOT}/target/wasm32-unknown-unknown/release/stellar_lc_wasm.wasm"
HERMES_CONFIG="${HERMES_CONFIG:-${CI_DIR}/hermes-config.toml}"
PATCH_HERMES_CONFIG="${PATCH_HERMES_CONFIG:-1}"

CONTAINER="${CONTAINER:-$(docker ps -qf name=osmosisd | head -n1)}"
CHAIN_BIN="${CHAIN_BIN:-osmosisd}"
CHAIN_HOME="${CHAIN_HOME:-/osmosis/.osmosisd}"
CHAIN_ID="${COSMOS_CHAIN_ID:-localosmosis}"
NODE="${NODE:-tcp://localhost:26657}"
COSMOS_REST="${COSMOS_REST_URL:-http://127.0.0.1:1318}"
COSMOS_PROPOSER_KEY="${COSMOS_PROPOSER_KEY:-val}"
COSMOS_VOTER_KEY="${COSMOS_VOTER_KEY:-val}"
COSMOS_GAS_DENOM="${COSMOS_GAS_DENOM:-uosmo}"

VOTING_PERIOD="${VOTING_PERIOD:-20}"

echo "=== upload-lc-wasm ==="
echo "  Crate         : ${CRATE}"
echo "  Wasm artifact : ${WASM_FILE}"
echo "  Cosmos chain  : ${CHAIN_ID} (container ${CONTAINER})"
echo "  Patch hermes  : ${PATCH_HERMES_CONFIG} (${HERMES_CONFIG})"
echo ""

echo "Step 1: cargo build -p ${CRATE} --target wasm32-unknown-unknown --release"
cd "${REPO_ROOT}"
cargo build \
  --target wasm32-unknown-unknown \
  -p "${CRATE}" \
  --release

if [[ ! -f "${WASM_FILE}" ]]; then
  echo "  ERROR: expected wasm artifact not found at ${WASM_FILE}"
  exit 1
fi

if command -v wasm-opt > /dev/null 2>&1; then
  wasm-opt \
    --enable-bulk-memory \
    --llvm-memory-copy-fill-lowering \
    -O1 --strip-debug \
    "${WASM_FILE}" -o "${WASM_FILE}"
  echo "  Lowered bulk-memory ops via wasm-opt."
else
  echo "  WARN: wasm-opt not installed — install binaryen if the upload is rejected for bulk-memory."
fi

WASM_SIZE=$(wc -c < "${WASM_FILE}")
LOCAL_SHA=$(shasum -a 256 "${WASM_FILE}" | awk '{print $1}')
echo "  ${WASM_SIZE} bytes, sha256=${LOCAL_SHA}"

echo ""
echo "Step 2: Probing Cosmos REST at ${COSMOS_REST}..."
if ! curl -sf "${COSMOS_REST}/cosmos/base/tendermint/v1beta1/node_info" > /dev/null 2>&1; then
  echo "  SKIP: ${CHAIN_ID} REST not reachable. Start it with: make -C ci cosmos-only"
  exit 0
fi
echo "  Reachable."

echo ""
echo "Step 3: Checking Docker container ${CONTAINER}..."
if ! docker inspect "${CONTAINER}" > /dev/null 2>&1; then
  echo "  SKIP: container '${CONTAINER}' not found. Cosmos REST is up but container is missing."
  exit 0
fi
echo "  Container present."

echo ""
echo "Step 4: Copying wasm into container..."
docker cp "${WASM_FILE}" "${CONTAINER}:/tmp/stellar_lc_wasm.wasm"
echo "  Copied to /tmp/stellar_lc_wasm.wasm"

TX_FLAGS="--keyring-backend test --home ${CHAIN_HOME} --chain-id ${CHAIN_ID} --node ${NODE} --gas auto --gas-adjustment 1.4 -y -o json"

echo ""
echo "Step 5: Submitting governance proposal to store wasm..."
PROPOSAL_OUTPUT=$(docker exec "${CONTAINER}" \
  "${CHAIN_BIN}" tx ibc-wasm store-code /tmp/stellar_lc_wasm.wasm \
  --from "${COSMOS_PROPOSER_KEY}" \
  --title "upload-lc-wasm: ${CRATE}" \
  --summary "Registers stellar_lc_wasm.wasm as the 10-stellar client type for 08-wasm" \
  --deposit "1${COSMOS_GAS_DENOM}" \
  ${TX_FLAGS} 2>&1) || {
  echo "  ERROR: gov proposal submission failed:"
  echo "${PROPOSAL_OUTPUT}"
  exit 1
}
echo "${PROPOSAL_OUTPUT}"
echo "  Waiting 4s for proposal tx to land..."
sleep 4

echo ""
echo "Step 6: Locating the new proposal in voting period..."
PROPOSAL_ID=$(curl -sf "${COSMOS_REST}/cosmos/gov/v1/proposals?proposal_status=PROPOSAL_STATUS_VOTING_PERIOD" 2>/dev/null \
  | python3 -c "import sys,json; ps=json.load(sys.stdin).get('proposals',[]); print(ps[-1]['id'] if ps else '')" 2>/dev/null || true)

if [[ -z "${PROPOSAL_ID}" ]]; then
  echo "  ERROR: no proposal currently in voting period."
  exit 1
fi
echo "  Proposal ID: ${PROPOSAL_ID}"

echo ""
echo "Step 7: Voting YES from ${COSMOS_VOTER_KEY} (genesis validator)..."
docker exec "${CONTAINER}" \
  "${CHAIN_BIN}" tx gov vote "${PROPOSAL_ID}" yes \
  --from "${COSMOS_VOTER_KEY}" \
  ${TX_FLAGS} > /dev/null 2>&1

echo "  Voted YES. Waiting ${VOTING_PERIOD}s for the voting period to elapse..."
sleep "${VOTING_PERIOD}"

echo ""
echo "Step 8: Verifying checksum on-chain..."
CHECKSUMS=$(docker exec "${CONTAINER}" \
  "${CHAIN_BIN}" query ibc-wasm checksums \
  --node "${NODE}" -o json 2>&1) || {
  echo "  ERROR: query ibc-wasm checksums failed:"
  echo "${CHECKSUMS}"
  exit 1
}

ON_CHAIN_HEX=$(echo "${CHECKSUMS}" | python3 -c "
import sys, json
cs = json.load(sys.stdin).get('checksums', []) or []
print('\n'.join(cs))
" 2>/dev/null | tr '[:upper:]' '[:lower:]')

if [[ -z "${ON_CHAIN_HEX}" ]]; then
  echo "  ERROR: no checksums registered on ${CHAIN_ID}. Proposal probably did not pass."
  exit 1
fi

if ! echo "${ON_CHAIN_HEX}" | grep -q "${LOCAL_SHA}"; then
  echo "  ERROR: local sha256 ${LOCAL_SHA} does not appear in on-chain checksums:"
  echo "${ON_CHAIN_HEX}"
  exit 1
fi
echo "  wasm registered with checksum ${LOCAL_SHA}"

if [[ "${PATCH_HERMES_CONFIG}" == "1" || "${PATCH_HERMES_CONFIG}" == "true" ]]; then
  echo ""
  echo "Step 9: Patching wasm_checksum_hex into ${HERMES_CONFIG}..."
  if [[ ! -f "${HERMES_CONFIG}" ]]; then
    echo "  ERROR: ${HERMES_CONFIG} missing."
    exit 1
  fi
  python3 - "${HERMES_CONFIG}" "${LOCAL_SHA}" <<'PY'
import sys, re, pathlib
path = pathlib.Path(sys.argv[1])
checksum = sys.argv[2]
text = path.read_text()
new = re.sub(
    r"wasm_checksum_hex\s*=\s*'[^']*'",
    f"wasm_checksum_hex = '{checksum}'",
    text,
    count=1,
)
if new == text:
    raise SystemExit(
        "  ERROR: wasm_checksum_hex line not found in hermes config — "
        "ensure the stellar-testnet block is present."
    )
path.write_text(new)
print(f"  Patched.")
PY
else
  echo ""
  echo "Step 9: SKIP hermes config patch (PATCH_HERMES_CONFIG=${PATCH_HERMES_CONFIG})."
  echo "  Use the checksum manually: ${LOCAL_SHA}"
fi

echo ""
echo "=== upload-lc-wasm done ==="
echo "  Wasm checksum : ${LOCAL_SHA}"
if [[ "${PATCH_HERMES_CONFIG}" == "1" || "${PATCH_HERMES_CONFIG}" == "true" ]]; then
  echo "  Hermes config : ${HERMES_CONFIG} (patched)"
fi
