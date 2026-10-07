#![allow(dead_code)]
use anchor_lang::AccountSerialize;
use async_trait::async_trait;
// This fixture is shared by the Rust 2024 quoter and Rust 2021 router tests.
// Keep their formatters from alternately reordering these grouped imports.
#[rustfmt::skip]
use rlp::states::{
    AccessControl, AccessLevel, Action, Asset, FLAG_CHAIN_RESOLVED, LiquidityPool, OracleLeg,
    OracleProvider, Role, Settings,
};
use solana_account::Account;
use solana_program_option::COption;
use solana_program_pack::Pack;
use solana_pubkey::Pubkey;
use solana_sysvar::clock::{self, Clock};
use spl_token::state::{Account as TokenAccount, AccountState, Mint};
use std::collections::BTreeMap;
#[rustfmt::skip]
use titan_integration_template::{
    account_caching::{AccountCacheError, AccountsCache},
    reflect_junior::{
        RLP_PROGRAM_ID, ReflectJuniorVenue, asset_address, settings_address, interface as rlp,
    },
    trading_venue::{FromAccount, QuoteRequest, SwapType, TradingVenue},
};

#[derive(Clone)]
pub struct Fixture {
    pub pool: Pubkey,
    pub mints: Vec<Pubkey>,
    pub lp_mint: Pubkey,
    pub oracles: [Pubkey; 2],
    pub accounts: BTreeMap<Pubkey, Account>,
    pub clock: Clock,
}

pub fn program_account<T: AccountSerialize>(value: &T) -> Account {
    let mut data = vec![];
    value.try_serialize(&mut data).unwrap();
    Account {
        lamports: 10_000_000,
        data,
        owner: RLP_PROGRAM_ID,
        executable: false,
        rent_epoch: 0,
    }
}

pub fn token_account(mint: Pubkey, owner: Pubkey, amount: u64) -> Account {
    let state = TokenAccount {
        mint,
        owner,
        amount,
        state: AccountState::Initialized,
        ..TokenAccount::default()
    };
    let mut data = vec![0; TokenAccount::LEN];
    TokenAccount::pack(state, &mut data).unwrap();
    Account {
        lamports: 10_000_000,
        data,
        owner: spl_token::ID,
        executable: false,
        rent_epoch: 0,
    }
}

pub fn doppler(slot: u64, price: u64, precision: u8) -> Account {
    let mut data = slot.to_le_bytes().to_vec();
    data.extend(price.to_le_bytes());
    data.push(precision);
    Account {
        lamports: 10_000_000,
        data,
        owner: rlp::constants::DOPPLER_ORACLE_PROGRAM_ID,
        executable: false,
        rent_epoch: 0,
    }
}

pub fn empty_leg() -> OracleLeg {
    OracleLeg {
        oracle: Pubkey::default(),
        feed_id: [0; 32],
        max_staleness: 0,
        inverse: false,
        provider: OracleProvider::Unset,
        max_confidence_bps: 0,
        decimals: 0,
    }
}

pub fn doppler_leg(oracle: Pubkey) -> OracleLeg {
    OracleLeg {
        oracle,
        feed_id: oracle.to_bytes(),
        provider: OracleProvider::Doppler,
        ..empty_leg()
    }
}

