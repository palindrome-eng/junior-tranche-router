use crate::{constants::PRECISION, errors::RlpError};

/// One price, already checked for freshness by the reader that produced it.
///
/// `price * 10^exponent` is the real price. `price` is a `u128` because a composed chain is
/// held at `COMPOSITION_SCALE` (10^18), and a price above 18.4 does not fit a `u64` there.
#[derive(Debug, Clone, Copy)]
pub struct OraclePrice {
    pub price: u128,
    pub exponent: i32,
    /// Confidence interval of this price in basis points, when the provider publishes one.
    ///
    /// `None` means the provider publishes no interval. Chainlink aggregates node responses
    /// into a median and emits only that. Doppler emits only a price. `None` means no
    /// measurement. It does not mean zero uncertainty.
    pub confidence_bps: Option<u16>,
}

impl OraclePrice {
    /// Value of `amount` tokens, normalised to `PRECISION` decimals.
    #[inline(never)]
    pub fn mul(&self, amount: u64, token_decimals: u8) -> Result<u128, RlpError> {
        if self.price == 0 {
            return Err(RlpError::PriceError);
        }

        let decimal_adjustment = PRECISION
            .checked_sub(token_decimals as u32)
            .ok_or(RlpError::MathOverflow)?;

        let normalized_amount = (amount as u128)
            .checked_mul(10u128.pow(decimal_adjustment))
            .ok_or(RlpError::MathOverflow)?;

        let factor = 10u128
            .checked_pow(self.exponent.unsigned_abs())
            .ok_or(RlpError::MathOverflow)?;

        if self.exponent >= 0 {
            normalized_amount
                .checked_mul(self.price)
                .and_then(|scaled| scaled.checked_mul(factor))
                .ok_or(RlpError::MathOverflow)
        } else {
            // A composed price sits at 10^18, so multiplying first and dividing after would
            // reach 10^36 and overflow a u128 above about 340 dollars of value. Carry the
            // product at 256 bits and divide inside it.
            super::mul_div_floor(normalized_amount, self.price, factor)
                .map_err(|_| RlpError::MathOverflow)
        }
    }
}
