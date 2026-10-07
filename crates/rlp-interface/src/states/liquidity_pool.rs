use anchor_lang::prelude::*;

pub const MAX_POOL_ASSETS: usize = 4;

#[derive(InitSpace)]
#[account]
pub struct LiquidityPool {
    pub bump: u8,
    pub index: u8,
    pub lp_token: Pubkey,
    pub cooldowns: u64,
    pub cooldown_duration: u64,
    pub deposit_cap: Option<u64>,
    pub asset_count: u8,
    pub assets: [u8; MAX_POOL_ASSETS],
    /// ProxyState account this pool's junior tranche covers.
    /// When set, `slash_for_nav_coverage` is enabled and pulls from this pool's
    /// reserve of the proxy's stablecoin_mint into the proxy's vault, capped at
    /// the proxy's current mark-to-market loss (principal + commission - vault_value).
    pub protected_vault: Option<Pubkey>,
}

impl LiquidityPool {
    pub fn has_asset(&self, asset_index: u8) -> bool {
        self.assets[..self.asset_count as usize].contains(&asset_index)
    }
}
