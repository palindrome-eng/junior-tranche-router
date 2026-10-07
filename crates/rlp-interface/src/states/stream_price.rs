use anchor_lang::prelude::*;

/// One Chainlink Data Streams report, verified and stored by `post_oracle_price`.
///
/// A Data Streams report is never an account of its own. The caller passes 288 signed bytes in
/// with the transaction. The verifier checks the DON signatures over them and returns the body
/// through `set_return_data`. This account holds that body as a price the chain resolver reads
/// like any other oracle account.
///
/// The PDA is `[STREAM_PRICE_SEED, asset_mint, feed_id]`, so each asset that names a feed has its
/// own price slot. Two assets on one feed then never share a stored book, and a report accepted
/// through a looser asset cannot leave a tighter asset with a spread it refuses to read. Every
/// bound a leg carries still applies again at read time.
#[account]
#[derive(InitSpace)]
pub struct StreamPrice {
    /// The feed the DON signed this price for. It binds the account to an asset, because a
    /// leg may name any address.
    pub feed_id: [u8; 32],
    /// Price as the DON signed it, unrescaled. A v3 posting stores a benchmark price and a v7
    /// posting a redemption rate.
    pub price: u128,
    /// `-decimals` for the scale the posting leg declared.
    pub exponent: i32,
    /// Half-width of the smallest symmetric interval covering the signed book, in basis points
    /// of the price, rounded up. A v7 posting stores zero here, because that schema carries no
    /// book and its reader returns no confidence at all.
    pub confidence_bps: u16,
    /// The `observationsTimestamp` of the report. It states when the price described the
    /// market.
    pub observations_timestamp: u32,
    /// Slot this account was last written at.
    pub posted_slot: u64,
    pub bump: u8,
}

impl StreamPrice {
    /// Bytes the account occupies, discriminator included.
    pub const LEN: usize = 8 + Self::INIT_SPACE;
}
