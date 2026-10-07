use super::{staleness_bound_seconds, OraclePrice};
use crate::errors::RlpError;
use crate::states::StreamPrice;
use anchor_lang::prelude::*;

/// Reads a rate this program verified from a Data Streams v7 report and stored.
///
/// The same account and the same bounds as `get_price_from_stream`, with one difference: the
/// v7 schema publishes no book, so this reader neither reads the stored confidence nor returns
/// one. A stored zero would otherwise read as a measured interval of zero width and pass every
/// bound a leg can carry.
///
/// The two readers are separate rather than one reader behind a flag, because the difference is
/// what they hand the confidence machinery and a boolean parameter would hide that from a
/// reader of `Asset::read_leg`.
#[inline(never)]
pub fn get_price_from_stream_rate(
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

    // The stored record cannot tell a reader which schema wrote it, but the feed id can: a v7
    // feed id carries `0x0007` in its first two bytes and a v3 feed id `0x0003`. Both schemas
    // write the same record at the same seed family, so without this a rate leg pointed at a v3
    // account would read a measured price and throw its spread away.
    // `build_chain` states the same rule on the leg and refuses it with `InvalidOracle`.
    // The two codes differ on purpose, so a refusal here is legible as the record being
    // the other schema rather than the leg being misconfigured.
    require!(
        stored.feed_id[0] == 0x00 && stored.feed_id[1] == 0x07,
        RlpError::UnsupportedOracleFeed
    );

    // The leg bound the account cannot supply for itself. The PDA is keyed on the feed id
    // alone, so one account serves every asset that names the feed. The exponent it carries is
    // the scale that the last poster's leg declared. An eight-decimal rate read by an
    // eighteen-decimal leg is ten orders of magnitude low, in the direction that mints.
    //
    // A zero scale is not caught here. A zero-scale leg would meet a stored exponent of zero
    // and read back cleanly. Nothing can write that record: `build_chain` refuses such a leg
    // and is the only writer of one, and the v7 `validate` refuses a zero scale at posting.
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

    Ok(OraclePrice {
        price: stored.price,
        exponent: stored.exponent,
        confidence_bps: None,
    })
}
