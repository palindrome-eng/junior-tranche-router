# Reflect junior tranche minting for Titan

This adapter supports **one direction only: an RLP pool reserve asset → the pool's junior LP token**, using RLP's atomic `deposit` instruction. LP tokens are output-only. Quotes, bounds, instruction generation, and route construction reject LP redemption and reserve-to-reserve swaps. The router also rejects manually encoded reverse legs.

RLP redemption requires a withdrawal request, cooldown, and later withdrawal, so it cannot be an atomic Titan route. There is no redemption implementation in this adapter.

The standalone interface and bundled program target `reflect-program-library` main at `ff753a9325ec2ce4d5d6f3210fad9548d0a94f36`, program ID `JrXLmS6aYJNJDVxdAfjNJE5wikT8ubf3TA9iL2JA9Av`. **Titan needs no private Reflect repository access.** Account layouts, deposit encoders, and oracle readers are in [`crates/rlp-interface`](crates/rlp-interface/README.md); the simulation binary is in [`tests/fixtures`](tests/fixtures/README.md). Dependencies are local or public crates.io packages.

## Configuration and directions

```rust,ignore
let mut venue = ReflectJuniorVenue::from_account(&pool_address, &pool_account)?
    .with_asset_mints(&all_reserve_mints)?;
venue.update_state(&accounts_cache).await?;
```

Supply **every reserve mint** in the pool, one to four distinct mints, in a stable order. Pool accounts store asset indices; the indexer must resolve those through RLP `Asset` accounts. A partial registry is rejected because it would understate pool value and overquote LP output.

Token indices `0..n` are the configured reserves; index `n` is the LP mint read from the pool account. `directions_num()` returns only `(0, n), (1, n), …`. A one-reserve pool therefore has exactly one route. The LP mint must be an initialized classic SPL mint with the pool as its mint authority.

Refresh the pool, settings, **LP mint and supply**, every reserve balance/mint/Asset, Clock sysvar, and every oracle leg. The account dependency list includes all of them. A failed refresh disables quoting until a complete refresh succeeds. Pyth, Doppler, Chainlink Data Feeds and posted Stream price/rate accounts retain RLP's validation and chained-price arithmetic. Oracle posting happens outside the adapter.

## Deposit pricing and limits

At a snapshot, deposit output is linear in the input's oracle value. The adapter mirrors RLP's integer valuation and SPL `PreciseNumber` operation order:

```text
NAV = sum(price[i].mul(reserve_balance[i], reserve_decimals[i]))
value = input_price.mul(amount, input_decimals)

existing pool: LP output = floor(value * LP supply / NAV)
launch rate:   LP output = floor(value / 10^(18 - LP decimals))
```

The launch rate applies when LP supply or NAV is zero. RLP rejects a zero-NAV pool with supply above its dead-share threshold, and the adapter does too. Mint bounds enforce the deposit cap (in LP supply atoms), SPL mint supply overflow, input reserve overflow, and arithmetic limits. The lower bound is the first input that mints at least one LP atom. Requests exceeding capacity return zero consumed/output and `not_enough_liquidity = true`.

RLP's reserve-swap fee and price-impact formula do not apply to deposits. The earlier reserve-swap size limit is therefore removed. The marginal price is the reserve-to-LP rate in raw atoms and remains constant within the snapshot's valid mint range.

## Permissions and instructions

Permissionless minting requires public `Action::Deposit`. Otherwise configure `.with_authority(titan_pda)` and grant **TitanPDA** an allowed Deposit role; the end user's permissions do not authorize its CPI. The Deposit killswitch disables minting. RLP's deposit handler does not apply the swap-specific private-asset restriction or Swap killswitch.

`deposit_instruction(request, signer, min_lp_tokens)` builds the deposit with an explicit positive LP minimum. Titan's `generate_swap_instruction()` method emits that same deposit despite the generic trait method name. `build_swap_leg()` defaults to a one-atom minimum; set `Venue::ReflectJuniorDeposit { min_lp_tokens }` to the desired per-leg minimum. The template does not add a final route-wide output minimum.

The CPI contains 16 fixed accounts (including optional permissions and event accounts), then every NAV tuple: **reserve token account, Asset, primary oracle, mint, extra oracle legs**. The LP output account is the signer's associated token account. In a route, the signer is TitanPDA and the router forwards newly minted LP tokens to the user while preserving prior custody balances.

`parse_pool_creations()` reports the output LP mint from `initialize_lp` and deposit inputs from `initialize_pool_reserve`. Merge these notifications by pool and resolve the entire reserve registry before quoting. Discovery's mint list does not imply that every pair is tradable; use `directions_num()`.

## Validation

```sh
make check-structure
make build-program
make test-local
```

Rust/Cargo is required; building the router additionally requires Solana's `cargo build-sbf`. `test-local` uses the bundled RLP binary and the freshly built router. Tests cover deposit ABI parity, all oracle providers, complete NAV, LP supply, caps, wiped/empty pools, direct mint execution, slippage failures, custody preservation and rejection of reverse routes. `make test-venue` additionally runs optimized pricing/allocation checks. The Raydium reference suite remains available via `make test-example`.

`RLP_PROGRAM_SO` optionally selects another compatible binary. Missing RLP fixtures fail tests. Ordinary Cargo tests skip routed execution if the router is not built; `make test-local` requires it. Fixture provenance and optional maintainer rebuilding instructions are in [`tests/fixtures/README.md`](tests/fixtures/README.md).

Optional live-state validation:

```sh
export SOLANA_RPC_URL=https://your-rpc
export RLP_POOL=your_pool_address
export RLP_ASSET_MINTS=reserve_a,reserve_b # every reserve, excluding the LP mint
make dump-programs
cargo test --locked --release --test your_venue -- --nocapture
cargo test --locked --manifest-path program-template/Cargo.toml --test your_venue_route -- --nocapture
```

Live suites use fresh signers and require public Deposit permissions. No deployment or RPC credentials are assumed. Program upgrades require reviewing account layouts, deposit math, oracle readers, instruction encoders and fixtures together.
