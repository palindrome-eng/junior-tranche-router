//! Oracle limits and identities from the recorded RLP revision.
use anchor_lang::prelude::*;

pub const ORACLE_MAXIMUM_AGE: u64 = 2 * 60;

pub const MAX_ORACLE_CONFIDENCE_RATIO: u64 = 50;

pub const PRECISION: u32 = 18;

pub const DOPPLER_ORACLE_PROGRAM_ID: Pubkey = Pubkey::new_from_array([
    0x05, 0xbe, 0xb9, 0xd8, 0x8c, 0xb5, 0xc1, 0xa2, 0x1e, 0x48, 0xe9, 0x94, 0x3b, 0x25, 0x84, 0xd6,
    0xe9, 0x30, 0x52, 0x66, 0x2a, 0x83, 0x99, 0x72, 0x3f, 0xcd, 0xac, 0x29, 0x36, 0xe1, 0x3b, 0x93,
]);

pub const DOPPLER_MAX_STALENESS: u64 = 200;

pub const CHAINLINK_ORACLE_PROGRAM_ID: Pubkey = Pubkey::new_from_array([
    0xf1, 0x4b, 0xf6, 0x5a, 0xd5, 0x6b, 0xd2, 0xba, 0x71, 0x5e, 0x45, 0x74, 0x2c, 0x23, 0x1f, 0x27,
    0xd6, 0x36, 0x21, 0xcf, 0x5b, 0x77, 0x8f, 0x37, 0xc1, 0xa2, 0x48, 0x95, 0x1d, 0x17, 0x56, 0x02,
]);

pub const COMPOSITION_EXPONENT: i32 = 18;

pub const COMPOSITION_SCALE: u128 = 1_000_000_000_000_000_000;

pub const MAX_ORACLE_LEGS: usize = 3;

pub const BPS_DENOMINATOR: u64 = 10_000;

pub const MAX_CONFIDENCE_BPS_CEILING: u16 = 1_000;

pub const DEFAULT_MAX_CONFIDENCE_BPS: u16 = 200;

pub const PYTH_MAX_STALENESS_SLOTS: u32 = 300;

pub const DEFAULT_ORACLE_STALENESS: u32 = 200;

pub const MAX_ORACLE_STALENESS_CEILING: u32 = 432_000;

pub const MAX_REPLAYABLE_ORACLE_STALENESS_CEILING: u32 = 1_500;

pub const SLOT_MILLIS: u64 = 400;

pub const STREAM_DECIMALS_LOW: u8 = 8;

pub const STREAM_DECIMALS_HIGH: u8 = 18;

pub const MAX_STREAM_RATE_DECIMALS: u8 = 18;
