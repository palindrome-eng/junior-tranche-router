use super::{RLP_PROGRAM_ID, asset_address, settings_address};
use crate::trading_venue::{
    protocol::PoolProtocol,
    venue_creation::{ParsedInstruction, PoolCreation},
};
use anchor_lang::{AnchorDeserialize, Discriminator};
use rlp::instructions::InitializeLiquidityPoolArgs;
use solana_pubkey::Pubkey;

/// Detect pool initialization and reserve initialization (including CPI).
/// `initialize_lp` identifies the output LP mint; reserve initializations identify
/// deposit inputs. Merge notifications by pool, then resolve all pool reserve
/// mints through its Asset registry. Only the venue's reserve-to-LP directions
/// are routable; this discovery list itself does not imply bidirectional swaps.
pub fn parse_pool_creations(instructions: &[ParsedInstruction]) -> Vec<PoolCreation> {
    let mut pools: Vec<PoolCreation> = vec![];
    let mut lp_mints = vec![];
    for ix in instructions {
        if ix.program_id != RLP_PROGRAM_ID {
            continue;
        }
        let mint;
        let pool = if ix
            .data
            .starts_with(rlp::instruction::InitializeLp::DISCRIMINATOR)
        {
            if ix.accounts.len() < 11 || ix.data.len() > 100 || ix.accounts[2] != settings_address()
            {
                continue;
            }
            let Ok(args) = InitializeLiquidityPoolArgs::try_from_slice(&ix.data[8..]) else {
                continue;
            };
            if args.assets.is_empty()
                || args.assets.len() > 4
                || args
                    .assets
                    .iter()
                    .enumerate()
                    .any(|(i, a)| args.assets[..i].contains(a))
            {
                continue;
            }
            if ix.accounts[4] == Pubkey::default() || ix.accounts[4] == ix.accounts[3] {
                continue;
            }
            mint = Some(ix.accounts[4]);
            lp_mints.push((ix.accounts[3], ix.accounts[4]));
            ix.accounts[3]
        } else if ix
            .data
            .starts_with(rlp::instruction::InitializePoolReserve::DISCRIMINATOR)
        {
            if ix.data.len() != 9 || ix.accounts.len() < 10 || ix.accounts[2] != settings_address()
            {
                continue;
            }
            let expected =
                Pubkey::find_program_address(&[b"liquidity_pool", &ix.data[8..9]], &RLP_PROGRAM_ID)
                    .0;
            if ix.accounts[3] != expected
                || ix.accounts[4] != asset_address(&ix.accounts[5])
                || ix.accounts[6]
                    != spl_associated_token_account::get_associated_token_address(
                        &expected,
                        &ix.accounts[5],
                    )
            {
                continue;
            }
            mint = Some(ix.accounts[5]);
            expected
        } else {
            continue;
        };
        let position = pools
            .iter()
            .position(|p| p.pool == pool)
            .unwrap_or_else(|| {
                pools.push(PoolCreation {
                    protocol: PoolProtocol::ReflectJunior,
                    pool,
                    mints: vec![],
                });
                pools.len() - 1
            });
        if let Some(mint) = mint {
            if !pools[position].mints.contains(&mint) {
                pools[position].mints.push(mint);
            }
        }
    }
    for pool in &mut pools {
        if let Some((_, lp)) = lp_mints.iter().find(|(key, _)| *key == pool.pool) {
            pool.mints.retain(|mint| mint != lp);
            pool.mints.push(*lp);
        }
    }
    pools
}
