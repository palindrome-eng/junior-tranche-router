//! Arguments in their original Borsh field order.
use anchor_lang::prelude::*;

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct DepositArgs {
    pub liquidity_pool_index: u8,
    pub amount: u64,
    pub min_lp_tokens: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct InitializeLiquidityPoolArgs {
    pub cooldown_duration: u64,
    pub deposit_cap: Option<u64>,
    pub assets: Vec<u8>,
    pub protected_vault: Option<Pubkey>,
}
