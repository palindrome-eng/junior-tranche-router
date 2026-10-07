# RLP compatibility fixtures

These files make local validation independent of private Reflect repository access. They are committed test inputs, not binaries downloaded during a build.

`rlp.so` is the RLP SBF program compiled from `palindrome-eng/reflect-program-library` main revision `ff753a9325ec2ce4d5d6f3210fad9548d0a94f36`, with the repository's committed lockfile, default features, and no source changes. Program ID: `JrXLmS6aYJNJDVxdAfjNJE5wikT8ubf3TA9iL2JA9Av`. Built with `cargo-build-sbf 3.1.7`, platform tools `v1.52` (Rust 1.89.0). It includes the withdrawal-ticket design. It is for local LiteSVM execution; it does not assert that any live deployment currently has these bytes. No signing key is included.

`rlp-abi.json` contains serialized account data, instruction data, and account metas generated with the original RLP Git crate at `feed345963668706911839270cbcb48d6a42ca4a` before switching to the local interface. The `abi_samples()` inputs in `tests/rlp_abi.rs` produced this capture. Account data includes Anchor discriminators. The original swap entries were removed when the adapter became mint-only. Deposit instruction bytes and account metas were captured independently from the original RLP crate at `ff753a9325ec2ce4d5d6f3210fad9548d0a94f36`. The account-layout and discovery entries retain their original capture. The fixture remains fixed when testing the local implementation.

On 2026-10-07, the source review against `ff753a9` confirmed that every account layout included in `rlp-abi.json`, the deposit instruction, oracle helpers, and pool-discovery instructions are unchanged. The ABI capture is intentionally retained as an independent compatibility check. Direct and routed mint simulations use the newly compiled program above.

SHA-256:

```text
c2405a5f937cb52a5406e05fd878d810022ecb1f9ee3b7f520e86f6eb03cac5b  rlp.so
ed5341d549191068db9b7e820c613caf9f09c7ae7e2a2809176e4e8494f6995a  rlp-abi.json
```

Normal use from the repository root:

```sh
make build-program
make test-local
```

For maintainers with source access, the optional `make build-rlp RLP_SOURCE=/path/to/reflect-program-library` command checks the revision and clean source before building into `programs/rlp.so`. Validate that candidate with `make test-local RLP_PROGRAM_SO="$PWD/programs/rlp.so"`, then replace this fixture and record its new hash and toolchain. Consumers never need this regeneration step. ABI captures must likewise be generated against the original program crate, not regenerated from the local implementation being tested.
