.PHONY: build push fmt test cargo-build

build:
	@if [ -z "$(SERVICE)" ]; then echo "usage: make build SERVICE=gateway|hermes|api"; exit 1; fi
	@set -a; . ./.env 2>/dev/null || true; set +a; \
	case "$(SERVICE)" in \
	  api) \
	    IMG="$${API_IMAGE:-amandagonsalvesx/stellar-eureka-api}:$${API_TAG:-latest}"; \
	    docker build -t "$$IMG" -f crates/api/Dockerfile . ;; \
	  gateway) \
	    IMG="$${GATEWAY_IMAGE:-amandagonsalvesx/stellar-eureka-gateway}:$${GATEWAY_TAG:-latest}"; \
	    docker build -t "$$IMG" -f crates/gateway/Dockerfile . ;; \
	  hermes) \
	    REPO="$${HERMES_REPO:-../hermes-relayer}"; \
	    IMG="$${HERMES_IMAGE:-amandagonsalvesx/stellar-hermes-cardano}:$${HERMES_TAG:-latest}"; \
	    if [ -n "$$DOCKER_USERNAME" ] && [ -n "$$DOCKER_TOKEN" ]; then echo "$$DOCKER_TOKEN" | docker login -u "$$DOCKER_USERNAME" --password-stdin; fi; \
	    docker buildx inspect multiarch >/dev/null 2>&1 || docker buildx create --name multiarch >/dev/null; \
	    docker buildx build --builder multiarch --platform linux/amd64,linux/arm64 -t "$$IMG" --push -f "$$REPO/ci/release/hermes.Dockerfile" "$$REPO" ;; \
	  *) echo "unknown SERVICE '$(SERVICE)' (gateway|hermes|api)"; exit 1 ;; \
	esac

push: build
	@set -a; . ./.env 2>/dev/null || true; set +a; \
	case "$(SERVICE)" in \
	  api) IMG="$${API_IMAGE:-amandagonsalvesx/stellar-eureka-api}:$${API_TAG:-latest}" ;; \
	  gateway) IMG="$${GATEWAY_IMAGE:-amandagonsalvesx/stellar-eureka-gateway}:$${GATEWAY_TAG:-latest}" ;; \
	  hermes) echo "hermes is built + pushed multi-arch by 'make build SERVICE=hermes'"; exit 0 ;; \
	  *) echo "unknown SERVICE '$(SERVICE)' (gateway|hermes|api)"; exit 1 ;; \
	esac; \
	if [ -n "$$DOCKER_USERNAME" ] && [ -n "$$DOCKER_TOKEN" ]; then echo "$$DOCKER_TOKEN" | docker login -u "$$DOCKER_USERNAME" --password-stdin; fi; \
	docker push "$$IMG"

fmt:
	cargo fmt --all

test:
	cargo test --locked

cargo-build:
	cargo build

lint:
	cargo clippy --locked --all-targets -- -D warnings -A clippy::manual_is_multiple_of -A clippy::too_many_arguments -A clippy::result_large_err

install:
	cargo run -p interstellar -- install

push-all:
	make push SERVICE=hermes && make push SERVICE=gateway && make push SERVICE=api