impl Fixture {
    pub fn new() -> Self {
        let (pool, bump) =
            Pubkey::find_program_address(&[b"liquidity_pool", &[0]], &RLP_PROGRAM_ID);
        let mints = vec![
            Pubkey::new_from_array([11; 32]),
            Pubkey::new_from_array([12; 32]),
        ];
        let oracles = [
            Pubkey::new_from_array([21; 32]),
            Pubkey::new_from_array([22; 32]),
        ];
        let lp_mint = Pubkey::new_from_array([13; 32]);
        let clock = Clock {
            slot: 10_000,
            unix_timestamp: 1_800_000_000,
            ..Clock::default()
        };
        let mut accounts = BTreeMap::new();
        accounts.insert(
            pool,
            program_account(&LiquidityPool {
                bump,
                index: 0,
                lp_token: lp_mint,
                cooldowns: 0,
                cooldown_duration: 86_400,
                deposit_cap: Some(2_501_000_000_000),
                asset_count: 2,
                assets: [0, 1, 255, 255],
                protected_vault: None,
            }),
        );
        let mut access_control = AccessControl::default();
        access_control
            .add_role_to_action(Action::Deposit, Role::PUBLIC)
            .unwrap();
        accounts.insert(
            settings_address(),
            program_account(&Settings {
                bump: Pubkey::find_program_address(&[b"settings"], &RLP_PROGRAM_ID).1,
                liquidity_pools: 1,
                assets: 2,
                access_control,
                swap_fee_bps: 30,
                supremo_count: 1,
            }),
        );
        accounts.insert(
            clock::ID,
            Account {
                lamports: 1,
                owner: solana_sdk_ids::sysvar::ID,
                data: bincode::serialize(&clock).unwrap(),
                executable: false,
                rent_epoch: 0,
            },
        );
        for i in 0..2 {
            let key = asset_address(&mints[i]);
            let asset = Asset {
                bump: Pubkey::find_program_address(&[b"asset", mints[i].as_ref()], &RLP_PROGRAM_ID)
                    .1,
                index: i as u8,
                mint: mints[i],
                legs: [doppler_leg(oracles[i]), empty_leg(), empty_leg()],
                leg_count: 1,
                max_composed_confidence_bps: 0,
                flags: FLAG_CHAIN_RESOLVED,
                access_level: AccessLevel::Public,
            };
            accounts.insert(key, program_account(&asset));
            let mint = Mint {
                mint_authority: COption::None,
                supply: 10_000_000_000_000_000,
                decimals: if i == 0 { 6 } else { 9 },
                is_initialized: true,
                freeze_authority: COption::None,
            };
            let mut data = vec![0; Mint::LEN];
            Mint::pack(mint, &mut data).unwrap();
            accounts.insert(
                mints[i],
                Account {
                    lamports: 10_000_000,
                    data,
                    owner: spl_token::ID,
                    executable: false,
                    rent_epoch: 0,
                },
            );
            accounts.insert(
                spl_associated_token_account::get_associated_token_address(&pool, &mints[i]),
                token_account(mints[i], pool, 1_000_000_000_000),
            );
            accounts.insert(
                oracles[i],
                doppler(clock.slot, if i == 0 { 2_000_000 } else { 1_000_000 }, 6),
            );
        }
        accounts.insert(lp_mint, mint_account(pool, 2_001_000_000_000, 6));
        Self {
            pool,
            mints,
            lp_mint,
            oracles,
            accounts,
            clock,
        }
    }

    pub fn chained() -> Self {
        let mut fixture = Self::new();
        for (i, (price, inverse)) in [(2_000_000, true), (3_000_000, false), (4_000_000, true)]
            .into_iter()
            .enumerate()
        {
            let asset_index = usize::from(i > 0);
            let key = asset_address(&fixture.mints[asset_index]);
            let mut asset: Asset = fixture.decode(key);
            let oracle = Pubkey::new_from_array([31 + i as u8; 32]);
            let leg_index = asset.leg_count as usize;
            asset.legs[leg_index] = doppler_leg(oracle);
            asset.legs[leg_index].inverse = inverse;
            asset.leg_count += 1;
            fixture.accounts.insert(key, program_account(&asset));
            fixture
                .accounts
                .insert(oracle, doppler(fixture.clock.slot, price, 6));
        }
        fixture
    }

    pub fn single_reserve() -> Self {
        let mut f = Self::new();
        f.mints.truncate(1);
        let mut pool: LiquidityPool = f.decode(f.pool);
        pool.asset_count = 1;
        pool.assets = [0, 255, 255, 255];
        f.accounts.insert(f.pool, program_account(&pool));
        f
    }

    pub fn four_reserves() -> Self {
        let mut f = Self::new();
        for i in 2..4 {
            let mint = Pubkey::new_from_array([12 + i as u8; 32]);
            let oracle = Pubkey::new_from_array([42 + i as u8; 32]);
            let asset = Asset {
                bump: Pubkey::find_program_address(&[b"asset", mint.as_ref()], &RLP_PROGRAM_ID).1,
                index: i as u8,
                mint,
                legs: [doppler_leg(oracle), empty_leg(), empty_leg()],
                leg_count: 1,
                max_composed_confidence_bps: 0,
                flags: FLAG_CHAIN_RESOLVED,
                access_level: AccessLevel::Public,
            };
            f.accounts
                .insert(asset_address(&mint), program_account(&asset));
            f.accounts.insert(
                mint,
                mint_account(Pubkey::new_unique(), 10_000_000_000_000_000, 6),
            );
            f.accounts
                .insert(oracle, doppler(f.clock.slot, 1_000_000, 6));
            f.set_reserve(mint, 7_000_000_000);
            f.mints.push(mint);
        }
        // Exercise the maximum 40-account deposit instruction (three legs per asset).
        for i in 0..4 {
            let key = asset_address(&f.mints[i]);
            let mut asset: Asset = f.decode(key);
            for j in 1..3 {
                let oracle = Pubkey::new_from_array([60 + (i * 2 + j) as u8; 32]);
                asset.legs[j] = doppler_leg(oracle);
                f.accounts
                    .insert(oracle, doppler(f.clock.slot, 1_000_000, 6));
            }
            asset.leg_count = 3;
            f.accounts.insert(key, program_account(&asset));
        }
        let mut pool: LiquidityPool = f.decode(f.pool);
        pool.asset_count = 4;
        pool.assets = [0, 1, 2, 3];
        f.accounts.insert(f.pool, program_account(&pool));
        let mut settings: Settings = f.decode(settings_address());
        settings.assets = 4;
        f.accounts
            .insert(settings_address(), program_account(&settings));
        f
    }

