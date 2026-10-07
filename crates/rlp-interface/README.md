# Standalone RLP interface

This local crate supplies the RLP types and arithmetic needed by the Titan adapter. It has no Git dependencies, build scripts, program entrypoint, or deployment steps. Consumers need only this repository and public crates.io dependencies. The adapter exposes it as `reflect_junior::interface`.

Source: `palindrome-eng/reflect-program-library`, `main` revision `ff753a9325ec2ce4d5d6f3210fad9548d0a94f36`, under `rlp/programs/rlp/src`. This is a compatibility snapshot, not an automatically updated SDK.

The 2026-10-07 refresh includes the latest error enum and program fixture. The new withdrawal-ticket design leaves the deposit account layouts and arithmetic unchanged. The mint adapter uses a fresh deposit ABI capture from that revision. Cooldown and withdrawal instructions are outside this crate's scope.

- `states/`: the original account fields, Borsh enum order, permission checks, and oracle metadata. Pool CPI methods and on-chain price resolution are omitted; the adapter resolves prices with a cached Clock. Unused `EnumIter` derives are omitted.
- `helpers/`: the original integer arithmetic, composition, Pyth, Chainlink Data Feeds, and posted Stream price/rate readers. Their calculations and validation are preserved; only formatting changes. The host Doppler reader lives in `src/reflect_junior/oracle.rs` in the root crate.
- `constants.rs`: the oracle constants used by these readers, copied from the original constants module.
- `errors.rs`: the original error enum, including unused variants to preserve numeric codes.
- `instructions.rs`, `instruction.rs`, `accounts.rs`: standalone equivalents of Anchor's generated argument types, discriminators, and account-meta builders for deposit and pool discovery. These are client encoders, without the program's instruction handlers.

The original account discriminators are retained through Anchor's account type names. `tests/rlp_abi.rs` in the root crate checks every included account layout, role/action/provider enum, option encoding, instruction discriminator, and account ordering against a fixed capture from the original crate. Direct and routed deposits execute against the independently compiled RLP fixture. Run `make check-structure` and, after `make build-program`, `make test-local` from the repository root.

When adopting another RLP deployment, review changes to layouts, discriminators, permissions, constants, price readers, and deposit rounding together. Recapture the ABI using that deployment's original crate and rebuild its simulation fixture. Merely changing `SOURCE_REVISION` is insufficient.

Deposit valuation and LP math live in the root adapter, using the same public `spl-math = 0.3.0` PreciseNumber operations as RLP. Swap encoders and withdrawal encoders are not included.
