//! Host counterpart of `Asset::resolve_price` at the pinned RLP revision.
//! Provider readers and composition math are bundled in `rlp-interface`. Doppler alone
//! needs a host reader because upstream calls the on-chain Clock syscall.
use crate::trading_venue::error::TradingVenueError;
use anchor_lang::prelude::{AccountInfo, Clock};
use rlp::{
    constants::*,
    helpers::*,
    states::{Asset, FLAG_CHAIN_RESOLVED, OracleProvider},
};
use solana_account::Account;
use solana_pubkey::Pubkey;

fn invalid() -> TradingVenueError {
    TradingVenueError::DeserializationFailed(
        "invalid, unresolved, stale, or uncertain RLP oracle".into(),
    )
}

pub(crate) fn resolve(
    asset: &Asset,
    keys: &[Pubkey],
    accounts: &mut [Account],
    clock: &Clock,
) -> Result<OraclePrice, TradingVenueError> {
    let count = asset.live_leg_count().map_err(|_| invalid())?;
    if asset.flags & FLAG_CHAIN_RESOLVED == 0 || keys.len() != count || accounts.len() != count {
        return Err(invalid());
    }
    let mut prices = Vec::with_capacity(count);
    for ((leg, key), account) in asset.legs[..count].iter().zip(keys).zip(accounts) {
        if *key != leg.oracle
            || leg
                .provider
                .expected_owner()
                .is_some_and(|owner| owner != account.owner)
        {
            return Err(invalid());
        }
        let ceiling = leg.provider.staleness_ceiling();
        let max_staleness = if leg.max_staleness == 0 {
            DEFAULT_ORACLE_STALENESS.min(ceiling)
        } else {
            leg.max_staleness.min(ceiling)
        };
        let info = AccountInfo::new(
            key,
            false,
            false,
            &mut account.lamports,
            &mut account.data,
            &account.owner,
            account.executable,
            account.rent_epoch,
        );
        let price = if *info.owner == DOPPLER_ORACLE_PROGRAM_ID {
            if leg.feed_id != key.to_bytes() {
                return Err(invalid());
            }
            let data = info.try_borrow_data().map_err(|_| invalid())?;
            if data.len() != 17 {
                return Err(invalid());
            }
            let slot = u64::from_le_bytes(data[..8].try_into().map_err(|_| invalid())?);
            let price = u64::from_le_bytes(data[8..16].try_into().map_err(|_| invalid())?);
            if price == 0
                || clock.slot.saturating_sub(slot)
                    > u64::from(max_staleness).min(DOPPLER_MAX_STALENESS)
            {
                return Err(invalid());
            }
            OraclePrice {
                price: price as u128,
                exponent: -(data[16] as i32),
                confidence_bps: None,
            }
        } else if Some(*info.owner) == OracleProvider::Pyth.expected_owner() {
            get_price_from_pyth(&info, clock, &leg.feed_id, max_staleness).map_err(|_| invalid())?
        } else if *info.owner == CHAINLINK_ORACLE_PROGRAM_ID {
            if leg.feed_id != key.to_bytes() {
                return Err(invalid());
            }
            get_price_from_chainlink(&info, clock, max_staleness).map_err(|_| invalid())?
        } else if *info.owner == rlp::ID {
            if leg.provider == OracleProvider::ChainlinkStreamRate {
                get_price_from_stream_rate(&info, clock, &leg.feed_id, leg.decimals, max_staleness)
                    .map_err(|_| invalid())?
            } else {
                get_price_from_stream(&info, clock, &leg.feed_id, leg.decimals, max_staleness)
                    .map_err(|_| invalid())?
            }
        } else {
            return Err(invalid());
        };
        if price
            .confidence_bps
            .is_some_and(|bps| bps > confidence_bound_bps(leg.max_confidence_bps))
        {
            return Err(invalid());
        }
        prices.push((price, leg.inverse));
    }
    let price = compose_oracle_legs(&prices).map_err(|_| invalid())?;
    if price
        .confidence_bps
        .is_some_and(|bps| bps > confidence_bound_bps(asset.max_composed_confidence_bps))
    {
        return Err(invalid());
    }
    Ok(price)
}
