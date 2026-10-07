//! One-way RLP deposit CPI. No swap or withdrawal discriminator is accepted.
use crate::error::TemplateError;
use anchor_lang::{prelude::*, solana_program::instruction::Instruction};

pub const PROGRAM_ID: Pubkey = pubkey!("JrXLmS6aYJNJDVxdAfjNJE5wikT8ubf3TA9iL2JA9Av");
pub const DEPOSIT_DISCRIMINATOR: [u8; 8] = [242, 35, 198, 137, 82, 225, 242, 182];

pub fn pool_index(pool: &AccountInfo) -> Result<u8> {
    require_keys_eq!(*pool.owner, PROGRAM_ID, TemplateError::InvalidAccountData);
    let data = pool.try_borrow_data()?;
    // Anchor LiquidityPool discriminator, then bump:u8 and index:u8.
    require!(
        data.len() >= 10 && data[..8] == [66, 38, 17, 64, 188, 80, 68, 129],
        TemplateError::InvalidAccountData
    );
    Ok(data[9])
}

pub fn deposit(
    amount: u64,
    pool_index: u8,
    min_lp_tokens: u64,
    accounts: &[AccountMeta],
) -> Result<Vec<Instruction>> {
    require!(
        amount > 0 && min_lp_tokens > 0,
        TemplateError::InvalidSwapInput
    );
    // 16 fixed accounts + 1..4 NAV tuples of 4..6 accounts each.
    require!(
        (20..=40).contains(&accounts.len()),
        TemplateError::MissingRemainingAccount
    );
    require_keys_eq!(
        accounts[15].pubkey,
        PROGRAM_ID,
        TemplateError::InvalidAccountData
    );
    require_keys_neq!(
        accounts[4].pubkey,
        accounts[7].pubkey,
        TemplateError::InvalidSwapInput
    );
    let mut data = Vec::with_capacity(25);
    data.extend_from_slice(&DEPOSIT_DISCRIMINATOR);
    data.push(pool_index);
    data.extend_from_slice(&amount.to_le_bytes());
    data.extend_from_slice(&min_lp_tokens.to_le_bytes());
    Ok(vec![Instruction {
        program_id: PROGRAM_ID,
        accounts: accounts.to_vec(),
        data,
    }])
}
