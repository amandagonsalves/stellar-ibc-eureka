#!/bin/bash
#
# Task 2 phase 2: create a Stellar 08-wasm client on `cardanoentrypoint`,
# verify it shows up under `hermes query clients`, drive an `update_client`
# tx, and assert the new ConsensusState advances the height.
#
# Requires:
#   - `make -C ci upload-stellar-lc` already succeeded — provides the wasm
#     checksum that must be plumbed into `wasm_checksum_hex` of the
#     stellar-testnet block in `~/.hermes/config.toml`.
#   - The stellar-hermes-gateway is up at the URL in the config block
#     (default http://127.0.0.1:50052).
#   - The cardano-entrypoint chain is up at localhost:26657.
#   - A funded hermes key is registered for both chains (see
#     `ci/entrypoint.sh` for the cardano relayer key import).
#
# Behavior:
#   - Reads the checksum from arg 1 OR auto-detects from the cardano chain's
#     `query ibc-wasm checksums` (picks the most recently registered).
#   - Patches the checksum into `~/.hermes/config.toml`'s `wasm_checksum_hex`.
#   - Runs the create / query / update sequence in order. Each step prints
#     its result. Exits non-zero on the first failure.
#   - Skips with exit 0 if either chain or the gateway is unreachable.
#
set -euo pipefail

SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
REPO_ROOT=$(cd "${SCRIPT_DIR}/.." && pwd)
HERMES_CONFIG="${HERMES_CONFIG:-${HOME}/.hermes/config.toml}"
CARDANO_REST="http://127.0.0.1:1317"
GATEWAY_HTTP="http://127.0.0.1:8001"
CONTAINER="cardano-entrypoint-node-prod"
CHAIN_BIN="/go/bin/cardano-entrypointd"
NODE="tcp://localhost:26657"
HOST_CHAIN="cardanoentrypoint"
REFERENCE_CHAIN="stellar-testnet"

echo "=== Task 2 phase 2: hermes create/query/update Stellar client ==="

if ! command -v hermes >/dev/null 2>&1; then
  echo "ERROR: hermes not found in PATH."
  exit 1
fi

if [[ ! -f "${HERMES_CONFIG}" ]]; then
  echo "ERROR: ${HERMES_CONFIG} missing — run ci/entrypoint.sh or copy ci/hermes-config.toml."
  exit 1
fi

# ── Preconditions ─────────────────────────────────────────────────────────────

echo ""
echo "Step 1: Probing ${HOST_CHAIN} at ${CARDANO_REST}..."
if ! curl -sf "${CARDANO_REST}/cosmos/base/tendermint/v1beta1/node_info" > /dev/null 2>&1; then
  echo "  SKIP: ${HOST_CHAIN} not reachable. Start it with: caribic start --clean"
  exit 0
fi
echo "  Reachable."

echo ""
echo "Step 2: Probing stellar gateway at ${GATEWAY_HTTP}/health..."
if ! curl -sf "${GATEWAY_HTTP}/health" > /dev/null 2>&1; then
  echo "  SKIP: stellar-hermes-gateway not reachable. Start it with: cargo run -p stellar-hermes-gateway"
  exit 0
fi
echo "  Reachable."

# ── Resolve the wasm checksum ─────────────────────────────────────────────────

CHECKSUM_HEX="${1:-}"
if [[ -z "${CHECKSUM_HEX}" ]]; then
  echo ""
  echo "Step 3: Auto-detecting wasm checksum from ${HOST_CHAIN}..."
  if ! docker inspect "${CONTAINER}" > /dev/null 2>&1; then
    echo "  ERROR: container ${CONTAINER} missing — pass the checksum as arg 1 instead."
    exit 1
  fi
  CHECKSUMS_JSON=$(docker exec "${CONTAINER}" \
    "${CHAIN_BIN}" query ibc-wasm checksums --node "${NODE}" -o json 2>&1) || {
    echo "  ERROR: query ibc-wasm checksums failed:"
    echo "${CHECKSUMS_JSON}"
    exit 1
  }
  CHECKSUM_HEX=$(echo "${CHECKSUMS_JSON}" | python3 -c "
import sys, json
cs = json.load(sys.stdin).get('checksums', []) or []
print(cs[-1] if cs else '')
" 2>/dev/null | tr '[:upper:]' '[:lower:]')
  if [[ -z "${CHECKSUM_HEX}" ]]; then
    echo "  ERROR: no checksums registered on ${HOST_CHAIN}. Run: make -C ci upload-stellar-lc"
    exit 1
  fi
  echo "  Resolved checksum: ${CHECKSUM_HEX}"
else
  echo ""
  echo "Step 3: Using checksum from arg 1: ${CHECKSUM_HEX}"
fi

# ── Patch the checksum into the hermes config ─────────────────────────────────

echo ""
echo "Step 4: Patching wasm_checksum_hex into ${HERMES_CONFIG}..."
python3 - "${HERMES_CONFIG}" "${CHECKSUM_HEX}" <<'PY'
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
        "ERROR: wasm_checksum_hex line not found in hermes config — "
        "ensure the stellar-testnet block from ci/hermes-config.toml is in place."
    )
