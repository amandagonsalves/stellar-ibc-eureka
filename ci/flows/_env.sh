load_env_file() {
  local file="$1"
  [[ -f "${file}" ]] || return 0
  while IFS='=' read -r key value || [[ -n "${key}" ]]; do
    [[ "${key}" =~ ^[A-Za-z_][A-Za-z0-9_]*$ ]] || continue
    if [[ -z "${!key:-}" ]]; then
      value="${value%\"}"
      value="${value#\"}"
      value="${value%\'}"
      value="${value#\'}"
      export "${key}=${value}"
    fi
  done < "${file}"
}

docker_login_if_creds() {
  if [[ "${1:-1}" != "1" && "${1:-1}" != "true" ]]; then
    return 0
  fi
  if [[ -z "${DOCKER_USERNAME:-}" || -z "${DOCKER_TOKEN:-}" ]]; then
    echo "  WARN: DOCKER_USERNAME or DOCKER_TOKEN unset in .env — skipping auto-login."
    echo "  If the push fails, run 'docker login' manually."
    return 0
  fi
  echo "  Logging in to DockerHub as ${DOCKER_USERNAME}..."
  echo "${DOCKER_TOKEN}" | docker login -u "${DOCKER_USERNAME}" --password-stdin > /dev/null
}
