//! Client account metas in Anchor order, including the optional-account sentinel.
use anchor_lang::{prelude::*, ToAccountMetas};

#[derive(AnchorSerialize)]
pub struct Deposit {
    pub signer: Pubkey,
    pub settings: Pubkey,
    pub permissions: Option<Pubkey>,
    pub liquidity_pool: Pubkey,
    pub lp_token: Pubkey,
    pub user_lp_account: Pubkey,
    pub asset: Pubkey,
    pub asset_mint: Pubkey,
    pub user_asset_account: Pubkey,
    pub pool_asset_account: Pubkey,
    pub oracle: Pubkey,
    pub token_program: Pubkey,
    pub associated_token_program: Pubkey,
    pub system_program: Pubkey,
    pub event_authority: Pubkey,
    pub program: Pubkey,
}

impl ToAccountMetas for Deposit {
    fn to_account_metas(&self, _is_signer: Option<bool>) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new(self.signer, true),
            AccountMeta::new_readonly(self.settings, false),
            AccountMeta::new_readonly(self.permissions.unwrap_or(crate::ID), false),
            AccountMeta::new_readonly(self.liquidity_pool, false),
            AccountMeta::new(self.lp_token, false),
            AccountMeta::new(self.user_lp_account, false),
            AccountMeta::new_readonly(self.asset, false),
            AccountMeta::new(self.asset_mint, false),
            AccountMeta::new(self.user_asset_account, false),
            AccountMeta::new(self.pool_asset_account, false),
            AccountMeta::new_readonly(self.oracle, false),
            AccountMeta::new_readonly(self.token_program, false),
            AccountMeta::new_readonly(self.associated_token_program, false),
            AccountMeta::new_readonly(self.system_program, false),
            AccountMeta::new_readonly(self.event_authority, false),
            AccountMeta::new_readonly(self.program, false),
        ]
    }
}

#[derive(AnchorSerialize)]
pub struct InitializePoolReserve {
    pub signer: Pubkey,
    pub permissions: Pubkey,
    pub settings: Pubkey,
    pub liquidity_pool: Pubkey,
    pub asset: Pubkey,
    pub asset_mint: Pubkey,
    pub pool_asset_account: Pubkey,
    pub system_program: Pubkey,
    pub token_program: Pubkey,
    pub associated_token_program: Pubkey,
}

impl ToAccountMetas for InitializePoolReserve {
    fn to_account_metas(&self, _is_signer: Option<bool>) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new(self.signer, true),
            AccountMeta::new_readonly(self.permissions, false),
            AccountMeta::new_readonly(self.settings, false),
            AccountMeta::new_readonly(self.liquidity_pool, false),
            AccountMeta::new_readonly(self.asset, false),
            AccountMeta::new_readonly(self.asset_mint, false),
            AccountMeta::new(self.pool_asset_account, false),
            AccountMeta::new_readonly(self.system_program, false),
            AccountMeta::new_readonly(self.token_program, false),
            AccountMeta::new_readonly(self.associated_token_program, false),
        ]
    }
}
