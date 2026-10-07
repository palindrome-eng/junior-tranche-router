//! Anchor instruction encoders. Discriminators are sha256("global:<name>")[..8].
use crate::instructions::{DepositArgs, InitializeLiquidityPoolArgs};
use anchor_lang::{prelude::*, Discriminator, InstructionData};

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct Deposit {
    pub args: DepositArgs,
}
impl Discriminator for Deposit {
    const DISCRIMINATOR: &'static [u8] = &[242, 35, 198, 137, 82, 225, 242, 182];
}
impl InstructionData for Deposit {}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct InitializeLp {
    pub args: InitializeLiquidityPoolArgs,
}
impl Discriminator for InitializeLp {
    const DISCRIMINATOR: &'static [u8] = &[110, 252, 116, 251, 81, 191, 57, 96];
}
impl InstructionData for InitializeLp {}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct InitializePoolReserve {
    pub _liquidity_pool_id: u8,
}
impl Discriminator for InitializePoolReserve {
    const DISCRIMINATOR: &'static [u8] = &[151, 225, 119, 195, 196, 190, 98, 18];
}
impl InstructionData for InitializePoolReserve {}
