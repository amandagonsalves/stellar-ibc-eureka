#!/bin/bash
set -euo pipefail

SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
CI_DIR=$(cd "${SCRIPT_DIR}/.." && pwd)
REPO_ROOT=$(cd "${CI_DIR}/.." && pwd)

source "${SCRIPT_DIR}/_env.sh"
load_env_file "${REPO_ROOT}/.env"

HERMES_REPO="${HERMES_REPO:-$(cd "${REPO_ROOT}/.." && pwd)/hermes-relayer}"
HERMES_IMAGE="${HERMES_IMAGE:-amandagonsalvesx/stellar-hermes-cardano}"
HERMES_TAG="${HERMES_TAG:-latest}"
HERMES_REGISTRY="${HERMES_REGISTRY:-}"
PUSH="${PUSH:-1}"
DOCKERFILE="${HERMES_REPO}/ci/release/hermes.Dockerfile"

if ! command -v docker > /dev/null 2>&1; then
  echo "ERROR: docker not found in PATH."
  exit 1
fi

if [[ ! -f "${HERMES_REPO}/Cargo.toml" ]]; then
  echo "ERROR: HERMES_REPO=${HERMES_REPO} does not look like a hermes-relayer checkout (no Cargo.toml)."
  echo "  Set HERMES_REPO=/path/to/hermes-relayer when invoking."
  exit 1
fi

if ! grep -q "ibc-relayer-cli" "${HERMES_REPO}/Cargo.toml"; then
  echo "ERROR: ${HERMES_REPO}/Cargo.toml does not reference ibc-relayer-cli."
  echo "  HERMES_REPO must point at the cardano-foundation hermes-relayer fork root."
  exit 1
fi

if [[ ! -f "${DOCKERFILE}" ]]; then
  echo "ERROR: Dockerfile missing at ${DOCKERFILE}"
  exit 1
fi

if [[ -n "${HERMES_REGISTRY}" ]]; then
  LOCAL_REF="${HERMES_IMAGE}:${HERMES_TAG}"
  REMOTE_REF="${HERMES_REGISTRY}/${HERMES_IMAGE}:${HERMES_TAG}"
else
  LOCAL_REF="${HERMES_IMAGE}:${HERMES_TAG}"
  REMOTE_REF="${LOCAL_REF}"
fi

echo "=== build-hermes-image ==="
echo "  HERMES_REPO  : ${HERMES_REPO}"
echo "  Dockerfile   : ${DOCKERFILE}"
echo "  Local ref    : ${LOCAL_REF}"
echo "  Remote ref   : ${REMOTE_REF}"
echo "  Push enabled : ${PUSH}"
echo ""

echo "Step 1: docker build -t ${LOCAL_REF} -f ${DOCKERFILE} ${HERMES_REPO}"
docker build \
  -t "${LOCAL_REF}" \
  -f "${DOCKERFILE}" \
  "${HERMES_REPO}"

echo ""
echo "Step 2: smoke-test the image with 'hermes version'"
ENV_ARG=()
if [[ -f "${REPO_ROOT}/.env" ]]; then
  ENV_ARG=(--env-file "${REPO_ROOT}/.env")
fi
set +e
docker run --rm "${ENV_ARG[@]}" "${LOCAL_REF}" version 2>&1 | head -5
set -e

if [[ "${LOCAL_REF}" != "${REMOTE_REF}" ]]; then
  echo ""
  echo "Step 3: tagging as ${REMOTE_REF}"
  docker tag "${LOCAL_REF}" "${REMOTE_REF}"
fi

if [[ "${PUSH}" == "1" || "${PUSH}" == "true" ]]; then
  echo ""
  echo "Step 4a: docker login (from .env credentials if set)"
  docker_login_if_creds "${PUSH}"

  echo ""
  echo "Step 4b: docker push ${REMOTE_REF}"
  if ! docker push "${REMOTE_REF}"; then
    echo ""
    echo "ERROR: push failed. Likely causes:"
    echo "  - DOCKER_USERNAME/DOCKER_TOKEN missing or wrong in .env"
    echo "  - HERMES_IMAGE (${HERMES_IMAGE}) does not match your DockerHub namespace"
    echo "  - repo does not exist on DockerHub (create it via the UI first)"
    exit 1
  fi
else
  echo ""
  echo "Step 4: SKIP push (PUSH=${PUSH}). Image lives locally as ${REMOTE_REF}."
fi

echo ""
echo "=== build-hermes-image done ==="
echo "  Image : ${REMOTE_REF}"
echo ""
echo "Use locally:  docker run --rm --env-file .env -v \$HOME/.hermes:/home/hermes/.hermes ${REMOTE_REF} <args>"
if [[ "${PUSH}" == "1" || "${PUSH}" == "true" ]]; then
  echo "Pull anywhere: docker pull ${REMOTE_REF}"
fi
