# Venue CPI adapters

`raydium_amm.rs` is the retained Titan reference. `reflect_junior.rs` serializes only RLP's Anchor `deposit` instruction: pool index, input amount, and minimum LP tokens. It accepts the 16 fixed deposit accounts plus complete NAV tuples. The route engine verifies that the deposit input/output accounts match the declared route direction.

The signer is TitanPDA. Restricted pools need a Deposit role granted to that PDA. The instruction does not post oracle prices. `venue_parity.rs` checks the local deposit encoder and route enum serialization; the root `rlp_abi` test compares the local encoder with a capture from the original RLP crate.
