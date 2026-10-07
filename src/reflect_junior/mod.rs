//! One-way Reflect junior LP minting: a reserve asset enters, LP tokens leave.
//!
//! Pool accounts contain asset *indices*, not mint addresses. Supply a mint
//! registry for every reserve with `with_asset_mints` before refreshing.
//! Redemption and reserve-to-reserve swaps are never exposed.
mod discovery;
pub mod math;
mod oracle;

use crate::{
    account_caching::AccountsCache,
    trading_venue::{
        AddressLookupTableTrait, FromAccount, QuoteRequest, QuoteResult, SwapType, TradingVenue,
        error::TradingVenueError, protocol::PoolProtocol, token_info::TokenInfo,
    },
};
use anchor_lang::{AccountDeserialize, InstructionData, ToAccountMetas};
use async_trait::async_trait;
pub use discovery::parse_pool_creations;
use math::{dead_shares, math_error, minted_lp_tokens};
pub use rlp as interface;
use rlp::{
    helpers::OraclePrice,
    states::{Action, Asset, LiquidityPool, MAX_POOL_ASSETS, Settings, UserPermissions},
};
use solana_account::Account;
use solana_instruction::{AccountMeta, Instruction};
use solana_program_option::COption;
use solana_program_pack::Pack;
use solana_pubkey::Pubkey;
use solana_sysvar::clock::{self, Clock};
use spl_associated_token_account::get_associated_token_address;
use spl_token::state::{Account as TokenAccount, AccountState, Mint};

pub const RLP_PROGRAM_ID: Pubkey = rlp::ID;
pub const RLP_REVISION: &str = rlp::SOURCE_REVISION;

pub fn settings_address() -> Pubkey {
    Pubkey::find_program_address(&[b"settings"], &RLP_PROGRAM_ID).0
}
pub fn asset_address(mint: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[b"asset", mint.as_ref()], &RLP_PROGRAM_ID).0
}
pub fn permissions_address(user: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[b"permissions", user.as_ref()], &RLP_PROGRAM_ID).0
}
pub fn event_authority() -> Pubkey {
    Pubkey::find_program_address(&[b"__event_authority"], &RLP_PROGRAM_ID).0
}

fn invalid(message: &'static str) -> TradingVenueError {
    TradingVenueError::DeserializationFailed(message.into())
}
fn decode<T: AccountDeserialize>(account: &Account) -> Result<T, TradingVenueError> {
    if account.owner != RLP_PROGRAM_ID || account.executable {
        return Err(invalid("invalid RLP account owner"));
    }
    T::try_deserialize(&mut account.data.as_slice())
        .map_err(|_| invalid("invalid RLP account discriminator or layout"))
}
fn pool_state(key: &Pubkey, account: &Account) -> Result<LiquidityPool, TradingVenueError> {
    let pool: LiquidityPool = decode(account)?;
    let (expected, bump) =
        Pubkey::find_program_address(&[b"liquidity_pool", &[pool.index]], &RLP_PROGRAM_ID);
    let n = pool.asset_count as usize;
    if *key != expected
        || pool.bump != bump
        || n == 0
        || n > MAX_POOL_ASSETS
        || (0..n).any(|i| pool.assets[..i].contains(&pool.assets[i]))
    {
        return Err(invalid("invalid RLP pool PDA or asset indices"));
    }
    Ok(pool)
}

#[derive(Clone)]
struct AssetState {
    address: Pubkey,
    state: Asset,
    reserve: Pubkey,
    balance: u64,
    price: OraclePrice,
    decimals: u8,
    marginal_value: f64,
}

#[derive(Clone)]
pub struct ReflectJuniorVenue {
    pub pool_id: Pubkey,
    pool: LiquidityPool,
    mints: Vec<Pubkey>,
    assets: Vec<AssetState>,
    token_info: Vec<TokenInfo>,
    required: Vec<Pubkey>,
    authority: Option<Pubkey>,
    permissions: Option<Pubkey>,
    public_deposit: bool,
    nav: u128,
    lp_supply: u64,
    lp_decimals: u8,
    initialized: bool,
}

