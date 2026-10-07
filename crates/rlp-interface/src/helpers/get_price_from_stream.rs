use super::{staleness_bound_seconds, OraclePrice};
use crate::errors::RlpError;
use crate::states::StreamPrice;
use anchor_lang::prelude::*;

/// Reads a price this program verified from a Data Streams report and stored.
///
/// Every bound applies a second time here. `post_oracle_price` bounds the report against the
/// leg configured when the report was posted. This bounds the stored price against the leg
/// that reads it now.
///
/// A `StreamPrice` written once and then abandoned stays a valid account. It holds a price
/// that stopped describing the market hours ago, and the two timestamps here catch that.
#[inline(never)]
pub fn get_price_from_stream(
    oracle_account: &AccountInfo,
    clock: &Clock,
    expected_feed_id: &[u8; 32],
    decimals: u8,
    max_staleness: u32,
) -> Result<OraclePrice> {
    let data = oracle_account.try_borrow_data()?;

    require!(data.len() == StreamPrice::LEN, RlpError::InvalidOracle);

    // `Asset`, `Settings` and every other account this program owns carry the same owner. The
    // discriminator separates a `StreamPrice` from all of them.
    require!(
        data[..8] == StreamPrice::DISCRIMINATOR[..],
        RlpError::InvalidOracleDiscriminator
    );

    let stored = StreamPrice::try_deserialize_unchecked(&mut &data[..])
        .map_err(|_| RlpError::InvalidOracle)?;

    // Binds this account to the asset being priced. A leg may name any address, so this
    // compares the feed the DON signed against the feed the leg declares.
    require!(stored.feed_id == *expected_feed_id, RlpError::InvalidOracle);

    // Validate v3 stream
    require!(
        stored.feed_id[0] == 0x00 && stored.feed_id[1] == 0x03,
        RlpError::UnsupportedOracleFeed
    );

    // The leg bound the account cannot supply for itself. The PDA is keyed on the feed id
    // alone, so one account serves every asset that names the feed. The exponent it carries is
    // the scale that the last poster's leg declared. An eight-decimal price read by an
    // eighteen-decimal leg is ten orders of magnitude low.
    //
    // `decimals` is zero only on a leg whose provider is not `ChainlinkStreamFeed`, and no v3
    // stream publishes at zero decimals. A zero therefore fails here.
    require!(
        stored.exponent == -(decimals as i32),
        RlpError::OracleScaleMismatch
    );

    // Two clocks, two different failures. `posted_slot` catches an account nobody has touched.
    // `observations_timestamp` catches a current posting of an old report. The multi-hour
    // window of the DON permits that replay and the bound of the leg refuses it.
    require!(
        clock.slot.saturating_sub(stored.posted_slot) <= max_staleness as u64,
        RlpError::OracleDataTooStale
    );
    require!(
        clock
            .unix_timestamp
            .saturating_sub(stored.observations_timestamp as i64)
            <= staleness_bound_seconds(max_staleness) as i64,
        RlpError::OracleDataTooStale
    );

    require!(stored.price > 0, RlpError::PriceError);

    // `Asset::read_leg` applies the confidence bound for every provider that publishes an
    // interval.
    Ok(OraclePrice {
        price: stored.price,
        exponent: stored.exponent,
        confidence_bps: Some(stored.confidence_bps),
    })
}
