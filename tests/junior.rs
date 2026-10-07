mod support;
use rlp::states::{
    AccessControl, AccessLevel, Action, Asset, LevelRoles, LiquidityPool, Role, Settings,
    UserPermissions,
};
use solana_program_pack::Pack;
use solana_pubkey::Pubkey;
use support::*;
use titan_integration_template::{
    reflect_junior::{
        RLP_PROGRAM_ID, ReflectJuniorVenue, asset_address, interface as rlp, permissions_address,
        settings_address,
    },
    trading_venue::{FromAccount, SwapType, TradingVenue},
};

#[cfg(debug_assertions)]
#[global_allocator]
static ALLOCATOR: assert_no_alloc::AllocDisabler = assert_no_alloc::AllocDisabler;

#[tokio::test]
async fn construction_boundaries_prices_and_no_allocations() {
    let f = Fixture::new();
    let v = f.venue().await;
    assert_eq!(v.directions_num(), vec![(0, 2), (1, 2)]);
    for from in 0..2 {
        let (lo, hi) = v.bounds(from as u8, 2).unwrap();
        assert!(lo > 0 && hi > lo);
        assert_eq!(v.quote(f.request(from, lo - 1)).unwrap().expected_output, 0);
        assert!(
            v.quote(f.request(from, hi + 1))
                .unwrap()
                .not_enough_liquidity
        );
        let mut last = v.quote(f.request(from, 0)).unwrap();
        assert_eq!(last.expected_output, 0);
        assert!(last.price > 0.0 && last.price.is_finite());
        for i in 0..=1000 {
            let amount = lo + ((u128::from(hi - lo) * i / 1000) as u64);
            let request = f.request(from, amount);
            let quote = assert_no_alloc::assert_no_alloc(|| v.quote(request).unwrap());
            assert!(!quote.not_enough_liquidity);
            assert_eq!(quote.amount, amount);
            assert!(quote.expected_output >= last.expected_output);
            assert!(quote.price <= last.price);
            if amount > last.amount {
                let actual = (quote.expected_output - last.expected_output) as f64;
                let marginal = (amount - last.amount) as f64 * quote.price;
                assert!(
                    (actual - marginal).abs() < 2.001,
                    "integer rounding exceeds Titan allowance"
                );
            }
            last = quote;
        }
        let request = f.request(from, hi);
        let start = std::time::Instant::now();
        for _ in 0..100_000 {
            std::hint::black_box(v.quote(std::hint::black_box(request.clone())).unwrap());
        }
        let ns = start.elapsed().as_nanos() / 100_000;
        eprintln!("direction {from}: {ns} ns/quote; bounds [{lo}, {hi}]");
        if !cfg!(debug_assertions) {
            assert!(ns < 1000, "quote exceeds 1 microsecond");
        }
    }
}

#[tokio::test]
async fn invalid_requests_and_refresh_failure_never_leave_live_quotes() {
    let mut f = Fixture::new();
    let mut v = f.venue().await;
    let mut request = f.request(0, 0);
    request.output_mint = request.input_mint;
    assert!(v.quote(request).is_err());
    let mut request = f.request(0, 0);
    request.swap_type = SwapType::ExactOut;
    assert!(v.quote(request).is_err());
    assert!(
        v.generate_swap_instruction(f.request(0, 0), Pubkey::new_unique())
            .is_err()
    );
    assert!(
        v.quote(f.request(0, u64::MAX)).is_err()
            || v.quote(f.request(0, u64::MAX))
                .unwrap()
                .not_enough_liquidity
    );
    f.accounts.remove(&f.oracles[1]);
    assert!(v.update_state(&f).await.is_err());
    assert!(!v.initialized());
    assert!(v.quote(f.request(0, 1)).is_err());
    assert!(v.directions_num().is_empty());
}

