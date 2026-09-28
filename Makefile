.DEFAULT_GOAL := help

CARGO ?= cargo
BINARY_NAME := mend
TARGET_DIR := target
RELEASE_BIN := $(TARGET_DIR)/release/$(BINARY_NAME)

.PHONY: help
help:
	@printf "Usage: make [target]\n\n"
	@printf "Targets:\n"
	@awk 'BEGIN {FS = ":.*?## "} /^[a-zA-Z_-]+:.*?## / {printf "  %-16s %s\n", $$1, $$2}' $(MAKEFILE_LIST)

.PHONY: check
check: ## Run workspace compiler type check
	@echo "==> Checking workspace..."
	$(CARGO) check --workspace --all-targets

.PHONY: clippy
clippy: ## Run clippy linter with strict warning denials
	@echo "==> Running clippy checks..."
	$(CARGO) clippy --workspace --all-targets -- -D warnings

.PHONY: fmt-check
fmt-check: ## Check code formatting without modifying files
	@echo "==> Checking formatting..."
	$(CARGO) fmt --all -- --check

.PHONY: fmt
fmt: ## Automatically format all Rust source files
	@echo "==> Formatting codebase..."
	$(CARGO) fmt --all

.PHONY: test
test: ## Run all unit, integration, and doc tests across the workspace
	@echo "==> Running full test suite..."
	$(CARGO) test --workspace

.PHONY: build
build: ## Build debug binary
	@echo "==> Building debug binary..."
	$(CARGO) build -p mend-cli

.PHONY: release
release: ## Build optimized, stripped release binary (<8MB target)
	@echo "==> Building optimized release binary..."
	$(CARGO) build --release -p mend-cli
	@ls -lh $(RELEASE_BIN) 2>/dev/null | awk '{print "Release binary size: " $$5}' || true

.PHONY: install
install: ## Install binary to ~/.cargo/bin
	@echo "==> Installing $(BINARY_NAME) to cargo bin path..."
	$(CARGO) install --path crates/mend-cli --force

.PHONY: init
init: install ## Install and automatically wire hook into current shell rc
	@echo "==> Initializing mend shell hook..."
	$(BINARY_NAME) init

.PHONY: clean
clean: ## Remove build artifacts and temporary cache files
	@echo "==> Cleaning build artifacts..."
	$(CARGO) clean
	@rm -f .mend_telemetry.json

.PHONY: ci
ci: fmt-check clippy test ## Full CI validation pipeline (fmt, clippy, test)
	@echo "==> All CI checks passed successfully!"
