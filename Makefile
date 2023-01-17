.PHONY: all build clean fmt fmt-check init linter pre-commit test

all: init build test

build:
	@echo ──────────── Build release ────────────────────
	@cargo +nightly build --release
	@ls -l ./target/wasm32-unknown-unknown/release/*.wasm

clean:
	@echo ──────────── Clean ────────────────────────────
	@rm -rvf target

fmt:
	@echo ──────────── Format ───────────────────────────
	@cargo fmt --all

fmt-check:
	@echo ──────────── Check format ─────────────────────
	@cargo fmt --all -- --check

init:
	@echo ──────────── Install toolchains ───────────────
	@rustup toolchain add nightly
	@rustup target add wasm32-unknown-unknown --toolchain nightly

linter:
	@echo ──────────── Run linter ───────────────────────
	@cargo +nightly clippy --all-targets -- --no-deps -D warnings

pre-commit: fmt linter test

test: build
	@if [ ! -f "./target/ft_main.wasm" ]; then\
	    curl -L\
	        "https://github.com/gear-dapps/sharded-fungible-token/releases/download/0.1.2/ft_main-0.1.2.opt.wasm"\
	        -o "./target/ft_main.wasm";\
	fi
	@if [ ! -f "./target/ft_logic.opt.wasm" ]; then\
	    curl -L\
	        "https://github.com/gear-dapps/sharded-fungible-token/releases/download/0.1.2/ft_logic-0.1.2.opt.wasm"\
	        -o "./target/ft_logic.opt.wasm";\
	fi
	@if [ ! -f "./target/ft_storage.opt.wasm" ]; then\
	    curl -L\
	        "https://github.com/gear-dapps/sharded-fungible-token/releases/download/0.1.2/ft_storage-0.1.2.opt.wasm"\
	        -o "./target/ft_storage.opt.wasm";\
	fi
	@if [ ! -f "./target/nft.wasm" ]; then curl -L "https://github.com/gear-dapps/non-fungible-token/releases/download/0.2.7/nft-0.2.7.opt.wasm" -o "./target/nft.wasm"; fi
	@echo ──────────── Run tests ────────────────────────
	@cargo +nightly test --release
