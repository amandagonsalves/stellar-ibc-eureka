.PHONY: f0 hermes-image gateway-image api-image hermes-keys upload-lc-wasm deploy-contracts caribic-preflight cosmos-only api-doc

SCRIPT_DIR := $(dir $(abspath $(lastword $(MAKEFILE_LIST))))
FLOWS_DIR  := $(SCRIPT_DIR)flows

f0:
	@bash $(FLOWS_DIR)/f0-bootstrap.sh

hermes-image:
	@bash $(FLOWS_DIR)/build-hermes-image.sh

gateway-image:
	@bash $(FLOWS_DIR)/build-gateway-image.sh

api-image:
	@bash $(FLOWS_DIR)/build-api-image.sh

hermes-keys:
	@bash $(FLOWS_DIR)/hermes-keys.sh

upload-lc-wasm:
	@bash $(FLOWS_DIR)/upload-lc-wasm.sh

deploy-contracts:
	@bash $(FLOWS_DIR)/upload-and-deploy-contracts.sh

caribic-preflight:
	@bash $(FLOWS_DIR)/caribic-preflight.sh

cosmos-only:
	@bash $(FLOWS_DIR)/cosmos-only.sh

api-doc:
	@cargo doc -p stellar-api --no-deps --open