#[test]
fn malformed_pool_accounts_return_errors_without_panicking() {
    let f = Fixture::new();
    let original = f.accounts[&f.pool].clone();
    for len in 0..original.data.len() {
        let mut account = original.clone();
        account.data.truncate(len);
        assert!(ReflectJuniorVenue::from_account(&f.pool, &account).is_err());
    }
    let mut account = original.clone();
    account.owner = spl_token::ID;
    assert!(ReflectJuniorVenue::from_account(&f.pool, &account).is_err());
    assert!(ReflectJuniorVenue::from_account(&Pubkey::new_unique(), &original).is_err());
    let mut pool: LiquidityPool = f.decode(f.pool);
    pool.asset_count = 255;
    assert!(ReflectJuniorVenue::from_account(&f.pool, &program_account(&pool)).is_err());
}

#[tokio::test]
async fn validates_staleness_owner_feed_identity_whitelist_and_reserves() {
    for case in 0..9 {
        let mut f = Fixture::new();
        match case {
            0 => {
                f.accounts
                    .insert(f.oracles[0], doppler(f.clock.slot - 201, 2_000_000, 6));
            }
            1 => {
                f.accounts.get_mut(&f.oracles[0]).unwrap().owner = spl_token::ID;
            }
            2 => {
                f.accounts.insert(f.oracles[0], doppler(f.clock.slot, 0, 6));
            }
            3 => {
                let key = asset_address(&f.mints[0]);
                let mut a: Asset = f.decode(key);
                a.legs[0].feed_id = [99; 32];
                f.accounts.insert(key, program_account(&a));
            }
            4 => {
                let key = asset_address(&f.mints[0]);
                let mut a: Asset = f.decode(key);
                a.index = 2;
                f.accounts.insert(key, program_account(&a));
            }
            5 => {
                let key = asset_address(&f.mints[0]);
                let mut a: Asset = f.decode(key);
                a.flags = 0;
                f.accounts.insert(key, program_account(&a));
            }
            6 => {
                f.accounts.get_mut(&f.mints[0]).unwrap().owner = spl_token_2022::ID;
            }
            7 => {
                let key = spl_associated_token_account::get_associated_token_address(
                    &f.pool,
                    &f.mints[0],
                );
                f.accounts
                    .insert(key, token_account(f.mints[1], f.pool, 1_000_000));
            }
            _ => {
                let mut s: Settings = f.decode(settings_address());
                s.access_control.access_map.action_permissions[0].role_count = 255;
                f.accounts.insert(settings_address(), program_account(&s));
            }
        }
        let mut v = f.uninitialized();
        assert!(v.update_state(&f).await.is_err(), "case {case}");
        assert!(!v.initialized());
    }
}

#[tokio::test]
async fn oracle_chain_rotation_and_inverse_legs_refresh_account_dependencies() {
    let mut f = Fixture::new();
    let mut v = f.venue().await;
    let base = v.quote(f.request(0, 10_000)).unwrap();
    let key = asset_address(&f.mints[0]);
    let mut a: Asset = f.decode(key);
    let extra = Pubkey::new_unique();
    a.legs[1] = doppler_leg(extra);
    a.legs[1].inverse = true;
    a.leg_count = 2;
    f.accounts.insert(key, program_account(&a));
    f.accounts
        .insert(extra, doppler(f.clock.slot, 2_000_000, 6));
    v.update_state(&f).await.unwrap();
    let quote = v.quote(f.request(0, 10_000)).unwrap();
    assert_eq!(base.expected_output, 20_000);
    assert_eq!(quote.expected_output, 19_990);
    assert!(
        v.get_required_pubkeys_for_update()
            .unwrap()
            .contains(&extra)
    );
    let ix = v
        .generate_swap_instruction(f.request(0, 10_000), Pubkey::new_unique())
        .unwrap();
    assert_eq!(ix.accounts.len(), 25);
    assert_eq!(ix.accounts[20].pubkey, extra);
    let old = f.oracles[0];
    let new = Pubkey::new_unique();
    a.legs[0] = doppler_leg(new);
    f.accounts.insert(key, program_account(&a));
    f.accounts.insert(new, doppler(f.clock.slot, 3_000_000, 6));
    f.accounts.remove(&old);
    v.update_state(&f).await.unwrap();
    assert!(!v.get_required_pubkeys_for_update().unwrap().contains(&old));
    assert!(v.get_required_pubkeys_for_update().unwrap().contains(&new));
}

