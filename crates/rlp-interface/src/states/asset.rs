use crate::constants::*;
use crate::errors::RlpError;
use anchor_lang::prelude::*;

/// Which program publishes a leg.
///
/// `Unset` is the zero value. The reader is then selected from the account owner. Any other
/// value pins the leg to one provider, and an account owned by a different oracle program
/// gives an error.
/// The discriminants are a wire contract, not an implementation detail. `post_oracle_price`
/// takes one as an instruction argument, and the proxy program sends the same numbers from its
/// own copy of this enum. Borsh derives its tag from declaration order while `as u8` reads the
/// discriminant, so a variant inserted anywhere but the end moves one and not the other.
/// `the_oracle_provider_discriminants_are_pinned` holds both to these values.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Debug, InitSpace)]
pub enum OracleProvider {
    Unset = 0,
    Doppler = 1,
    Pyth = 2,
    Chainlink = 3,
    /// Chainlink Data Streams. `post_oracle_price` verifies a signed report and stores it in
    /// a `StreamPrice` account this program owns. A different product from `Chainlink` above,
    /// which is Data Feeds.
    ChainlinkStreamFeed = 4,
    /// Chainlink Data Streams, the redemption rate schema. It stores the same `StreamPrice`
    /// record at the same seed family as `ChainlinkStreamFeed`. The tag is therefore the only
    /// thing that tells a reader which schema wrote the account.
    ChainlinkStreamRate = 5,
}

impl OracleProvider {
    /// True when this provider publishes a confidence interval with each update.
    ///
    /// Pyth publishes one directly. A Data Streams v3 report publishes a bid and an ask.
    /// `stream_report` turns that book into the same basis-point measure. A Data Streams v7
    /// report publishes a rate and no book at all. A Data Feeds round holds a slot, a timestamp
    /// and an answer. A Doppler account holds a slot, a price and a precision. `Unset` selects
    /// the reader at read time, so it promises nothing.
    pub fn publishes_confidence(&self) -> bool {
        matches!(self, Self::Pyth | Self::ChainlinkStreamFeed)
    }

    /// Highest staleness bound this provider accepts, in slots.
    ///
    /// Each value is the bound the matching reader can actually enforce. A ceiling above what
    /// the reader applies would let a stored configuration claim a freshness window the code
    /// then ignores, so the write and the read agree here by construction.
    ///
    /// Chainlink gets the wide ceiling because exchange-rate feeds update rarely, and its
    /// reader applies the stored bound directly. Doppler stops at its own 200-slot limit. Pyth
    /// stops at 300 slots, which is the 120-second global bound converted at 400 ms a slot.
    ///
    /// `ChainlinkStreamFeed` gets the replayable ceiling. The wide ceiling belongs to Data
    /// Feeds. A Data Feeds round exists only because the OCR store accepted it and stamped the
    /// slot on chain. Nobody can present an old Data Feeds round as new. A Data Streams report
    /// is a signed message with no home on chain. The window of the DON is one day wide.
    /// Anyone holding a report can present it at any moment inside that window. This program
    /// then stamps a current slot on it. The poster chooses the price inside a replay window,
    /// and this ceiling bounds that window.
    pub fn staleness_ceiling(&self) -> u32 {
        match self {
            Self::Chainlink => MAX_ORACLE_STALENESS_CEILING,
            Self::Doppler => DOPPLER_MAX_STALENESS as u32,
            Self::Pyth => PYTH_MAX_STALENESS_SLOTS,
            Self::ChainlinkStreamFeed | Self::ChainlinkStreamRate => {
                MAX_REPLAYABLE_ORACLE_STALENESS_CEILING
            }
            // `Unset` picks its reader at read time, so it promises nothing and takes the
            // tightest bound of the others.
            Self::Unset => DOPPLER_MAX_STALENESS as u32,
        }
    }

    /// Owner program this tag expects. `Unset` has none, because the owner decides.
    pub fn expected_owner(&self) -> Option<Pubkey> {
        match self {
            Self::Unset => None,
            Self::Doppler => Some(DOPPLER_ORACLE_PROGRAM_ID),
            Self::Pyth => Some(pyth_solana_receiver_sdk::ID),
            Self::Chainlink => Some(CHAINLINK_ORACLE_PROGRAM_ID),
            // This program writes and owns a `StreamPrice`. Nothing else may own an account
            // that a Data Streams leg prices from, under either schema.
            Self::ChainlinkStreamFeed | Self::ChainlinkStreamRate => Some(crate::ID),
        }
    }
}