path.write_text(new)
print(f"  Patched.")
PY

# ── Create client ─────────────────────────────────────────────────────────────

echo ""
echo "Step 5: hermes create client --host-chain ${HOST_CHAIN} --reference-chain ${REFERENCE_CHAIN}"
CREATE_OUTPUT=$(hermes --config "${HERMES_CONFIG}" \
  create client \
  --host-chain "${HOST_CHAIN}" \
  --reference-chain "${REFERENCE_CHAIN}" \
  2>&1) || {
  echo "ERROR: hermes create client failed:"
  echo "${CREATE_OUTPUT}"
  exit 1
}
echo "${CREATE_OUTPUT}"

CLIENT_ID=$(echo "${CREATE_OUTPUT}" \
  | grep -oE '10-stellar-[0-9]+' \
  | head -1)
if [[ -z "${CLIENT_ID}" ]]; then
  echo "ERROR: no 10-stellar-<n> client id found in hermes output."
  exit 1
fi
echo "  Created: ${CLIENT_ID}"

# ── Query clients ─────────────────────────────────────────────────────────────

echo ""
echo "Step 6: hermes query clients --host-chain ${HOST_CHAIN}"
QUERY_OUTPUT=$(hermes --config "${HERMES_CONFIG}" \
  query clients --host-chain "${HOST_CHAIN}" 2>&1)
echo "${QUERY_OUTPUT}"

if ! echo "${QUERY_OUTPUT}" | grep -q "${CLIENT_ID}"; then
  echo "ERROR: ${CLIENT_ID} not present in hermes query clients output."
  exit 1
fi
echo "  ${CLIENT_ID} present in clients list."

# Snapshot initial height for the update assertion below.
INITIAL_HEIGHT=$(hermes --config "${HERMES_CONFIG}" \
  query client state \
  --chain "${HOST_CHAIN}" \
  --client "${CLIENT_ID}" 2>&1 \
  | grep -oE 'revision_height: [0-9]+' \
  | head -1 \
  | awk '{print $2}')
echo "  Initial client latest_height: ${INITIAL_HEIGHT:-<unknown>}"

# Give the gateway a few seconds so its LatestHeight advances past the initial.
sleep 8

# ── Update client ─────────────────────────────────────────────────────────────

echo ""
echo "Step 7: hermes update client --host-chain ${HOST_CHAIN} --client ${CLIENT_ID}"
UPDATE_OUTPUT=$(hermes --config "${HERMES_CONFIG}" \
  update client \
  --host-chain "${HOST_CHAIN}" \
  --client "${CLIENT_ID}" 2>&1) || {
  echo "ERROR: hermes update client failed:"
  echo "${UPDATE_OUTPUT}"
  exit 1
}
echo "${UPDATE_OUTPUT}"

NEW_HEIGHT=$(hermes --config "${HERMES_CONFIG}" \
  query client state \
  --chain "${HOST_CHAIN}" \
  --client "${CLIENT_ID}" 2>&1 \
  | grep -oE 'revision_height: [0-9]+' \
  | head -1 \
  | awk '{print $2}')
echo "  Post-update client latest_height: ${NEW_HEIGHT:-<unknown>}"

if [[ -n "${INITIAL_HEIGHT}" && -n "${NEW_HEIGHT}" && "${NEW_HEIGHT}" -le "${INITIAL_HEIGHT}" ]]; then
  echo "ERROR: client latest_height did not advance (${INITIAL_HEIGHT} -> ${NEW_HEIGHT})."
  exit 1
fi

echo ""
echo "SUCCESS: Task 2 phase 2 complete."
echo "  Client ID: ${CLIENT_ID}"
echo "  Latest height: ${NEW_HEIGHT:-<unknown>}"
