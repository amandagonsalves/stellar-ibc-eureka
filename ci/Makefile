.PHONY: test upload-wasm health-check hermes-health docker-health docker-health-local upload-stellar-lc

SCRIPT_DIR  := $(dir $(abspath $(lastword $(MAKEFILE_LIST))))
REPO_ROOT   := $(abspath $(SCRIPT_DIR)..)
TESTS_DIR   := $(SCRIPT_DIR)tests
WASM_FILE   := $(REPO_ROOT)/target/wasm32-unknown-unknown/release/stellar_ibc_light_client.wasm


test: upload-wasm health-check
	@echo ""
	@echo "All tests passed."

upload-wasm:
	@test -f "$(WASM_FILE)" || (echo "ERROR: WASM not built. Run: bash ci/entrypoint.sh" && exit 1)
	@bash $(TESTS_DIR)/upload-wasm.sh

health-check:
	@bash $(TESTS_DIR)/health-check.sh

hermes-health:
	@bash $(SCRIPT_DIR)hermes-health.sh

upload-stellar-lc:
	@bash $(SCRIPT_DIR)upload-stellar-lc.sh