/// One hop of a price chain.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Debug, InitSpace)]
pub struct OracleLeg {
    /// Account this leg reads.
    pub oracle: Pubkey,
    /// Expected identity inside that account. Pyth carries a `feed_id`. Doppler and Chainlink
    /// accounts carry no embedded identity, so this equals `oracle`.
    pub feed_id: [u8; 32],
    /// Staleness bound for this leg, in slots. Zero selects the provider ceiling.
    pub max_staleness: u32,
    /// True when this leg divides instead of multiplying.
    pub inverse: bool,
    pub provider: OracleProvider,
    /// Confidence bound for this leg, in basis points. Zero selects the default.
    pub max_confidence_bps: u16,
    /// Scale the source publishes its price at. A Data Streams report carries no scale in its
    /// signed bytes. Chainlink publishes a v3 stream at 8 or 18 decimals, and a v7 rate at any
    /// scale up to 18. Zero on every other provider. Pyth carries an exponent, Doppler a
    /// precision byte, and Data Feeds a `decimals` header field.
    pub decimals: u8,
}

impl OracleLeg {
    pub fn is_empty(&self) -> bool {
        self.oracle == Pubkey::default()
            && self.feed_id == [0u8; 32]
            && self.max_staleness == 0
            && !self.inverse
            && self.provider == OracleProvider::Unset
            && self.max_confidence_bps == 0
            && self.decimals == 0
    }
}

/// Bit 0 of `Asset::flags`. The program sets it once the chain has resolved against a posted
/// price and passed the band the authority supplied.
///
/// An authority can configure a leg before any price for its feed exists, because the posting
/// path refuses to write an account no asset names. That first `update_oracle` therefore has
/// nothing to resolve and the band cannot run. Nothing but the leg itself then checks the
/// declared scale of a Data Streams leg. This bit keeps the asset off every money path until
/// an `update_oracle` has priced the chain for real.
pub const FLAG_CHAIN_RESOLVED: u8 = 1 << 0;

#[derive(AnchorSerialize, AnchorDeserialize, InitSpace, Clone, Copy, PartialEq, Debug)]
pub enum AccessLevel {
    Public,
    Private,
}

#[account]
#[derive(InitSpace)]
pub struct Asset {
    pub bump: u8,
    pub index: u8,
    pub mint: Pubkey,
    /// Chain of one to three legs, read in order. Slots at and after `leg_count` are empty.
    pub legs: [OracleLeg; MAX_ORACLE_LEGS],
    pub leg_count: u8,
    /// Confidence bound for the composed chain, in basis points. Zero selects the default.
    pub max_composed_confidence_bps: u16,
    /// Bit flags. Only `FLAG_CHAIN_RESOLVED` is defined, and the other seven must stay zero.
    pub flags: u8,
    pub access_level: AccessLevel,
}

impl Asset {
    /// Address of the account leg 0 reads. Instruction constraints bind the primary oracle
    /// account against this value.
    pub fn primary_oracle(&self) -> &Pubkey {
        &self.legs[0].oracle
    }

    /// Number of live legs, after checking that every slot at and after the count is empty.
    ///
    /// This function also refuses an undefined flag bit. Every path that prices the chain
    /// calls this function. A byte that carries a meaning this program does not implement
    /// therefore never reaches such a path.
    pub fn live_leg_count(&self) -> Result<usize> {
        require!(
            self.flags & !FLAG_CHAIN_RESOLVED == 0,
            RlpError::InvalidOracleConfiguration
        );

        let count = self.leg_count as usize;
        require!(
            count >= 1 && count <= MAX_ORACLE_LEGS,
            RlpError::InvalidOracleConfiguration
        );
        for leg in self.legs.iter().skip(count) {
            require!(leg.is_empty(), RlpError::InvalidOracleConfiguration);
        }
        Ok(count)
    }

    pub fn is_public(&self) -> bool {
        self.access_level == AccessLevel::Public
    }
}
