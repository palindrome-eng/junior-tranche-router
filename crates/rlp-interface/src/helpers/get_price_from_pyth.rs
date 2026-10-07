use super::OraclePrice;
use crate::constants::*;
use crate::errors::RlpError;
use anchor_lang::prelude::*;
use pyth_solana_receiver_sdk::price_update::{PriceUpdateV2, VerificationLevel};

#[inline(never)]
pub fn get_price_from_pyth(
    oracle_account: &AccountInfo,
    clock: &Clock,
    expected_feed_id: &[u8; 32],
    max_staleness_slots: u32,
) -> Result<OraclePrice> {
    let oracle_account_data = oracle_account.try_borrow_data()?;

    let mut data_slice: &[u8] = &oracle_account_data;
    let oracle =
        PriceUpdateV2::try_deserialize(&mut data_slice).map_err(|_| RlpError::InvalidOracle)?;

    require!(
        oracle.verification_level == VerificationLevel::Full,
        RlpError::PriceError
    );

    require!(
        &oracle.price_message.feed_id == expected_feed_id,
        RlpError::InvalidOracle
    );

    let price_timestamp = oracle.price_message.publish_time;
    let current_timestamp = clock.unix_timestamp;
    let age = current_timestamp - price_timestamp;

    // The stored bound is in slots and Pyth publishes a timestamp, so convert. A slot is
    // 400 ms. The division rounds up, so the converted bound is at most one second more
    // permissive than the slot count asked for, never less.
    let converted_age = ((max_staleness_slots as u64) * 2 + 4) / 5;
    let bound = converted_age.min(ORACLE_MAXIMUM_AGE) as i64;

    require!(age >= 0 && age <= bound, RlpError::PriceError);

    let price = oracle.price_message.price;
    let conf = oracle.price_message.conf;

    require!(price > 0, RlpError::PriceError);
    // Compare at 128 bits. A u64 product overflows above a raw price of about 1.8e15.
    require!(
        (conf as u128)
            .checked_mul(MAX_ORACLE_CONFIDENCE_RATIO as u128)
            .map_or(false, |c| c <= price as u128),
        RlpError::PriceError
    );

    // Relative uncertainty in basis points, rounded up, so a narrow interval never reports
    // zero uncertainty.
    let price_u128 = (price as u128).max(1);
    let confidence_bps =
        u16::try_from(((conf as u128) * (BPS_DENOMINATOR as u128) + price_u128 - 1) / price_u128)
            .unwrap_or(u16::MAX);

    Ok(OraclePrice {
        price: price as u128,
        exponent: oracle.price_message.exponent,
        confidence_bps: Some(confidence_bps),
    })
}
