#!/bin/bash
set -euo pipefail

SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
REPO_ROOT=$(cd "${SCRIPT_DIR}/.." && pwd)

HTTP_PORT="${STELLAR_GATEWAY_HTTP_PORT:-8005}"
GRPC_PORT="${STELLAR_GATEWAY_GRPC_PORT:-50052}"
GATEWAY_BIN="${REPO_ROOT}/target/release/stellar-gateway"

GATEWAY_PID=""

cleanup() {
  if [[ -n "${GATEWAY_PID}" ]]; then
    echo "==> Stopping gateway (pid ${GATEWAY_PID})..."
    kill "${GATEWAY_PID}" 2>/dev/null || true
    wait "${GATEWAY_PID}" 2>/dev/null || true
  fi
}
trap cleanup EXIT

# ── Build ─────────────────────────────────────────────────────────────────────

echo "==> Building stellar-hermes-gateway..."
cargo build --release -p stellar-hermes-gateway --manifest-path "${REPO_ROOT}/Cargo.toml"
echo "    Built: ${GATEWAY_BIN}"

# ── Start ─────────────────────────────────────────────────────────────────────

echo ""
echo "==> Starting gateway (HTTP :${HTTP_PORT}, gRPC :${GRPC_PORT})..."

STELLAR_GATEWAY_HOST=127.0.0.1 \
STELLAR_GATEWAY_HTTP_PORT="${HTTP_PORT}" \
STELLAR_GATEWAY_GRPC_PORT="${GRPC_PORT}" \
STELLAR_RPC_URL="http://127.0.0.1:1" \
STELLAR_SIGNING_KEY="SCZANGBA5AKIA4MKQHKROLIOA7JJXZC4WVQJWMF4AEKF6XKMJKF6YH3" \
IBC_CONTRACT_ID="test" \
TRANSFER_CONTRACT_ID="test" \
NETWORK_PASSPHRASE="Test SDF Network ; September 2015" \
  "${GATEWAY_BIN}" &

GATEWAY_PID=$!

# ── Wait for HTTP ─────────────────────────────────────────────────────────────

echo ""
echo "==> Waiting for HTTP server on :${HTTP_PORT}..."
for i in $(seq 1 20); do
  if curl -sf "http://127.0.0.1:${HTTP_PORT}/health" &>/dev/null; then
    break
  fi
  if ! kill -0 "${GATEWAY_PID}" 2>/dev/null; then
    echo "ERROR: gateway process exited unexpectedly"
    exit 1
  fi
  sleep 0.5
  if [[ $i -eq 20 ]]; then
    echo "ERROR: HTTP server did not become ready after 10s"
    exit 1
  fi
done

# ── HTTP health check ─────────────────────────────────────────────────────────

echo "==> HTTP health check..."
HTTP_BODY=$(curl -sf "http://127.0.0.1:${HTTP_PORT}/health")
echo "    Response: ${HTTP_BODY}"
if [[ "${HTTP_BODY}" != "Server is up." ]]; then
  echo "ERROR: unexpected HTTP health response: ${HTTP_BODY}"
  exit 1
fi
echo "    PASS"

# ── Wait for gRPC ─────────────────────────────────────────────────────────────

echo ""
echo "==> Waiting for gRPC server on :${GRPC_PORT}..."
for i in $(seq 1 20); do
  if grpcurl -plaintext -connect-timeout 1 \
      "127.0.0.1:${GRPC_PORT}" grpc.health.v1.Health/Check &>/dev/null; then
    break
  fi
  sleep 0.5
  if [[ $i -eq 20 ]]; then
    echo "ERROR: gRPC server did not become ready after 10s"
    exit 1
  fi
done

# ── gRPC health check ─────────────────────────────────────────────────────────

echo "==> gRPC health check (tonic_health)..."
GRPC_STATUS=$(grpcurl -plaintext \
  "127.0.0.1:${GRPC_PORT}" \
  grpc.health.v1.Health/Check 2>&1)
echo "    Response: ${GRPC_STATUS}"
if ! echo "${GRPC_STATUS}" | grep -q "SERVING"; then
  echo "ERROR: gRPC health check did not return SERVING"
  exit 1
fi
echo "    PASS"

# ── gRPC reflection check ─────────────────────────────────────────────────────

echo ""
echo "==> gRPC reflection: listing StellarGatewayQuery services..."
SERVICES=$(grpcurl -plaintext "127.0.0.1:${GRPC_PORT}" list 2>&1)
echo "    Services: ${SERVICES}"
if ! echo "${SERVICES}" | grep -q "stellar.gateway.v1.StellarGatewayQuery"; then
  echo "ERROR: StellarGatewayQuery not found in reflection list"
  exit 1
fi
echo "    PASS"

echo ""
echo "==> All gateway health checks passed."