impl FromAccount for ReflectJuniorVenue {
    fn from_account(pubkey: &Pubkey, account: &Account) -> Result<Self, TradingVenueError> {
        let pool = pool_state(pubkey, account)?;
        let lp_mint = pool.lp_token;
        Ok(Self {
            pool_id: *pubkey,
            pool,
            mints: vec![],
            assets: vec![],
            token_info: vec![],
            required: vec![*pubkey, settings_address(), lp_mint, clock::ID],
            authority: None,
            permissions: None,
            public_deposit: false,
            nav: 0,
            lp_supply: 0,
            lp_decimals: 0,
            initialized: false,
        })
    }
}

impl ReflectJuniorVenue {
    /// Configure every pool reserve mint, in stable token-index order.
    /// The LP mint is appended as the final token; it is output-only.
    /// Resolve pool asset indices through RLP Asset accounts in your indexer.
    pub fn with_asset_mints(mut self, mints: &[Pubkey]) -> Result<Self, TradingVenueError> {
        if mints.is_empty()
            || mints.len() > MAX_POOL_ASSETS
            || mints.iter().enumerate().any(|(i, m)| {
                *m == Pubkey::default() || *m == self.pool.lp_token || mints[..i].contains(m)
            })
        {
            return Err(invalid("supply one to four distinct RLP reserve mints"));
        }
        self.mints = mints.to_vec();
        self.initialized = false;
        self.required = vec![
            self.pool_id,
            settings_address(),
            self.pool.lp_token,
            clock::ID,
        ];
        for mint in mints {
            self.required.extend([
                *mint,
                asset_address(mint),
                get_associated_token_address(&self.pool_id, mint),
            ]);
        }
        if let Some(authority) = self.authority {
            self.required.push(permissions_address(&authority));
        }
        Ok(self)
    }

    /// Quote deposits for a specific RLP signer. For routed swaps this must be TitanPDA,
    /// since RLP checks the CPI signer, not the end user's permissions.
    /// Omit this when Action::Deposit is public.
    pub fn with_authority(mut self, authority: Pubkey) -> Self {
        self.authority = Some(authority);
        self.required.push(permissions_address(&authority));
        self.initialized = false;
        self
    }

    fn input(&self, request: &QuoteRequest) -> Result<&AssetState, TradingVenueError> {
        if request.swap_type != SwapType::ExactIn {
            return Err(TradingVenueError::ExactOutNotSupported);
        }
        if !self.initialized {
            return Err(TradingVenueError::NotInitialized(self.pool_id.into()));
        }
        if request.output_mint != self.pool.lp_token || request.input_mint == self.pool.lp_token {
            return Err(TradingVenueError::UnsupportedVenue(
                "RLP supports reserve-to-LP minting only; redemption is not atomic".into(),
            ));
        }
        self.assets
            .iter()
            .find(|a| a.state.mint == request.input_mint)
            .ok_or(TradingVenueError::InvalidMint(request.input_mint.into()))
    }

    /// Build an atomic deposit with a positive minimum LP output.
    pub fn deposit_instruction(
        &self,
        request: QuoteRequest,
        user: Pubkey,
        min_lp_tokens: u64,
    ) -> Result<Instruction, TradingVenueError> {
        let from = self.input(&request)?;
        let quote = self.quote(request.clone())?;
        if request.amount == 0
            || min_lp_tokens == 0
            || quote.not_enough_liquidity
            || quote.expected_output == 0
        {
            return Err(math_error());
        }
        if !self.public_deposit && (self.authority != Some(user) || self.permissions.is_none()) {
            return Err(TradingVenueError::UnsupportedVenue(
                "RLP deposit requires permissions for the actual CPI signer".into(),
            ));
        }
        let permissions = if self.authority == Some(user) {
            self.permissions
        } else {
            None
        };
        let mut accounts = rlp::accounts::Deposit {
            signer: user,
            settings: settings_address(),
            permissions,
            liquidity_pool: self.pool_id,
            lp_token: self.pool.lp_token,
            user_lp_account: get_associated_token_address(&user, &self.pool.lp_token),
            asset: from.address,
            asset_mint: from.state.mint,
            user_asset_account: get_associated_token_address(&user, &from.state.mint),
            pool_asset_account: from.reserve,
            oracle: *from.state.primary_oracle(),
            token_program: spl_token::ID,
            associated_token_program: spl_associated_token_account::ID,
            system_program: solana_sdk_ids::system_program::ID,
            event_authority: event_authority(),
            program: RLP_PROGRAM_ID,
        }
        .to_account_metas(None);
        // NAV needs EVERY reserve, including zero balances. Each tuple is
        // reserve, Asset, primary oracle, mint, followed by that asset's extra legs.
        for asset in &self.assets {
            accounts.extend([
                AccountMeta::new_readonly(asset.reserve, false),
                AccountMeta::new_readonly(asset.address, false),
                AccountMeta::new_readonly(*asset.state.primary_oracle(), false),
                AccountMeta::new_readonly(asset.state.mint, false),
            ]);
            for leg in &asset.state.legs[1..asset.state.leg_count as usize] {
                accounts.push(AccountMeta::new_readonly(leg.oracle, false));
            }
        }
        Ok(Instruction {
            program_id: RLP_PROGRAM_ID,
            accounts,
            data: rlp::instruction::Deposit {
                args: rlp::instructions::DepositArgs {
                    liquidity_pool_index: self.pool.index,
                    amount: request.amount,
                    min_lp_tokens,
                },
            }
            .data(),
        })
    }

