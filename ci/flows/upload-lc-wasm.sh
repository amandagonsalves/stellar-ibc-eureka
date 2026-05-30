#!/bin/bash
set -euo pipefail

SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
CI_DIR=$(cd "${SCRIPT_DIR}/.." && pwd)
REPO_ROOT=$(cd "${CI_DIR}/.." && pwd)

source "${SCRIPT_DIR}/_env.sh"
load_env_file "${REPO_ROOT}/.env"

CRATE="light-client-wasm"
WASM_FILE="${REPO_ROOT}/target/wasm32-unknown-unknown/release/light_client_wasm.wasm"
HERMES_CONFIG="${HERMES_CONFIG:-${CI_DIR}/hermes-config.toml}"
PATCH_HERMES_CONFIG="${PATCH_HERMES_CONFIG:-1}"

CHAIN_ID="${COSMOS_CHAIN_ID:-localosmosis}"
API_URL="${STELLAR_API_URL:-http://127.0.0.1:8101}"

PROPOSAL_TITLE="${PROPOSAL_TITLE:-upload-light-client-wasm: ${CRATE}}"
PROPOSAL_SUMMARY="${PROPOSAL_SUMMARY:-Registers light_client_wasm.wasm as the 10-stellar client type for 08-wasm}"
DEPOSIT_AMOUNT="${DEPOSIT_AMOUNT:-10000000}"
STORE_GAS_LIMIT="${STORE_GAS_LIMIT:-60000000}"
STORE_FEE_AMOUNT="${STORE_FEE_AMOUNT:-1800000}"
VOTE_GAS_LIMIT="${VOTE_GAS_LIMIT:-200000}"
VOTE_FEE_AMOUNT="${VOTE_FEE_AMOUNT:-10000}"
VOTE_OPTION="${VOTE_OPTION:-1}"
TX_WAIT_TIMEOUT="${TX_WAIT_TIMEOUT:-60}"
VOTING_PERIOD="${VOTING_PERIOD:-20}"
FUND_AMOUNT="${FUND_AMOUNT:-100000000}"
FUND_GAS_LIMIT="${FUND_GAS_LIMIT:-200000}"
FUND_FEE_AMOUNT="${FUND_FEE_AMOUNT:-10000}"

echo "=== upload-lc-wasm ==="
echo "  Crate         : ${CRATE}"
echo "  Wasm artifact : ${WASM_FILE}"
echo "  Cosmos chain  : ${CHAIN_ID} (via ${API_URL})"
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
echo "Step 2: Probing api at ${API_URL}/cosmos/node-info..."
if ! curl -sf "${API_URL}/cosmos/node-info" > /dev/null 2>&1; then
  echo "  SKIP: api not reachable at ${API_URL}. Start it with: docker compose --profile local up -d api"
  exit 0
fi
echo "  Reachable."

echo ""
echo "Step 3: Ensuring proposer account is funded..."
PROPOSER_ADDR=$(curl -sf "${API_URL}/cosmos/proposer" | python3 -c "import sys,json; print(json.load(sys.stdin).get('address') or '')" 2>/dev/null || true)
if [[ -z "${PROPOSER_ADDR}" || "${PROPOSER_ADDR}" == "None" ]]; then
  echo "  ERROR: api did not return a proposer address (COSMOS_PROPOSER_PRIVATE_KEY missing?)"
  exit 1
fi
echo "  Proposer: ${PROPOSER_ADDR}"
FUND_RESP=$(curl -sS --fail-with-body -X POST "${API_URL}/cosmos/bank/send" \
  -H 'content-type: application/json' \
  --max-time $((TX_WAIT_TIMEOUT + 30)) \
  -d @- <<EOF
{
  "to": "${PROPOSER_ADDR}",
  "amount": ${FUND_AMOUNT},
  "gas_limit": ${FUND_GAS_LIMIT},
  "fee_amount": ${FUND_FEE_AMOUNT},
  "skip_if_account_exists": true
}
EOF
)
echo "${FUND_RESP}" | python3 -m json.tool 2>/dev/null || echo "${FUND_RESP}"

echo ""
echo "Step 4: Submitting store-code proposal via api..."
WASM_B64=$(base64 < "${WASM_FILE}" | tr -d '\n')
STORE_RESP=$(curl -sS --fail-with-body -X POST "${API_URL}/cosmos/ibc-wasm/store-code" \
  -H 'content-type: application/json' \
  --max-time $((TX_WAIT_TIMEOUT + 30)) \
  -d @- <<EOF
{
  "wasm_base64": "${WASM_B64}",
  "title": "${PROPOSAL_TITLE}",
  "summary": "${PROPOSAL_SUMMARY}",
  "deposit_amount": ${DEPOSIT_AMOUNT},
  "gas_limit": ${STORE_GAS_LIMIT},
  "fee_amount": ${STORE_FEE_AMOUNT},
  "wait_for_landing": true,
  "wait_timeout_secs": ${TX_WAIT_TIMEOUT}
}
EOF
)
echo "${STORE_RESP}" | python3 -m json.tool 2>/dev/null || echo "${STORE_RESP}"

PROPOSAL_ID=$(echo "${STORE_RESP}" | python3 -c "import sys,json; d=json.loads(sys.stdin.read()); pid=d.get('proposal_id'); print(pid if pid is not None else '')" 2>/dev/null || true)
if [[ -z "${PROPOSAL_ID}" ]]; then
  echo "  ERROR: proposal_id not present in api response."
  exit 1
fi
echo "  Proposal ID: ${PROPOSAL_ID}"

echo ""
echo "Step 5: Voting YES on proposal ${PROPOSAL_ID} via api..."
curl -sS --fail-with-body -X POST "${API_URL}/cosmos/gov/vote" \
  -H 'content-type: application/json' \
  -d @- > /dev/null <<EOF
{
  "proposal_id": ${PROPOSAL_ID},
  "option": ${VOTE_OPTION},
  "gas_limit": ${VOTE_GAS_LIMIT},
  "fee_amount": ${VOTE_FEE_AMOUNT}
}
EOF
echo "  Voted. Waiting ${VOTING_PERIOD}s for the voting period to elapse..."
sleep "${VOTING_PERIOD}"

echo ""
echo "Step 6: Verifying checksum on-chain via api..."
ON_CHAIN_HEX=$(curl -sf "${API_URL}/cosmos/ibc-wasm/checksums" \
  | python3 -c "
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
  echo "Step 7: Patching wasm_checksum_hex via api..."
  curl -sS --fail-with-body -X POST "${API_URL}/hermes/wasm-checksum" \
    -H 'content-type: application/json' \
    -d @- <<EOF | python3 -m json.tool 2>/dev/null || true
{
  "checksum": "${LOCAL_SHA}"
}
EOF
else
  echo ""
  echo "Step 7: SKIP hermes config patch (PATCH_HERMES_CONFIG=${PATCH_HERMES_CONFIG})."
  echo "  Use the checksum manually: ${LOCAL_SHA}"
fi

echo ""
echo "=== upload-lc-wasm done ==="
echo "  Proposal ID   : ${PROPOSAL_ID}"
echo "  Wasm checksum : ${LOCAL_SHA}"
if [[ "${PATCH_HERMES_CONFIG}" == "1" || "${PATCH_HERMES_CONFIG}" == "true" ]]; then
  echo "  Hermes config : patched via api (${HERMES_CONFIG_PATH:-/etc/hermes/config.toml} in container)"
fi
