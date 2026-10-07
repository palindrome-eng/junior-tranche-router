use crate::constants::{DEFAULT_MAX_CONFIDENCE_BPS, MAX_CONFIDENCE_BPS_CEILING, SLOT_MILLIS};
use crate::errors::RlpError;
use anchor_lang::prelude::*;

/// The staleness bound of a leg, in seconds.
///
/// A leg stores the bound in slots and a report carries a timestamp. Three callers use this.
/// `stream_report::validate` applies the bound to a report before any account holds it.
/// `get_price_from_stream` applies it to the stored result. `get_price_from_chainlink` applies
/// it to the observation time of a round.
///
/// The result rounds up. A bound of one or two slots is less than one second. A bound of zero
/// seconds refuses every price, because timestamps have one-second resolution.
#[inline(always)]
pub fn staleness_bound_seconds(max_staleness: u32) -> u64 {
    (max_staleness as u64)
        .saturating_mul(SLOT_MILLIS)
        .saturating_add(999)
        / 1_000
}

/// The confidence bound a leg or a chain applies, in basis points.
///
/// Zero selects `DEFAULT_MAX_CONFIDENCE_BPS`, and any stored value is clamped to
/// `MAX_CONFIDENCE_BPS_CEILING`. `build_chain` refuses a value above the ceiling at write time,
/// and this clamps again at read time, so a stored bound above it is never obeyed.
///
/// Every site that compares a measured interval against a bound calls this: `read_leg` for a
/// single leg, `resolve_price` for the composed chain, `DataStreamsReportV3::validate` before a
/// report is stored, and `check_message_bounds` before a Pyth message is accepted. Separate
/// copies would let the bound that admits a price and the bound that later prices from it
/// disagree.
#[inline(always)]
pub fn confidence_bound_bps(max_confidence_bps: u16) -> u16 {
    if max_confidence_bps == 0 {
        DEFAULT_MAX_CONFIDENCE_BPS
    } else {
        max_confidence_bps.min(MAX_CONFIDENCE_BPS_CEILING)
    }
}

/// Scale `value` by `10^shift`. A positive shift multiplies. A negative shift divides and
/// rounds towards zero.
#[inline(never)]
pub fn scale_pow10(value: u128, shift: i32) -> Result<u128> {
    if shift >= 0 {
        let factor = 10u128
            .checked_pow(shift as u32)
            .ok_or(RlpError::MathOverflow)?;
        value
            .checked_mul(factor)
            .ok_or_else(|| RlpError::MathOverflow.into())
    } else {
        let factor = 10u128
            .checked_pow(shift.unsigned_abs())
            .ok_or(RlpError::MathOverflow)?;
        value
            .checked_div(factor)
            .ok_or_else(|| RlpError::MathOverflow.into())
    }
}

/// Compute `a * b / d` at 256-bit width, rounding down.
///
/// The product of two prices held at `COMPOSITION_SCALE` reaches `10^36`. That fits `u128`,
/// but an inverse leg divides by a scaled price and the intermediate does not. The product is
/// therefore carried in two 128-bit limbs and divided at full width.
#[inline(never)]
pub fn mul_div_floor(a: u128, b: u128, d: u128) -> Result<u128> {
    if d == 0 {
        return Err(RlpError::MathOverflow.into());
    }

    // 128 x 128 -> 256, schoolbook over 64-bit limbs.
    const MASK: u128 = u64::MAX as u128;
    let (a_hi, a_lo) = (a >> 64, a & MASK);
    let (b_hi, b_lo) = (b >> 64, b & MASK);

    let ll = a_lo * b_lo;
    let lh = a_lo * b_hi;
    let hl = a_hi * b_lo;
    let hh = a_hi * b_hi;

    let mid = (ll >> 64) + (lh & MASK) + (hl & MASK);
    let lo = (ll & MASK) | (mid << 64);
    let hi = hh + (lh >> 64) + (hl >> 64) + (mid >> 64);

    // Fast path. It applies when the odd part of `d` fits a `u64`. The general path below
    // handles every other divisor.
    let twos = d.trailing_zeros();
    let odd = d >> twos;
    if let Ok(divisor) = u64::try_from(odd) {
        let (mut s_hi, mut s_lo) = (hi, lo);
        if twos > 0 {
            s_lo = (s_lo >> twos) | (s_hi << (128 - twos));
            s_hi >>= twos;
        }

        let divisor = divisor as u128;
        let limbs = [
            (s_hi >> 64) as u64,
            s_hi as u64,
            (s_lo >> 64) as u64,
            s_lo as u64,
        ];
        let mut remainder: u128 = 0;
        let mut quotient = [0u64; 4];

        for (index, limb) in limbs.iter().enumerate() {
            // `remainder < divisor <= u64::MAX`, so `remainder << 64` cannot overflow u128.
            let current = (remainder << 64) | *limb as u128;
            quotient[index] = (current / divisor) as u64;
            remainder = current % divisor;
        }

        if quotient[0] != 0 || quotient[1] != 0 {
            return Err(RlpError::MathOverflow.into());
        }

        return Ok(((quotient[2] as u128) << 64) | quotient[3] as u128);
    }

    // General path: binary long division of the 256-bit value by `d`.
    if hi >= d {
        // The quotient would not fit u128.
        return Err(RlpError::MathOverflow.into());
    }

    let mut remainder = hi;
    let mut quotient: u128 = 0;

    for bit in (0..128).rev() {
        // `remainder` may overflow when doubled. The true value is then `2^128 + shifted`,
        // which is necessarily >= d, so the wrapping subtraction below is the correct step.
        let overflowed = remainder >> 127 == 1;
        remainder = (remainder << 1) | ((lo >> bit) & 1);

        if overflowed || remainder >= d {
            remainder = remainder.wrapping_sub(d);
            quotient |= 1u128 << bit;
        }
    }

    Ok(quotient)
}
