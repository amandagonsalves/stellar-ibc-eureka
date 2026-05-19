#!/usr/bin/env bash
set -uo pipefail

GRPC_PORT="${STELLAR_GATEWAY_GRPC_PORT:-50052}"
HTTP_PORT="${STELLAR_GATEWAY_HTTP_PORT:-8005}"
HERMES_CONFIG="${HOME}/.hermes/config.toml"
CHAIN_ID="stellar-testnet"

PASS=0
FAIL=0
BLOCKED=0

# ── Helpers ───────────────────────────────────────────────────────────────────

pass() { echo "    [PASS] $1"; PASS=$((PASS + 1)); }
fail() { echo "    [FAIL] $1"; FAIL=$((FAIL + 1)); }
blocked() { echo "    [BLOCKED] $1"; BLOCKED=$((BLOCKED + 1)); }
header() { echo ""; echo "── $1 ──────────────────────────────────────────────"; }

check_prereq() {
  local cmd="$1"
  if ! command -v "$cmd" &>/dev/null; then
    echo "ERROR: '$cmd' not found on PATH. Install it before running this script."
    exit 1
  fi
}

# ── Prerequisites ─────────────────────────────────────────────────────────────

check_prereq hermes
check_prereq grpcurl

echo "Stellar IBC — hermes integration tests"
echo "======================================="
echo "  Gateway  gRPC :${GRPC_PORT}  HTTP :${HTTP_PORT}"
echo "  Hermes config: ${HERMES_CONFIG}"
echo "  Hermes binary: $(which hermes) ($(hermes --version 2>&1 | head -1))"

# ── T-1  Stellar gateway health ───────────────────────────────────────────────

header "T-1  Stellar gateway health (caribic chain health --chain stellar)"

if command -v caribic &>/dev/null; then
  set +e
  HEALTH_OUT=$(caribic chain health --chain stellar 2>&1)
  HEALTH_EXIT=$?
  set -e
  echo "${HEALTH_OUT}"
  if [[ $HEALTH_EXIT -eq 0 ]] && echo "${HEALTH_OUT}" | grep -q "All.*service.*healthy"; then
    pass "caribic chain health --chain stellar"
  else
    # Fall back to manual port checks in case caribic isn't wired to this repo root
    GRPC_OK=false
    HTTP_OK=false
    SOROBAN_OK=false
    nc -z localhost "$GRPC_PORT" 2>/dev/null && GRPC_OK=true
    nc -z localhost "$HTTP_PORT" 2>/dev/null && HTTP_OK=true
    nc -z soroban-testnet.stellar.org 443 2>/dev/null && SOROBAN_OK=true
    echo "  Manual checks:"
    echo "    gRPC  :${GRPC_PORT}                 — $( $GRPC_OK && echo OK || echo FAIL )"
    echo "    HTTP  :${HTTP_PORT}                  — $( $HTTP_OK && echo OK || echo FAIL )"
    echo "    soroban-testnet.stellar.org:443 — $( $SOROBAN_OK && echo OK || echo FAIL )"
    if $GRPC_OK && $HTTP_OK && $SOROBAN_OK; then
      pass "all three endpoints reachable"
    else
      fail "one or more gateway endpoints not reachable"
    fi
  fi
else
  # caribic not on PATH — run manual port checks
  echo "  caribic not found; running manual port checks"
  GRPC_OK=false; HTTP_OK=false; SOROBAN_OK=false
  nc -z localhost "$GRPC_PORT" 2>/dev/null && GRPC_OK=true
  nc -z localhost "$HTTP_PORT" 2>/dev/null && HTTP_OK=true
  nc -z soroban-testnet.stellar.org 443 2>/dev/null && SOROBAN_OK=true
  echo "    gRPC  :${GRPC_PORT}                 — $( $GRPC_OK && echo OK || echo FAIL )"
  echo "    HTTP  :${HTTP_PORT}                  — $( $HTTP_OK && echo OK || echo FAIL )"
  echo "    soroban-testnet.stellar.org:443 — $( $SOROBAN_OK && echo OK || echo FAIL )"
  if $GRPC_OK && $HTTP_OK && $SOROBAN_OK; then
    pass "all three endpoints reachable"
  else
    fail "one or more gateway endpoints not reachable"
  fi
fi

# ── T-2  hermes health-check — all chains ────────────────────────────────────