#[tokio::test]
async fn deposit_permissions_and_killswitch_are_independent_of_swaps() {
    let mut f = Fixture::new();
    let user = Pubkey::new_unique();
    // Private asset flags and Swap fees/permissions do not gate RLP deposit.
    let key = asset_address(&f.mints[0]);
    let mut a: Asset = f.decode(key);
    a.access_level = AccessLevel::Private;
    f.accounts.insert(key, program_account(&a));
    let mut s: Settings = f.decode(settings_address());
    s.swap_fee_bps = u16::MAX;
    s.access_control.killswitch.freeze(&Action::Swap);
    f.accounts.insert(settings_address(), program_account(&s));
    assert!(f.venue().await.quote(f.request(0, 10_000)).is_ok());

    s.access_control = AccessControl::default();
    s.access_control
        .add_role_to_action(Action::Swap, Role::PUBLIC)
        .unwrap();
    s.access_control
        .add_role_to_action(Action::Deposit, Role::CRANK)
        .unwrap();
    f.accounts.insert(settings_address(), program_account(&s));
    assert!(f.uninitialized().update_state(&f).await.is_err());
    let permissions = UserPermissions {
        bump: Pubkey::find_program_address(&[b"permissions", user.as_ref()], &RLP_PROGRAM_ID).1,
        authority: user,
        protocol_roles: LevelRoles {
            roles: vec![Role::CRANK],
        },
    };
    f.accounts
        .insert(permissions_address(&user), program_account(&permissions));
    let mut v = f.uninitialized().with_authority(user);
    v.update_state(&f).await.unwrap();
    assert!(v.quote(f.request(0, 10_000)).is_ok());
    assert!(
        v.generate_swap_instruction(f.request(0, 10_000), Pubkey::new_unique())
            .is_err()
    );
    let ix = v
        .generate_swap_instruction(f.request(0, 10_000), user)
        .unwrap();
    assert_eq!(ix.accounts[2].pubkey, permissions_address(&user));
    s.access_control.killswitch.freeze(&Action::Deposit);
    f.accounts.insert(settings_address(), program_account(&s));
    assert!(v.update_state(&f).await.is_err());
    assert!(!v.initialized());
}

#[tokio::test]
async fn every_reverse_and_reserve_swap_direction_is_rejected() {
    let f = Fixture::new();
    let v = f.venue().await;
    let mints = v.tradable_mints().unwrap();
    assert_eq!(mints, vec![f.mints[0], f.mints[1], f.lp_mint]);
    for (i, input) in mints.iter().enumerate() {
        for (j, output) in mints.iter().enumerate() {
            if i < 2 && j == 2 {
                continue;
            }
            let mut request = f.request(0, 10_000);
            request.input_mint = *input;
            request.output_mint = *output;
            assert!(v.quote(request.clone()).is_err());
            assert!(v.bounds(i as u8, j as u8).is_err());
            assert!(
                v.generate_swap_instruction(request.clone(), Pubkey::new_unique())
                    .is_err()
            );
            assert!(
                titan_integration_template::swap_route::build_swap_leg(
                    &v,
                    &request,
                    Pubkey::new_unique(),
                    0,
                    1,
                    1_000_000_000
                )
                .is_err()
            );
        }
    }
}