    pub fn restricted(authority: Pubkey) -> Self {
        use rlp::states::{LevelRoles, UserPermissions};
        let mut f = Self::new();
        let mut settings: Settings = f.decode(settings_address());
        settings.access_control = AccessControl::default();
        settings
            .access_control
            .add_role_to_action(Action::Deposit, Role::MANAGER)
            .unwrap();
        f.accounts
            .insert(settings_address(), program_account(&settings));
        let (key, bump) =
            Pubkey::find_program_address(&[b"permissions", authority.as_ref()], &RLP_PROGRAM_ID);
        f.accounts.insert(
            key,
            program_account(&UserPermissions {
                bump,
                authority,
                protocol_roles: LevelRoles {
                    roles: vec![Role::MANAGER],
                },
            }),
        );
        f
    }

    pub fn bootstrap() -> Self {
        let mut f = Self::new();
        f.set_lp_supply(1000);
        for mint in f.mints.clone() {
            f.set_reserve(mint, 0);
        }
        f
    }

    pub fn set_reserve(&mut self, mint: Pubkey, balance: u64) {
        let key = spl_associated_token_account::get_associated_token_address(&self.pool, &mint);
        self.accounts
            .insert(key, token_account(mint, self.pool, balance));
    }
    pub fn set_lp_supply(&mut self, supply: u64) {
        self.accounts
            .insert(self.lp_mint, mint_account(self.pool, supply, 6));
    }
    pub fn set_cap(&mut self, cap: Option<u64>) {
        let mut pool: LiquidityPool = self.decode(self.pool);
        pool.deposit_cap = cap;
        self.accounts.insert(self.pool, program_account(&pool));
    }

    pub fn uninitialized(&self) -> ReflectJuniorVenue {
        ReflectJuniorVenue::from_account(&self.pool, &self.accounts[&self.pool])
            .unwrap()
            .with_asset_mints(&self.mints)
            .unwrap()
    }
    pub async fn venue(&self) -> ReflectJuniorVenue {
        let mut venue = self.uninitialized();
        venue.update_state(self).await.unwrap();
        venue
    }
    pub fn request(&self, from: usize, amount: u64) -> QuoteRequest {
        QuoteRequest {
            input_mint: self.mints[from],
            output_mint: self.lp_mint,
            amount,
            swap_type: SwapType::ExactIn,
        }
    }
    pub fn decode<T: anchor_lang::AccountDeserialize>(&self, key: Pubkey) -> T {
        T::try_deserialize(&mut self.accounts[&key].data.as_slice()).unwrap()
    }
}

#[async_trait]
impl AccountsCache for Fixture {
    async fn get_account(&self, key: &Pubkey) -> Result<Option<Account>, AccountCacheError> {
        Ok(self.accounts.get(key).cloned())
    }
    async fn get_accounts(
        &self,
        keys: &[Pubkey],
    ) -> Result<Vec<Option<Account>>, AccountCacheError> {
        Ok(keys.iter().map(|k| self.accounts.get(k).cloned()).collect())
    }
}

pub fn mint_account(authority: Pubkey, supply: u64, decimals: u8) -> Account {
    let mint = Mint {
        mint_authority: COption::Some(authority),
        supply,
        decimals,
        is_initialized: true,
        freeze_authority: COption::None,
    };
    let mut data = vec![0; Mint::LEN];
    Mint::pack(mint, &mut data).unwrap();
    Account {
        lamports: 10_000_000,
        data,
        owner: spl_token::ID,
        executable: false,
        rent_epoch: 0,
    }
}
