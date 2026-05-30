#!/bin/bash
set -euo pipefail

SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
CI_DIR=$(cd "${SCRIPT_DIR}/.." && pwd)
REPO_ROOT=$(cd "${CI_DIR}/.." && pwd)

source "${SCRIPT_DIR}/_env.sh"
load_env_file "${REPO_ROOT}/.env"

OSMOSIS_CONFIG_JSON="${OSMOSIS_CONFIG_JSON:-${REPO_ROOT}/crates/osmosis/assets/default-config.json}"
HERMES_SERVICE="${HERMES_SERVICE:-hermes}"
HERMES_CONFIG_IN_CONTAINER="${HERMES_CONFIG_IN_CONTAINER:-/home/hermes/.hermes/config.toml}"

LOCAL_CHAIN_ID="${COSMOS_CHAIN_ID:-localosmosis}"
LOCAL_KEY_NAME="${LOCAL_KEY_NAME:-localosmosis}"
STELLAR_CHAIN_ID="${STELLAR_CHAIN_ID:-stellar-testnet}"
STELLAR_KEY_NAME="${STELLAR_KEY_NAME:-stellar-relayer}"

COMPOSE_ARGS=(--profile local --profile hermes)

echo "=== hermes-keys ==="
echo "  Config source : ${OSMOSIS_CONFIG_JSON}"
echo "  Compose svc   : ${HERMES_SERVICE}"
echo "  Hermes config : ${HERMES_CONFIG_IN_CONTAINER}"
echo "  Local chain   : ${LOCAL_CHAIN_ID} (key: ${LOCAL_KEY_NAME})"
echo "  Stellar chain : ${STELLAR_CHAIN_ID} (key: ${STELLAR_KEY_NAME})"
echo ""

if ! command -v docker > /dev/null 2>&1; then
  echo "ERROR: docker not found in PATH."
  exit 1
fi

if ! command -v jq > /dev/null 2>&1; then
  echo "ERROR: jq not found in PATH. Install: brew install jq"
  exit 1
fi

if [[ ! -f "${OSMOSIS_CONFIG_JSON}" ]]; then
  echo "ERROR: osmosis config file not found at ${OSMOSIS_CONFIG_JSON}"
  exit 1
fi

VAL_MNEMONIC=$(jq -er '.keys.val' "${OSMOSIS_CONFIG_JSON}" 2>/dev/null || true)
RELAYER_MNEMONIC=$(jq -er '.keys.relayer' "${OSMOSIS_CONFIG_JSON}" 2>/dev/null || true)

if [[ -z "${RELAYER_MNEMONIC}" || -z "${VAL_MNEMONIC}" ]]; then
  echo "ERROR: ${OSMOSIS_CONFIG_JSON} is missing .keys.val or .keys.relayer"
  exit 1
fi

cd "${REPO_ROOT}"

echo "Step 1: stopping ${HERMES_SERVICE} if running (so we can attach a one-shot import container)..."
docker compose "${COMPOSE_ARGS[@]}" stop "${HERMES_SERVICE}" > /dev/null 2>&1 || true

import_key () {
  local chain_id="$1"
  local key_name="$2"
  local mnemonic="$3"

  echo "  importing ${key_name} for ${chain_id}..."
  printf '%s\n' "${mnemonic}" \
    | docker compose "${COMPOSE_ARGS[@]}" run --rm --no-deps -T \
        --entrypoint sh "${HERMES_SERVICE}" \
        -c "cat > /tmp/m.txt && hermes --config ${HERMES_CONFIG_IN_CONTAINER} keys add --chain ${chain_id} --mnemonic-file /tmp/m.txt --key-name ${key_name} --overwrite; rc=\$?; rm -f /tmp/m.txt; exit \$rc"
}

echo ""
echo "Step 2: importing keys into the hermes-keys named volume..."
import_key "${LOCAL_CHAIN_ID}" "${LOCAL_KEY_NAME}" "${RELAYER_MNEMONIC}"
import_key "${STELLAR_CHAIN_ID}" "${STELLAR_KEY_NAME}" "${VAL_MNEMONIC}"

echo ""
echo "Step 3: starting ${HERMES_SERVICE} fresh with keys in place..."
docker compose "${COMPOSE_ARGS[@]}" up -d "${HERMES_SERVICE}" > /dev/null

echo ""
echo "Step 4: tail of ${HERMES_SERVICE} logs (last 20 lines after 5s):"
sleep 5
docker compose "${COMPOSE_ARGS[@]}" logs --tail 20 "${HERMES_SERVICE}" 2>&1 | sed 's/^/  /'

echo ""
echo "=== hermes-keys done ==="
echo "  Keys are now in volume stellar-ibc_hermes-keys (persists across restarts)."
echo "  Follow live logs    : docker compose ${COMPOSE_ARGS[*]} logs -f ${HERMES_SERVICE}"
echo "  Wipe + re-import    : docker compose ${COMPOSE_ARGS[*]} down && docker volume rm stellar-ibc_hermes-keys && make -C ci hermes-keys"