#[tokio::test]
async fn complete_nav_lp_supply_caps_and_wiped_pools_are_enforced() {
    let mut f = Fixture::new();
    let mut partial = ReflectJuniorVenue::from_account(&f.pool, &f.accounts[&f.pool])
        .unwrap()
        .with_asset_mints(&f.mints[..1])
        .unwrap();
    assert!(partial.update_state(&f).await.is_err());
    let amount = 1_000_000;
    let first = f
        .venue()
        .await
        .quote(f.request(0, amount))
        .unwrap()
        .expected_output;
    assert_eq!(first, 2_000_000);
    // Changing the OTHER reserve changes the deposit exchange rate too.
    f.set_reserve(f.mints[1], 2_001_000_000_000_000);
    assert_eq!(
        f.venue()
            .await
            .quote(f.request(0, amount))
            .unwrap()
            .expected_output,
        1_000_249
    );
    f = Fixture::new();
    f.set_lp_supply(1_000_500_000_000);
    assert_eq!(
        f.venue()
            .await
            .quote(f.request(0, amount))
            .unwrap()
            .expected_output,
        first / 2
    );
    f.set_cap(Some(1_000_500_000_000));
    assert!(f.venue().await.bounds(0, 2).is_err());
    f = Fixture::bootstrap();
    assert_eq!(
        f.venue()
            .await
            .quote(f.request(0, amount))
            .unwrap()
            .expected_output,
        2_000_000
    );
    f.set_lp_supply(1001);
    assert!(f.uninitialized().update_state(&f).await.is_err());
    f = Fixture::new();
    f.accounts
        .insert(f.lp_mint, mint_account(Pubkey::new_unique(), 1_000_000, 6));
    assert!(f.uninitialized().update_state(&f).await.is_err());
}

#[tokio::test]
async fn deposit_accounts_nav_tail_minimum_and_route_custody() {
    let f = Fixture::new();
    let v = f.venue().await;
    let user = Pubkey::new_unique();
    let request = f.request(0, 123);
    let ix = v.deposit_instruction(request.clone(), user, 42).unwrap();
    assert_eq!(ix.program_id, RLP_PROGRAM_ID);
    assert_eq!(ix.accounts.len(), 24);
    assert_eq!(ix.accounts[0].pubkey, user);
    assert!(ix.accounts[0].is_signer && ix.accounts[0].is_writable);
    assert_eq!(ix.accounts[2].pubkey, RLP_PROGRAM_ID);
    assert!(!ix.accounts[2].is_signer);
    assert_eq!(
        ix.accounts[8].pubkey,
        spl_associated_token_account::get_associated_token_address(&user, &f.mints[0])
    );
    assert_eq!(ix.accounts[15].pubkey, RLP_PROGRAM_ID);
    assert_eq!(ix.data.len(), 25);
    assert_eq!(ix.data[8], 0);
    assert_eq!(&ix.data[9..17], &123u64.to_le_bytes());
    assert_eq!(&ix.data[17..25], &42u64.to_le_bytes());
    let (spec, accounts) = titan_integration_template::swap_route::build_swap_leg(
        &v,
        &request,
        user,
        0,
        1,
        1_000_000_000,
    )
    .unwrap();
    assert_eq!(spec.n_accounts, 25);
    assert!(!accounts[0].is_signer);
    assert_eq!(accounts.last().unwrap().pubkey, RLP_PROGRAM_ID);
}

