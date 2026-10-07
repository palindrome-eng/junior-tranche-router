use super::{mul_div_floor, scale_pow10, OraclePrice};
use crate::constants::{COMPOSITION_EXPONENT, COMPOSITION_SCALE};
use crate::errors::RlpError;
use anchor_lang::prelude::*;

/// Fold a chain of oracle legs into one price.
///
/// Each leg is `(price, inverse)`. A straight leg multiplies. An inverse leg divides. The
/// accumulator is pinned at `COMPOSITION_SCALE` after every step.
///
/// Each reader checks the staleness of its own leg before this function runs. Every leg is
/// fresh, so the result carries no slot.
#[inline(never)]
pub fn compose_oracle_legs(legs: &[(OraclePrice, bool)]) -> Result<OraclePrice> {
    require!(!legs.is_empty(), RlpError::InvalidOracleConfiguration);

    // A single straight leg passes through without scaling or rounding.
    if legs.len() == 1 && !legs[0].1 {
        require!(legs[0].0.price != 0, RlpError::PriceError);
        return Ok(OraclePrice {
            price: legs[0].0.price,
            exponent: legs[0].0.exponent,
            confidence_bps: legs[0].0.confidence_bps,
        });
    }

    let mut acc: u128 = COMPOSITION_SCALE;

    // Relative uncertainties add across a product. The sum is an upper bound. A leg that
    // publishes no confidence interval adds nothing, so the sum covers the measured legs only.
    let mut confidence_bps: Option<u16> = None;

    for (leg, inverse) in legs.iter() {
        // Lift the leg to COMPOSITION_SCALE: price * 10^(exponent + 18).
        let shift = leg
            .exponent
            .checked_add(COMPOSITION_EXPONENT)
            .ok_or(RlpError::MathOverflow)?;
        let scaled = scale_pow10(leg.price, shift)?;
        require!(scaled != 0, RlpError::PriceError);

        acc = if *inverse {
            mul_div_floor(acc, COMPOSITION_SCALE, scaled)?
        } else {
            mul_div_floor(acc, scaled, COMPOSITION_SCALE)?
        };

        if let Some(leg_bps) = leg.confidence_bps {
            confidence_bps = Some(confidence_bps.unwrap_or(0).saturating_add(leg_bps));
        }
    }

    require!(acc != 0, RlpError::PriceError);

    Ok(OraclePrice {
        price: acc,
        exponent: -COMPOSITION_EXPONENT,
        confidence_bps,
    })
}
