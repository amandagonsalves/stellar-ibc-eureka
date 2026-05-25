#!/bin/bash
set -euo pipefail

SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
CI_DIR=$(cd "${SCRIPT_DIR}/.." && pwd)
REPO_ROOT=$(cd "${CI_DIR}/.." && pwd)

source "${SCRIPT_DIR}/_env.sh"
load_env_file "${REPO_ROOT}/.env"

INCUBATOR_REPO="${INCUBATOR_REPO:-$(cd "${REPO_ROOT}/.." && pwd)/cardano-ibc-incubator}"
LOCAL_CLOCK_IMAGE="${LOCAL_CLOCK_IMAGE:-cardano-node-local-clock:10.1.4-3}"
LOCAL_CLOCK_DOCKERFILE="${INCUBATOR_REPO}/chains/cardano/Dockerfile.local-clock"
LOCAL_CLOCK_CONTEXT="${INCUBATOR_REPO}/chains/cardano"

echo "=== caribic-preflight ==="
echo "  Why: caribic's setup.rs:817 runs 'docker run ${LOCAL_CLOCK_IMAGE}'"
echo "       to generate SPO genesis data, but the image build at"
echo "       start.rs:1817 fires later in the same start command. Result:"
echo "       'caribic start' fails on a clean machine with"
echo "       'Unable to find image cardano-node-local-clock:10.1.4-3 locally'."
echo "       This script builds the image up-front so caribic finds it."
echo ""

if ! command -v docker > /dev/null 2>&1; then
  echo "ERROR: docker not found in PATH."
  exit 1
fi

if [[ ! -d "${INCUBATOR_REPO}" ]]; then
  echo "ERROR: INCUBATOR_REPO=${INCUBATOR_REPO} does not exist."
  echo "  Set INCUBATOR_REPO=/path/to/cardano-ibc-incubator in .env or env."
  exit 1
fi

if [[ ! -f "${LOCAL_CLOCK_DOCKERFILE}" ]]; then
  echo "ERROR: missing ${LOCAL_CLOCK_DOCKERFILE}"
  echo "  The Dockerfile shipped with cardano-ibc-incubator should be at"
  echo "  chains/cardano/Dockerfile.local-clock. Check your checkout."
  exit 1
fi

if docker image inspect "${LOCAL_CLOCK_IMAGE}" > /dev/null 2>&1; then
  echo "  Image already present locally: ${LOCAL_CLOCK_IMAGE}"
  echo "  No build needed. You can now run: caribic start"
  exit 0
fi

echo "Step 1: docker build -t ${LOCAL_CLOCK_IMAGE} -f ${LOCAL_CLOCK_DOCKERFILE} ${LOCAL_CLOCK_CONTEXT}"
docker build \
  -t "${LOCAL_CLOCK_IMAGE}" \
  -f "${LOCAL_CLOCK_DOCKERFILE}" \
  "${LOCAL_CLOCK_CONTEXT}"

echo ""
echo "Step 2: verify image is present"
docker image inspect "${LOCAL_CLOCK_IMAGE}" --format '{{.Id}} {{index .RepoTags 0}}' \
  || { echo "ERROR: image not found after build"; exit 1; }

echo ""
echo "=== caribic-preflight done ==="
echo "  Image : ${LOCAL_CLOCK_IMAGE}"
echo ""
echo "Next: run 'caribic start' (or 'caribic --verbose 5 start' for noisy logs)"
echo "      from inside ${INCUBATOR_REPO}/caribic/."
echo ""
echo "Track upstream fix at:"
echo "  cardano-ibc-incubator/caribic/src/setup.rs:817   (early docker run)"
echo "  cardano-ibc-incubator/caribic/src/start.rs:1817  (late docker build)"
echo "  The two need to be reordered so the build happens before the run."