#[tokio::test]
async fn direct_deposits_match_sbf_at_bounds_samples_and_bootstrap() {
    use litesvm::LiteSVM;
    use solana_sdk::{signature::Keypair, signer::Signer};
    use solana_transaction::Transaction;
    let path = std::env::var_os("RLP_PROGRAM_SO")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rlp.so")
        });
    assert!(
        path.is_file(),
        "missing RLP simulation binary: {}",
        path.display()
    );
    for f in [
        Fixture::new(),
        Fixture::chained(),
        Fixture::bootstrap(),
        Fixture::single_reserve(),
        Fixture::four_reserves(),
    ] {
        let v = f.venue().await;
        for from in 0..f.mints.len() {
            let (lo, hi) = v.bounds(from as u8, f.mints.len() as u8).unwrap();
            let mut amounts = vec![lo, hi];
            amounts.extend((1..20).map(|i| lo + ((u128::from(hi - lo) * i / 20) as u64)));
            for amount in amounts {
                let mut svm = LiteSVM::new()
                    .with_blockhash_check(false)
                    .with_sigverify(false);
                svm.add_program_from_file(RLP_PROGRAM_ID, &path).unwrap();
                for (key, account) in &f.accounts {
                    if *key != solana_sysvar::clock::ID {
                        svm.set_account(*key, account.clone()).unwrap();
                    }
                }
                svm.set_sysvar(&f.clock);
                let payer = Keypair::new();
                svm.airdrop(&payer.pubkey(), 1_000_000_000).unwrap();
                for (i, mint) in f.mints.iter().enumerate() {
                    svm.set_account(
                        spl_associated_token_account::get_associated_token_address(
                            &payer.pubkey(),
                            mint,
                        ),
                        token_account(*mint, payer.pubkey(), if i == from { amount } else { 0 }),
                    )
                    .unwrap();
                }
                let lp_ata = spl_associated_token_account::get_associated_token_address(
                    &payer.pubkey(),
                    &f.lp_mint,
                );
                svm.set_account(lp_ata, token_account(f.lp_mint, payer.pubkey(), 0))
                    .unwrap();
                let old_supply = spl_token::state::Mint::unpack(&f.accounts[&f.lp_mint].data)
                    .unwrap()
                    .supply;
                let request = f.request(from, amount);
                let expected = v.quote(request.clone()).unwrap().expected_output;
                let ix = v
                    .deposit_instruction(request, payer.pubkey(), expected)
                    .unwrap();
                if amount == hi {
                    let input_ata = spl_associated_token_account::get_associated_token_address(
                        &payer.pubkey(),
                        &f.mints[from],
                    );
                    svm.set_account(
                        input_ata,
                        token_account(f.mints[from], payer.pubkey(), hi + 1),
                    )
                    .unwrap();
                    let mut over_cap = ix.clone();
                    over_cap.data[9..17].copy_from_slice(&(hi + 1).to_le_bytes());
                    over_cap.data[17..25].copy_from_slice(&1u64.to_le_bytes());
                    let failed = svm
                        .send_transaction(Transaction::new_signed_with_payer(
                            &[over_cap],
                            Some(&payer.pubkey()),
                            &[&payer],
                            svm.latest_blockhash(),
                        ))
                        .expect_err("one atom above the input bound must exceed the deposit cap");
                    assert!(
                        failed
                            .meta
                            .logs
                            .iter()
                            .any(|line| line.contains("DepositCapOverflow")),
                        "{failed:?}"
                    );
                    svm.set_account(
                        input_ata,
                        token_account(f.mints[from], payer.pubkey(), amount),
                    )
                    .unwrap();
                }
                let tx = Transaction::new_signed_with_payer(
                    &[ix],
                    Some(&payer.pubkey()),
                    &[&payer],
                    svm.latest_blockhash(),
                );
                svm.send_transaction(tx).unwrap_or_else(|e| {
                    panic!("RLP direct deposit failed from={from} amount={amount}: {e:?}")
                });
                let output = svm
                    .get_account(&spl_associated_token_account::get_associated_token_address(
                        &payer.pubkey(),
                        &f.lp_mint,
                    ))
                    .unwrap();
                assert_eq!(
                    spl_token::state::Account::unpack(&output.data)
                        .unwrap()
                        .amount,
                    expected
                );
                let new_supply =
                    spl_token::state::Mint::unpack(&svm.get_account(&f.lp_mint).unwrap().data)
                        .unwrap()
                        .supply;
                assert_eq!(new_supply - old_supply, expected);
                let reserve = spl_associated_token_account::get_associated_token_address(
                    &f.pool,
                    &f.mints[from],
                );
                let before = spl_token::state::Account::unpack(&f.accounts[&reserve].data)
                    .unwrap()
                    .amount;
                let after =
                    spl_token::state::Account::unpack(&svm.get_account(&reserve).unwrap().data)
                        .unwrap()
                        .amount;
                assert_eq!(after - before, amount);
            }
        }
    }
}

