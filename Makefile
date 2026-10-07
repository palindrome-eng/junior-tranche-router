# Reflect junior tranche / Titan integration checks.
RLP_REVISION := ff753a9325ec2ce4d5d6f3210fad9548d0a94f36
RLP_PROGRAM := JrXLmS6aYJNJDVxdAfjNJE5wikT8ubf3TA9iL2JA9Av
RLP_SOURCE ?=
RLP_PROGRAM_SO ?= $(CURDIR)/tests/fixtures/rlp.so
PROGRAM := --manifest-path program-template/Cargo.toml
DUMP_URL := $(if $(SOLANA_RPC_URL),$(SOLANA_RPC_URL),m)
RAYDIUM := 675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8
PROGRAMS := $(RLP_PROGRAM) $(RAYDIUM) sspUE1vrh7xRoXxGsg7vR1zde2WdGtJRbyK9uRumBDy ssmbu3KZxgonUtjEMCKspZzxvUQCxAFnyh1rcHUeEDo

.PHONY: check-structure test-venue test-example test-local scorecard build-program build-rlp dump-programs

check-structure:
	cargo test --locked --lib --test scorecard --test venue_creation --test your_venue_creation --test rlp_abi
	cargo test --locked $(PROGRAM) --lib --test venue_parity

test-venue:
	cargo test --locked --profile release-debug --test junior -- --skip direct_deposits --nocapture
	cargo test --locked --release --test junior --test your_venue -- --nocapture
	cargo test --locked $(PROGRAM) --test your_venue_route -- --nocapture

test-example:
	cargo test --locked --release --test example --test venue_creation -- --nocapture
	cargo test --locked $(PROGRAM) --test example_route -- --nocapture

test-local:
	@test -f "$(RLP_PROGRAM_SO)" || (echo 'Missing RLP_PROGRAM_SO simulation fixture'; exit 1)
	@test -f program-template/target/deploy/titan_v3_venue_template.so || (echo 'Run make build-program'; exit 1)
	RLP_PROGRAM_SO="$(RLP_PROGRAM_SO)" cargo test --locked --test junior --test rlp_abi -- --nocapture
	RLP_PROGRAM_SO="$(RLP_PROGRAM_SO)" cargo test --locked $(PROGRAM) --test junior_route --test venue_parity -- --nocapture

scorecard:
	cargo test --locked --test scorecard -- --nocapture

build-program:
	cargo build-sbf --manifest-path program-template/programs/titan-v3-venue-template/Cargo.toml --sbf-out-dir program-template/target/deploy -- --locked

# Optional maintainer command; consumers use tests/fixtures/rlp.so.
build-rlp:
	@test -n "$(RLP_SOURCE)" || (echo 'Set RLP_SOURCE to the reflect-program-library checkout'; exit 1)
	@test "$$(git -C "$(RLP_SOURCE)" rev-parse HEAD)" = "$(RLP_REVISION)" || (echo 'RLP_SOURCE must be at $(RLP_REVISION)'; exit 1)
	@test -z "$$(git -C "$(RLP_SOURCE)" status --porcelain -- rlp/programs/rlp rlp/Cargo.toml rlp/Cargo.lock)" || (echo 'RLP source has local changes'; exit 1)
	cargo build-sbf --manifest-path "$(RLP_SOURCE)/rlp/programs/rlp/Cargo.toml" --sbf-out-dir "$(CURDIR)/programs" -- --locked

dump-programs:
	@mkdir -p programs
	@set -e; for p in $(PROGRAMS); do \
		if [ ! -f programs/$$p.so ]; then solana program dump -u "$(DUMP_URL)" $$p programs/$$p.so; fi; \
	done
