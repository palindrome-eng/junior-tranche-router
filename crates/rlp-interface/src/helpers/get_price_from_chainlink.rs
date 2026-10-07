use super::{staleness_bound_seconds, OraclePrice};
use crate::errors::RlpError;
use anchor_lang::prelude::*;

// Chainlink Solana OCR2 store `Transmissions` account:
//
//   [0..8]     anchor discriminator
//   [8..200]   header
//     +0       version    u8   (absolute offset 8)
//     +130     decimals   u8   (absolute offset 138)
//     +135     latest_round_id u32 LE (absolute offset 143)
//     +140     live_length u32 LE (absolute offset 148)
//   [200..]    ring buffer of 48-byte rounds:
//     +0   slot        u64 LE
//     +8   timestamp   u32 LE
//     +16  answer      i128 LE
//
// The header's `state` byte at offset 9 reads 1 on every live mainnet feed. The official
// `read_feed_v2` does not inspect it. It is not a flag a consumer may gate on, and gating on
// it rejects every real feed.
//
// v2 feeds carry `live_length == 1`, so the latest round sits at a fixed offset and no ring
// traversal is needed. The official reader rejects anything else and so does this one.

const CL_DISCRIMINATOR: [u8; 8] = [96, 179, 69, 66, 128, 129, 73, 117];
const CL_HEADER_END: usize = 8 + 192;
const CL_DECIMALS: usize = 138;
const CL_LATEST_ROUND: usize = 143;
const CL_LIVE_LENGTH: usize = 148;
const CL_ENTRY_LEN: usize = 48;
const CL_T_SLOT: usize = CL_HEADER_END;
/// `Transmission.timestamp`, u32 LE seconds, at +8 of the round.
const CL_T_TIMESTAMP: usize = CL_HEADER_END + 8;
const CL_T_ANSWER: usize = CL_HEADER_END + 16;
/// Header `version`, at +0 of the 192-byte header. Every live mainnet feed reads 2.
const CL_VERSION: usize = 8;
const CL_EXPECTED_VERSION: u8 = 2;

#[inline(never)]
pub fn get_price_from_chainlink(
    oracle_account: &AccountInfo,
    clock: &Clock,
    max_staleness: u32,
) -> Result<OraclePrice> {
    let data = oracle_account.try_borrow_data()?;

    require!(
        data.len() >= CL_HEADER_END + CL_ENTRY_LEN,
        RlpError::InvalidOracle
    );

    // The account must be a Transmissions feed, not another account owned by the store
    // program that happens to be long enough to parse.
    require!(
        data[0..8] == CL_DISCRIMINATOR,
        RlpError::InvalidOracleDiscriminator
    );

    // The store writes a layout version into the header, and the official `read_feed_v2`
    // checks it. This reader parses the same bytes by hand. A discriminator names the account
    // type and not the layout. A store upgrade that moved a field keeps the discriminator and
    // changes every offset below.
    require!(
        data[CL_VERSION] == CL_EXPECTED_VERSION,
        RlpError::UnsupportedOracleFeed
    );

    let live_length =
        u32::from_le_bytes(data[CL_LIVE_LENGTH..CL_LIVE_LENGTH + 4].try_into().unwrap());
    require!(live_length == 1, RlpError::UnsupportedOracleFeed);

    let latest_round_id = u32::from_le_bytes(
        data[CL_LATEST_ROUND..CL_LATEST_ROUND + 4]
            .try_into()
            .unwrap(),
    );
    require!(latest_round_id != 0, RlpError::PriceError);

    let decimals = data[CL_DECIMALS];
    let slot = u64::from_le_bytes(data[CL_T_SLOT..CL_T_SLOT + 8].try_into().unwrap());
    let answer = i128::from_le_bytes(data[CL_T_ANSWER..CL_T_ANSWER + 16].try_into().unwrap());
    let timestamp =
        u32::from_le_bytes(data[CL_T_TIMESTAMP..CL_T_TIMESTAMP + 4].try_into().unwrap());

    require!(
        clock.slot.saturating_sub(slot) <= max_staleness as u64,
        RlpError::OracleDataTooStale
    );

    // Both clocks. The slot records when the store accepted the round. The timestamp records
    // when the observation was made, and the two are not the same instant. A store that accepts
    // a round carrying an old observation moves the slot and not the timestamp.
    //
    // The timestamp widens to the width of the clock. A value near the end of the `u32` range
    // still compares against real time. The subtraction saturates, so a timestamp in the
    // future passes rather than reading as very old.
    require!(
        clock.unix_timestamp.saturating_sub(timestamp as i64)
            <= staleness_bound_seconds(max_staleness) as i64,
        RlpError::OracleDataTooStale
    );

    require!(answer > 0, RlpError::PriceError);

    // Chainlink reports a positive `decimals`. The exponent is its negation. A round carries a
    // slot, a timestamp and an answer. There is no uncertainty to report.
    Ok(OraclePrice {
        price: answer as u128,
        exponent: -(decimals as i32),
        confidence_bps: None,
    })
}