#[tokio::test]
async fn all_supported_oracle_providers_preserve_prices_and_reject_stale_observations() {
    use pyth_solana_receiver_sdk::price_update::{
        PriceFeedMessage, PriceUpdateV2, VerificationLevel,
    };
    use rlp::states::{OracleProvider, StreamPrice};
    for provider in [
        OracleProvider::Chainlink,
        OracleProvider::ChainlinkStreamFeed,
        OracleProvider::ChainlinkStreamRate,
        OracleProvider::Pyth,
    ] {
        let mut f = Fixture::new();
        let expected = f
            .venue()
            .await
            .quote(f.request(0, 10_000))
            .unwrap()
            .expected_output;
        let key = asset_address(&f.mints[0]);
        let mut asset: Asset = f.decode(key);
        let oracle = f.oracles[0];
        asset.legs[0].provider = provider;
        let mut account = match provider {
            OracleProvider::Chainlink => {
                let mut account = f.accounts[&oracle].clone();
                account.owner = rlp::constants::CHAINLINK_ORACLE_PROGRAM_ID;
                account.data = vec![0; 248];
                account.data[..8].copy_from_slice(&[96, 179, 69, 66, 128, 129, 73, 117]);
                account.data[8] = 2;
                account.data[138] = 8;
                account.data[143..147].copy_from_slice(&1u32.to_le_bytes());
                account.data[148..152].copy_from_slice(&1u32.to_le_bytes());
                account.data[200..208].copy_from_slice(&f.clock.slot.to_le_bytes());
                account.data[208..212]
                    .copy_from_slice(&(f.clock.unix_timestamp as u32).to_le_bytes());
                account.data[216..232].copy_from_slice(&200_000_000i128.to_le_bytes());
                account
            }
            OracleProvider::Pyth => {
                let feed_id = [91; 32];
                asset.legs[0].feed_id = feed_id;
                let price = PriceUpdateV2 {
                    write_authority: Pubkey::new_unique(),
                    verification_level: VerificationLevel::Full,
                    price_message: PriceFeedMessage {
                        feed_id,
                        price: 2_000_000,
                        conf: 100,
                        exponent: -6,
                        publish_time: f.clock.unix_timestamp,
                        prev_publish_time: f.clock.unix_timestamp,
                        ema_price: 2_000_000,
                        ema_conf: 100,
                    },
                    posted_slot: f.clock.slot,
                };
                let mut account = program_account(&price);
                account.owner = pyth_solana_receiver_sdk::ID;
                account.data.resize(PriceUpdateV2::LEN, 0);
                account
            }
            _ => {
                let mut feed_id = [0; 32];
                feed_id[1] = if provider == OracleProvider::ChainlinkStreamFeed {
                    3
                } else {
                    7
                };
                asset.legs[0].feed_id = feed_id;
                asset.legs[0].decimals = 8;
                program_account(&StreamPrice {
                    feed_id,
                    price: 200_000_000,
                    exponent: -8,
                    confidence_bps: if provider == OracleProvider::ChainlinkStreamFeed {
                        10
                    } else {
                        0
                    },
                    observations_timestamp: f.clock.unix_timestamp as u32,
                    posted_slot: f.clock.slot,
                    bump: 0,
                })
            }
        };
        f.accounts.insert(key, program_account(&asset));
        f.accounts.insert(oracle, account.clone());
        let mut v = f.venue().await;
        assert_eq!(
            v.quote(f.request(0, 10_000)).unwrap().expected_output,
            expected,
            "provider {provider:?}"
        );
        // A current posting of an old observation must still fail.
        if provider == OracleProvider::Chainlink {
            account.data[208..212]
                .copy_from_slice(&((f.clock.unix_timestamp - 1000) as u32).to_le_bytes());
        } else if provider == OracleProvider::Pyth {
            let mut price: PriceUpdateV2 = f.decode(oracle);
            price.price_message.publish_time -= 1000;
            account = program_account(&price);
            account.owner = pyth_solana_receiver_sdk::ID;
            account.data.resize(PriceUpdateV2::LEN, 0);
        } else {
            let mut price: StreamPrice = f.decode(oracle);
            price.observations_timestamp -= 1000;
            account = program_account(&price);
        }
        f.accounts.insert(oracle, account);
        assert!(
            v.update_state(&f).await.is_err(),
            "stale provider {provider:?}"
        );
    }
}