    async fn refresh(&mut self, cache: &dyn AccountsCache) -> Result<(), TradingVenueError> {
        if self.mints.is_empty() {
            return Err(TradingVenueError::MissingState(
                "configure reserve mints with with_asset_mints before refreshing".into(),
            ));
        }
        let mut required = vec![self.pool_id, settings_address(), clock::ID];
        let core = fetch(cache, &required).await?;
        let pool = pool_state(&self.pool_id, &core[0])?;
        let settings: Settings = decode(&core[1])?;
        // The bundled access-map helpers use unchecked slices on the stored counts.
        if settings.access_control.access_map.mapping_count as usize
            > rlp::states::MAX_ACTION_MAPPINGS
            || settings
                .access_control
                .access_map
                .action_permissions
                .iter()
                .any(|m| m.role_count as usize > rlp::states::MAX_ROLES)
        {
            return Err(invalid("malformed RLP access map"));
        }
        if settings
            .access_control
            .killswitch
            .is_frozen(&Action::Deposit)
        {
            return Err(TradingVenueError::InactivePoolError(
                self.pool_id,
                self.protocol(),
            ));
        }
        if core[2].owner != solana_sdk_ids::sysvar::ID {
            return Err(invalid("invalid Clock owner"));
        }
        let clock: Clock =
            bincode::deserialize(&core[2].data).map_err(|_| invalid("invalid Clock sysvar"))?;
        let public_deposit = settings.access_control.is_public_action(Action::Deposit);
        let mut permissions = None;
        let mut authorized = false;
        if let Some(authority) = self.authority {
            let key = permissions_address(&authority);
            required.push(key);
            if let Some(account) = cache.get_account(&key).await? {
                let creds: UserPermissions = decode(&account)?;
                if creds.authority != authority
                    || creds.bump
                        != Pubkey::find_program_address(
                            &[b"permissions", authority.as_ref()],
                            &RLP_PROGRAM_ID,
                        )
                        .1
                {
                    return Err(invalid("RLP permissions authority mismatch"));
                }
                authorized =
                    creds.can_perform_protocol_action(Action::Deposit, &settings.access_control);
                permissions = Some(key);
            }
        }
        if !public_deposit && !authorized {
            return Err(TradingVenueError::InactivePoolError(
                self.pool_id,
                self.protocol(),
            ));
        }
        if self.mints.len() != pool.asset_count as usize || self.mints.contains(&pool.lp_token) {
            return Err(invalid(
                "deposit pricing requires the complete reserve mint registry",
            ));
        }
        required.push(pool.lp_token);
        let lp_account = fetch(cache, &[pool.lp_token]).await?.remove(0);
        if lp_account.owner != spl_token::ID || lp_account.executable {
            return Err(invalid("invalid RLP receipt mint owner"));
        }
        let lp = Mint::unpack(&lp_account.data).map_err(|_| invalid("invalid RLP receipt mint"))?;
        if !(3..=18).contains(&lp.decimals)
            || lp.mint_authority != COption::Some(self.pool_id)
            || lp.freeze_authority != COption::None
        {
            return Err(invalid("invalid RLP receipt mint authority or decimals"));
        }
        let mut nav = 0u128;
        let mut assets = Vec::with_capacity(self.mints.len());
        let mut token_info = Vec::with_capacity(self.mints.len() + 1);
        for mint in &self.mints {
            let key = asset_address(mint);
            let reserve = get_associated_token_address(&self.pool_id, mint);
            let keys = [key, *mint, reserve];
            required.extend(keys);
            let accounts = fetch(cache, &keys).await?;
            let asset: Asset = decode(&accounts[0])?;
            if asset.mint != *mint
                || asset.bump
                    != Pubkey::find_program_address(&[b"asset", mint.as_ref()], &RLP_PROGRAM_ID).1
                || asset.index >= settings.assets
                || !pool.has_asset(asset.index)
                || assets
                    .iter()
                    .any(|a: &AssetState| a.state.index == asset.index)
            {
                return Err(invalid("mint is not a distinct whitelisted RLP pool asset"));
            }
            if accounts[1].owner != spl_token::ID || accounts[2].owner != spl_token::ID {
                return Err(invalid("RLP supports classic SPL Token only"));
            }
            let mint_state =
                Mint::unpack(&accounts[1].data).map_err(|_| invalid("invalid reserve mint"))?;
            let vault = TokenAccount::unpack(&accounts[2].data)
                .map_err(|_| invalid("invalid reserve token account"))?;
            if mint_state.decimals > 18
                || vault.owner != self.pool_id
                || vault.mint != *mint
                || vault.state != AccountState::Initialized
            {
                return Err(invalid(
                    "invalid RLP reserve authority, mint, decimals, or state",
                ));
            }
            let count = asset
                .live_leg_count()
                .map_err(|_| invalid("invalid RLP oracle chain"))?;
            let oracle_keys: Vec<_> = asset.legs[..count].iter().map(|l| l.oracle).collect();
            required.extend(&oracle_keys);
            // Keep newly discovered keys visible even if a price fetch fails.
            self.required = required.clone();
            let mut oracle_accounts = fetch(cache, &oracle_keys).await?;
            let price = oracle::resolve(&asset, &oracle_keys, &mut oracle_accounts, &clock)?;
            nav = nav
                .checked_add(
                    price
                        .mul(vault.amount, mint_state.decimals)
                        .map_err(|_| math_error())?,
                )
                .ok_or_else(math_error)?;
            let marginal_value = price.price as f64
                * 10f64.powi(18 - i32::from(mint_state.decimals) + price.exponent);
            if !marginal_value.is_finite() || marginal_value <= 0.0 {
                return Err(math_error());
            }
            token_info.push(TokenInfo::new(mint, &accounts[1], clock.epoch)?);
            assets.push(AssetState {
                address: key,
                reserve,
                balance: vault.amount,
                state: asset,
                price,
                decimals: mint_state.decimals,
                marginal_value,
            });
        }
        if nav == 0 && lp.supply > dead_shares(lp.decimals).ok_or_else(math_error)? {
            return Err(TradingVenueError::InactivePoolError(
                self.pool_id,
                self.protocol(),
            ));
        }
        token_info.push(TokenInfo::new(&pool.lp_token, &lp_account, clock.epoch)?);
        required.sort_unstable();
        required.dedup();
        self.pool = pool;
        self.assets = assets;
        self.token_info = token_info;
        self.required = required;
        self.permissions = permissions;
        self.public_deposit = public_deposit;
        self.nav = nav;
        self.lp_supply = lp.supply;
        self.lp_decimals = lp.decimals;
        self.initialized = true;
        Ok(())
    }
}

