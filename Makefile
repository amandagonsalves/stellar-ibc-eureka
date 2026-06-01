.PHONY: build push fmt test cargo-build

build:
	@if [ -z "$(SERVICE)" ]; then echo "usage: make build SERVICE=gateway|hermes|api"; exit 1; fi
	@set -a; . ./.env 2>/dev/null || true; set +a; \
	case "$(SERVICE)" in \
	  api) \
	    IMG="$${API_IMAGE:-amandagonsalvesx/stellar-ibc-api}:$${API_TAG:-latest}"; \
	    docker build -t "$$IMG" -f Dockerfile . ;; \
	  gateway) \
	    IMG="$${GATEWAY_IMAGE:-amandagonsalvesx/stellar-gateway}:$${GATEWAY_TAG:-latest}"; \
	    docker build -t "$$IMG" -f Dockerfile . ;; \
	  hermes) \
	    REPO="$${HERMES_REPO:-../hermes-relayer}"; \
	    IMG="$${HERMES_IMAGE:-amandagonsalvesx/stellar-hermes-cardano}:$${HERMES_TAG:-latest}"; \
	    docker build -t "$$IMG" -f "$$REPO/ci/release/hermes.Dockerfile" "$$REPO" ;; \
	  *) echo "unknown SERVICE '$(SERVICE)' (gateway|hermes|api)"; exit 1 ;; \
	esac

push: build
	@set -a; . ./.env 2>/dev/null || true; set +a; \
	case "$(SERVICE)" in \
	  api) IMG="$${API_IMAGE:-amandagonsalvesx/stellar-ibc-api}:$${API_TAG:-latest}" ;; \
	  gateway) IMG="$${GATEWAY_IMAGE:-amandagonsalvesx/stellar-gateway}:$${GATEWAY_TAG:-latest}" ;; \
	  hermes) IMG="$${HERMES_IMAGE:-amandagonsalvesx/stellar-hermes-cardano}:$${HERMES_TAG:-latest}" ;; \
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
