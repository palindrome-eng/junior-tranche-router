# Titan CPI template for one-way junior LP minting

`Venue::ReflectJuniorDeposit { min_lp_tokens }` executes RLP's `deposit` instruction. Its only direction is **reserve asset → junior LP token**. It uses the current route leg amount, enforces the positive minimum LP output, and signs as TitanPDA. It never calls RLP `swap`, `request_withdrawal`, or `withdraw`.

The adapter is in `programs/titan-v3-venue-template/src/instructions/venues/reflect_junior.rs`. It reads the pool index from the RLP-owned pool account. The route engine binds the leg's selected input/output ATAs to the deposit accounts, rejecting manually encoded reverse directions. Account ordering matches the standalone interface and upstream ABI capture.

From the repository root:

```sh
make build-program
make check-structure
make test-local
```

The deterministic mint-route test executes real RLP and router binaries in LiteSVM. It checks LP supply increases, minimum-output enforcement, reverse-leg rejection and preservation of existing TitanPDA balances. The optional live route suite follows only the venue's declared reserve-to-LP directions.

The program ID in Anchor.toml is the local test router ID. Deployment to Titan's router and Deposit permission provisioning are separate integration steps. See the [root README](../README.md) for complete reserve configuration and signer requirements. No private Reflect checkout is needed.
