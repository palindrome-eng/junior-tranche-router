//! Golden bytes captured with the original RLP crate before removing its Git dependency.
use anchor_lang::{AccountDeserialize, AccountSerialize, InstructionData, ToAccountMetas};
use rlp::states::*;
use serde_json::{Value, json};
use solana_pubkey::Pubkey;

fn key(byte: u8) -> Pubkey {
    Pubkey::new_from_array([byte; 32])
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn account<T: AccountSerialize + AccountDeserialize>(value: &T) -> Value {
    let mut bytes = vec![];
    value.try_serialize(&mut bytes).unwrap();
    let decoded = T::try_deserialize(&mut bytes.as_slice()).unwrap();
    let mut roundtrip = vec![];
    decoded.try_serialize(&mut roundtrip).unwrap();
    assert_eq!(bytes, roundtrip);
    json!(hex(&bytes))
}

fn metas(value: &impl ToAccountMetas) -> Value {
    // Anchor's generated client structs ignore the signer override argument.
    assert_eq!(
        value.to_account_metas(None),
        value.to_account_metas(Some(false))
    );
    assert_eq!(
        value.to_account_metas(None),
        value.to_account_metas(Some(true))
    );
    json!(
        value
            .to_account_metas(None)
            .iter()
            .map(|m| { json!([m.pubkey.to_string(), m.is_signer, m.is_writable]) })
            .collect::<Vec<_>>()
    )
}

fn abi_samples() -> Value {
    let roles = [
        Role::UNSET,
        Role::PUBLIC,
        Role::TESTEE,
        Role::FREEZE,
        Role::CRANK,
        Role::MANAGER,
        Role::SUPREMO,
    ];
    let actions = [
        Action::Deposit,
        Action::Withdraw,
        Action::Slash,
        Action::Swap,
        Action::FreezeDeposit,
        Action::FreezeWithdraw,
        Action::FreezeSlash,
        Action::FreezeSwap,
        Action::InitializeLiquidityPool,
        Action::AddAsset,
        Action::UpdateDepositCap,
        Action::DepositRewards,
        Action::Management,
        Action::SuspendDeposits,
        Action::UpdateRole,
        Action::UpdateAction,
        Action::UpdateOracle,
        Action::RemovePoolAsset,
    ];
    let mut access = AccessControl::default();
    for (i, action) in actions.into_iter().enumerate() {
        let mapping = &mut access.access_map.action_permissions[i];
        mapping.action = action;
        mapping.allowed_roles[..roles.len()].copy_from_slice(&roles);
        mapping.role_count = roles.len() as u8;
    }
    access.access_map.mapping_count = MAX_ACTION_MAPPINGS as u8;
    access.killswitch.frozen = 0x01020304;
    let settings = Settings {
        bump: 253,
        liquidity_pools: 7,
        assets: 42,
        access_control: access,
        swap_fee_bps: 123,
        supremo_count: 2,
    };
    let providers = [
        OracleProvider::Unset,
        OracleProvider::Doppler,
        OracleProvider::Pyth,
        OracleProvider::Chainlink,
        OracleProvider::ChainlinkStreamFeed,
        OracleProvider::ChainlinkStreamRate,
    ];
    let assets: Vec<_> = providers
        .into_iter()
        .enumerate()
        .map(|(i, provider)| {
            account(&Asset {
                bump: 252,
                index: i as u8,
                mint: key(1),
                legs: std::array::from_fn(|j| OracleLeg {
                    oracle: key(10 + j as u8),
                    feed_id: [20 + j as u8; 32],
                    max_staleness: 200 + j as u32,
                    inverse: j % 2 == 1,
                    provider,
                    max_confidence_bps: 50 + j as u16,
                    decimals: 8,
                }),
                leg_count: 3,
                max_composed_confidence_bps: 400,
                flags: FLAG_CHAIN_RESOLVED,
                access_level: if i % 2 == 0 {
                    AccessLevel::Public
                } else {
                    AccessLevel::Private
                },
            })
        })
        .collect();
    let pools: Vec<_> = [false, true]
        .into_iter()
        .map(|optional| {
            account(&LiquidityPool {
                bump: 251,
                index: 9,
                lp_token: key(2),
                cooldowns: 0x0102030405060708,
                cooldown_duration: 86_400,
                deposit_cap: optional.then_some(9_000_000_000),
                asset_count: 4,
                assets: [1, 4, 7, 9],
                protected_vault: optional.then_some(key(3)),
            })
        })
        .collect();
    let deposit_metas: Vec<_> = [None, Some(key(3))]
        .into_iter()
        .map(|permissions| {
            metas(&rlp::accounts::Deposit {
                signer: key(1),
                settings: key(2),
                permissions,
                liquidity_pool: key(4),
                lp_token: key(5),
                user_lp_account: key(6),
                asset: key(7),
                asset_mint: key(8),
                user_asset_account: key(9),
                pool_asset_account: key(10),
                oracle: key(11),
                token_program: key(12),
                associated_token_program: key(13),
                system_program: key(14),
                event_authority: key(15),
                program: rlp::ID,
            })
        })
        .collect();
    let deposits: Vec<_> = [
        (0, 1, 1),
        (9, 0x0102030405060708, u64::MAX),
        (255, u64::MAX, 123),
    ]
    .into_iter()
    .map(|(liquidity_pool_index, amount, min_lp_tokens)| {
        hex(&rlp::instruction::Deposit {
            args: rlp::instructions::DepositArgs {
                liquidity_pool_index,
                amount,
                min_lp_tokens,
            },
        }
        .data())
    })
    .collect();
    let initializations: Vec<_> = [false, true]
        .into_iter()
        .map(|optional| {
            hex(&rlp::instruction::InitializeLp {
                args: rlp::instructions::InitializeLiquidityPoolArgs {
                    cooldown_duration: 86_400,
                    deposit_cap: optional.then_some(9_000_000_000),
                    assets: vec![1, 4, 7, 9],
                    protected_vault: optional.then_some(key(3)),
                },
            }
            .data())
        })
        .collect();
    json!({
        "program_id": rlp::ID.to_string(),
        "settings": account(&settings), "assets": assets, "pools": pools,
        "permissions": account(&UserPermissions { bump: 250, authority: key(4),
            protocol_roles: LevelRoles { roles: roles.to_vec() } }),
        "stream_price": account(&StreamPrice { feed_id: [5; 32], price: u128::MAX - 42,
            exponent: -18, confidence_bps: 321, observations_timestamp: 1_800_000_000,
            posted_slot: 0x0102030405060708, bump: 249 }),
        "deposit_metas": deposit_metas, "deposit_data": deposits, "initialize_lp_data": initializations,
        "reserve_data": hex(&rlp::instruction::InitializePoolReserve { _liquidity_pool_id: 9 }.data()),
        "reserve_metas": metas(&rlp::accounts::InitializePoolReserve {
            signer: key(1), permissions: key(2), settings: key(3), liquidity_pool: key(4),
            asset: key(5), asset_mint: key(6), pool_asset_account: key(7), system_program: key(8),
            token_program: key(9), associated_token_program: key(10) }),
    })
}

#[test]
fn account_layouts_and_instructions_match_original_rlp() {
    let original: Value = serde_json::from_str(include_str!("fixtures/rlp-abi.json")).unwrap();
    assert_eq!(abi_samples(), original);
}