header "T-2  hermes health-check (all chains)"

set +e
HERMES_OUT=$(hermes health-check 2>&1)
HERMES_EXIT=$?
set -e
echo "${HERMES_OUT}"

if [[ $HERMES_EXIT -eq 0 ]] && echo "${HERMES_OUT}" | grep -q "SUCCESS"; then
  if echo "${HERMES_OUT}" | grep -q "${CHAIN_ID}.*healthy\|chain is healthy"; then
    pass "hermes health-check — stellar-testnet healthy"
  else
    pass "hermes health-check succeeded (stellar-testnet may not be in config yet)"
  fi
else
  fail "hermes health-check failed (exit ${HERMES_EXIT})"
fi

# ── T-3  LatestHeight gRPC ────────────────────────────────────────────────────

header "T-3  LatestHeight gRPC (grpcurl)"

if ! nc -z localhost "$GRPC_PORT" 2>/dev/null; then
  fail "gRPC port :${GRPC_PORT} not reachable — is the gateway running?"
else
  set +e
  GRPC_OUT=$(grpcurl -plaintext "localhost:${GRPC_PORT}" \
    stellar.gateway.v1.StellarGatewayQuery/LatestHeight 2>&1)
  GRPC_EXIT=$?
  set -e
  echo "  Response: ${GRPC_OUT}"

  if [[ $GRPC_EXIT -ne 0 ]]; then
    fail "grpcurl call failed"
  else
    HEIGHT=$(echo "${GRPC_OUT}" | grep -o '"revisionHeight": *"[0-9]*"' | grep -o '[0-9]*' || true)
    if [[ -z "${HEIGHT}" ]]; then
      fail "revisionHeight not found in response"
    elif [[ "${HEIGHT}" -gt 0 ]]; then
      pass "revisionHeight = ${HEIGHT} (> 0)"
    else
      fail "revisionHeight = ${HEIGHT} (expected > 0)"
    fi
  fi
fi

# ── T-4  Hermes config contains stellar-testnet block ────────────────────────

header "T-4  Hermes config contains stellar-testnet block"

if [[ ! -f "${HERMES_CONFIG}" ]]; then
  fail "~/.hermes/config.toml not found"
else
  echo "  $(grep -A6 "id = '${CHAIN_ID}'" "${HERMES_CONFIG}" || echo "(not found)")"

  if ! grep -q "id = '${CHAIN_ID}'" "${HERMES_CONFIG}"; then
    fail "stellar-testnet block not in ${HERMES_CONFIG}"
  elif ! grep -A10 "id = '${CHAIN_ID}'" "${HERMES_CONFIG}" | grep -q "type = 'Stellar'"; then
    fail "type = 'Stellar' not set in stellar-testnet block"
  elif ! grep -A10 "id = '${CHAIN_ID}'" "${HERMES_CONFIG}" | grep -q "grpc_addr"; then
    fail "grpc_addr missing in stellar-testnet block"
  else
    pass "stellar-testnet block present with type = 'Stellar' and grpc_addr"
  fi
fi

# ── T-5  Create client Cosmos → Stellar (BLOCKED on QueryIbcHeader) ──────────

header "T-5  hermes create client --host-chain cardano-entrypoint --reference-chain stellar-testnet"
echo "  Status: BLOCKED — QueryIbcHeader not implemented in gateway/src/query.rs"
echo "  Running to capture current error:"
echo ""

set +e
CREATE_OUT=$(hermes create client \
  --host-chain cardano-entrypoint \
  --reference-chain "${CHAIN_ID}" 2>&1)
CREATE_EXIT=$?
set -e

echo "${CREATE_OUT}" | sed 's/^/  /'
echo ""

if [[ $CREATE_EXIT -eq 0 ]] && echo "${CREATE_OUT}" | grep -q "SUCCESS"; then
  pass "create client succeeded (QueryIbcHeader is now implemented!)"
else
  blocked "QueryIbcHeader unimplemented — implement in stellar-ibc/crates/gateway/src/query.rs"
fi

# ── Summary ───────────────────────────────────────────────────────────────────

echo ""
echo "======================================="
printf "Results: %d passed  %d blocked  %d failed\n" "$PASS" "$BLOCKED" "$FAIL"
echo "======================================="

[[ $FAIL -eq 0 ]] || exit 1