async fn fetch(
    cache: &dyn AccountsCache,
    keys: &[Pubkey],
) -> Result<Vec<Account>, TradingVenueError> {
    let accounts = cache.get_accounts(keys).await?;
    if accounts.len() != keys.len() {
        return Err(TradingVenueError::FailedToFetchMultipleAccountData);
    }
    accounts
        .into_iter()
        .zip(keys)
        .map(|(a, k)| a.ok_or(TradingVenueError::NoAccountFound((*k).into())))
        .collect()
}

#[async_trait]
impl TradingVenue for ReflectJuniorVenue {
    fn initialized(&self) -> bool {
        self.initialized
    }
    fn program_id(&self) -> Pubkey {
        RLP_PROGRAM_ID
    }
    fn program_dependencies(&self) -> Vec<Pubkey> {
        vec![
            RLP_PROGRAM_ID,
            spl_token::ID,
            spl_associated_token_account::ID,
        ]
    }
    fn market_id(&self) -> Pubkey {
        self.pool_id
    }
    fn protocol(&self) -> PoolProtocol {
        PoolProtocol::ReflectJunior
    }
    fn get_token_info(&self) -> &[TokenInfo] {
        &self.token_info
    }
    fn get_required_pubkeys_for_update(&self) -> Result<Vec<Pubkey>, TradingVenueError> {
        Ok(self.required.clone())
    }
    fn directions_num(&self) -> Vec<(u8, u8)> {
        if !self.initialized {
            return vec![];
        }
        let lp_index = self.assets.len() as u8;
        (0..lp_index).map(|input| (input, lp_index)).collect()
    }
    async fn update_state(&mut self, cache: &dyn AccountsCache) -> Result<(), TradingVenueError> {
        self.initialized = false;
        self.refresh(cache).await
    }
    fn quote(&self, request: QuoteRequest) -> Result<QuoteResult, TradingVenueError> {
        let from = self.input(&request)?;
        let price = if self.lp_supply == 0 || self.nav == 0 {
            from.marginal_value / 10f64.powi(18 - i32::from(self.lp_decimals))
        } else {
            from.marginal_value * self.lp_supply as f64 / self.nav as f64
        };
        let output = from
            .price
            .mul(request.amount, from.decimals)
            .ok()
            .and_then(|value| minted_lp_tokens(value, self.nav, self.lp_supply, self.lp_decimals));
        let unavailable = from.balance.checked_add(request.amount).is_none()
            || output.is_none()
            || output
                .and_then(|n| self.lp_supply.checked_add(n))
                .is_none_or(|supply| self.pool.deposit_cap.is_some_and(|cap| supply > cap));
        Ok(QuoteResult {
            input_mint: request.input_mint,
            output_mint: request.output_mint,
            amount: if unavailable { 0 } else { request.amount },
            expected_output: if unavailable { 0 } else { output.unwrap() },
            not_enough_liquidity: unavailable,
            price,
        })
    }
    fn generate_swap_instruction(
        &self,
        request: QuoteRequest,
        user: Pubkey,
    ) -> Result<Instruction, TradingVenueError> {
        self.deposit_instruction(request, user, 1)
    }
    fn bounds(&self, input: u8, output: u8) -> Result<(u64, u64), TradingVenueError> {
        let mut request = QuoteRequest {
            input_mint: self.get_token(input as usize)?.pubkey,
            output_mint: self.get_token(output as usize)?.pubkey,
            amount: 0,
            swap_type: SwapType::ExactIn,
        };
        self.input(&request)?;
        // Exact integer search avoids skipping a very narrow valid interval.
        let mut lo = 0u64;
        let mut hi = u64::MAX;
        while lo < hi {
            let mid = lo + (hi - lo) / 2 + (hi - lo) % 2;
            request.amount = mid;
            if self
                .quote(request.clone())
                .is_ok_and(|q| !q.not_enough_liquidity)
            {
                lo = mid;
            } else {
                hi = mid - 1;
            }
        }
        let upper = lo;
        request.amount = upper;
        if upper == 0 || self.quote(request.clone())?.expected_output == 0 {
            return Err(TradingVenueError::NoQuotableValue(self.pool_id.into()));
        }
        lo = 1;
        hi = upper;
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            request.amount = mid;
            if self.quote(request.clone())?.expected_output > 0 {
                hi = mid;
            } else {
                lo = mid + 1;
            }
        }
        Ok((lo, upper))
    }
}

#[async_trait]
impl AddressLookupTableTrait for ReflectJuniorVenue {
    async fn get_lookup_table_keys(
        &self,
        _: Option<&dyn AccountsCache>,
    ) -> Result<Vec<Pubkey>, TradingVenueError> {
        let mut keys = self.required.clone();
        keys.extend([
            RLP_PROGRAM_ID,
            event_authority(),
            spl_token::ID,
            spl_associated_token_account::ID,
        ]);
        keys.sort_unstable();
        keys.dedup();
        Ok(keys)
    }
}
