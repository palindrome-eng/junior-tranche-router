//! RLP deposit arithmetic, with the same SPL PreciseNumber operation order.
use crate::trading_venue::error::TradingVenueError;
use spl_math::precise_number::PreciseNumber;

pub(crate) fn math_error() -> TradingVenueError {
    TradingVenueError::MathError("RLP deposit arithmetic overflow or invalid pool value".into())
}

pub fn dead_shares(lp_decimals: u8) -> Option<u64> {
    10u64.checked_pow(u32::from(lp_decimals).checked_sub(3)?)
}

/// LP atoms minted by `LiquidityPool::calculate_lp_tokens_on_deposit`.
/// Keep PreciseNumber's rounding and floor; a floating-point quote can differ
/// from execution by an atom. Reject values outside the SPL mint's u64 range
/// before converting, so malformed snapshots cannot panic in `to_imprecise`.
pub fn minted_lp_tokens(value: u128, nav: u128, supply: u64, decimals: u8) -> Option<u64> {
    if decimals > 18 || (nav == 0 && supply > dead_shares(decimals)?) {
        return None;
    }
    dead_shares(decimals)?;
    let value = PreciseNumber::new(value)?;
    let minted = if supply == 0 || nav == 0 {
        value.checked_div(&PreciseNumber::new(
            10u128.checked_pow(18 - u32::from(decimals))?,
        )?)?
    } else {
        value
            .checked_mul(&PreciseNumber::new(u128::from(supply))?)?
            .checked_div(&PreciseNumber::new(nav)?)?
    }
    .floor()?;
    if minted.greater_than(&PreciseNumber::new(u128::from(u64::MAX))?) {
        return None;
    }
    u64::try_from(minted.to_imprecise()?).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deposits_follow_nav_and_launch_rates() {
        assert_eq!(
            minted_lp_tokens(
                3_000_000_000_000_000_000,
                2_000_000_000_000_000_000,
                1_000_000,
                6
            ),
            Some(1_500_000)
        );
        assert_eq!(
            minted_lp_tokens(1_000_000_000_000_000_000, 0, 1000, 6),
            Some(1_000_000)
        );
        assert_eq!(minted_lp_tokens(1, 0, 1001, 6), None);
        assert_eq!(minted_lp_tokens(u128::MAX, 1, u64::MAX, 6), None);
        assert_eq!(minted_lp_tokens(0, 1, 1, 6), Some(0));
    }
}
