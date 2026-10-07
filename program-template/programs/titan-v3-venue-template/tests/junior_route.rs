//! Deterministic execution through TitanPDA custody and the real RLP binary.
#[path = "../../../../tests/support/mod.rs"]
mod support;
use anchor_lang::{AccountSerialize, ToAccountMetas};
use litesvm::LiteSVM;
use solana_account::Account;
use solana_instruction::{AccountMeta, Instruction};
use solana_program_pack::Pack;
use solana_pubkey::Pubkey;
use solana_sdk::{signature::Keypair, signer::Signer};
use solana_transaction::Transaction;
use support::*;
use titan_integration_template::{
    reflect_junior::RLP_PROGRAM_ID,
    swap_route::{build_swap_leg, encode_swap_route_v3_data, Venue, ROUTE_WEIGHT_ALL},
    trading_venue::TradingVenue,
};
use titan_v3_venue_template::{self as router, state::TitanPda};

#[tokio::test]
async fn mint_routes_preserve_custody_reject_reverse_and_enforce_minimum() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let rlp_path = std::env::var("RLP_PROGRAM_SO")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| root.join("tests/fixtures/rlp.so"));
    let route_path = root.join("program-template/target/deploy/titan_v3_venue_template.so");
    assert!(
        rlp_path.is_file(),
        "missing RLP simulation binary: {}",
        rlp_path.display()
    );
    if !route_path.is_file() {
        eprintln!("SKIP local routed execution: run make build-program");
        return;
    }
    let (titan, bump) = Pubkey::find_program_address(&[TitanPda::SEED], &router::ID);
    for f in [
        Fixture::new(),
        Fixture::chained(),
        Fixture::bootstrap(),
        Fixture::restricted(titan),
    ] {
        let mut v = f.uninitialized().with_authority(titan);
        v.update_state(&f).await.unwrap();
        for from in 0..2 {
            let (lo, hi) = v.bounds(from as u8, 2).unwrap();
            for amount in [lo, (hi + lo) / 2, hi] {
                let mut svm = LiteSVM::new()
                    .with_blockhash_check(false)
                    .with_sigverify(false)
                    .with_transaction_history(0);
                svm.add_program_from_file(RLP_PROGRAM_ID, &rlp_path)
                    .unwrap();
                svm.add_program_from_file(router::ID, &route_path).unwrap();
                for (key, account) in &f.accounts {
                    if *key != solana_sysvar::clock::ID {
                        svm.set_account(*key, account.clone()).unwrap();
                    }
                }
                svm.set_sysvar(&f.clock);
                let payer = Keypair::new();
                svm.airdrop(&payer.pubkey(), 1_000_000_000).unwrap();
                let mut data = vec![];
                TitanPda { bump }.try_serialize(&mut data).unwrap();
                svm.set_account(
                    titan,
                    Account {
                        lamports: 10_000_000,
                        data,
                        owner: router::ID,
                        executable: false,
                        rent_epoch: 0,
                    },
                )
                .unwrap();
                let ordered = [f.mints[from], f.lp_mint];
                let user_atas = ordered.map(|m| {
                    spl_associated_token_account::get_associated_token_address(&payer.pubkey(), &m)
                });
                let titan_atas = ordered.map(|m| {
                    spl_associated_token_account::get_associated_token_address(&titan, &m)
                });
                for i in 0..2 {
                    svm.set_account(
                        user_atas[i],
                        token_account(ordered[i], payer.pubkey(), if i == 0 { amount } else { 0 }),
                    )
                    .unwrap();
                    svm.set_account(
                        titan_atas[i],
                        token_account(ordered[i], titan, if i == 0 { 17 } else { 29 }),
                    )
                    .unwrap();
                }
                let request = f.request(from, amount);
                let expected = v.quote(request.clone()).unwrap().expected_output;
                let (mut leg, leg_accounts) =
                    build_swap_leg(&v, &request, titan, 0, 1, ROUTE_WEIGHT_ALL).unwrap();
                let mut accounts = router::accounts::SwapRouteV3 {
                    payer: payer.pubkey(),
                    user: payer.pubkey(),
                    titan_pda: titan,
                    input_token_account: user_atas[0],
                    output_token_account: user_atas[1],
                    token_program: spl_token::ID,
                    token_2022_program: spl_token_2022::ID,
                    system_program: solana_sdk_ids::system_program::ID,
                    associated_token_program: spl_associated_token_account::ID,
                    token_ledger: None,
                    reserved_optional_account_0: None,
                    reserved_optional_account_1: None,
                }
                .to_account_metas(None);
                let fixed_accounts = accounts.len();
                accounts.extend(titan_atas.map(|k| AccountMeta::new(k, false)));
                accounts.extend(ordered.map(|k| AccountMeta::new_readonly(k, false)));
                accounts.extend(leg_accounts);
                // Keep valid route indices (0 -> 1), but reverse the mint/ATA
                // ordering. The deposit guard must reject both a funded reverse
                // route and a zero-input leg that would otherwise skip its CPI.
                let mut reverse_accounts = accounts.clone();
                reverse_accounts.swap(3, 4); // user input/output accounts
                reverse_accounts.swap(fixed_accounts, fixed_accounts + 1);
                reverse_accounts.swap(fixed_accounts + 2, fixed_accounts + 3);
                svm.set_account(user_atas[1], token_account(f.lp_mint, payer.pubkey(), 1))
                    .unwrap();
                for reverse_amount in [0, 1] {
                    let reverse_tx = Transaction::new_signed_with_payer(
                        &[Instruction {
                            program_id: router::ID,
                            accounts: reverse_accounts.clone(),
                            data: encode_swap_route_v3_data(reverse_amount, 2, &[leg]),
                        }],
                        Some(&payer.pubkey()),
                        &[&payer],
                        svm.latest_blockhash(),
                    );
                    let failed = svm
                        .send_transaction(reverse_tx)
                        .expect_err("reverse mint route must be rejected");
                    assert!(
                        failed
                            .meta
                            .logs
                            .iter()
                            .any(|line| line.contains("InvalidSwapInput")),
                        "{failed:?}"
                    );
                }
                svm.set_account(user_atas[1], token_account(f.lp_mint, payer.pubkey(), 0))
                    .unwrap();
                leg.venue = Venue::ReflectJuniorDeposit {
                    min_lp_tokens: expected + 1,
                };
                let failed = Instruction {
                    program_id: router::ID,
                    accounts: accounts.clone(),
                    data: encode_swap_route_v3_data(amount, 2, &[leg]),
                };
                let tx = Transaction::new_signed_with_payer(
                    &[failed],
                    Some(&payer.pubkey()),
                    &[&payer],
                    svm.latest_blockhash(),
                );
                assert!(
                    svm.send_transaction(tx).is_err(),
                    "minimum output must be enforced by the CPI"
                );
                leg.venue = Venue::ReflectJuniorDeposit {
                    min_lp_tokens: expected,
                };
                let ix = Instruction {
                    program_id: router::ID,
                    accounts,
                    data: encode_swap_route_v3_data(amount, 2, &[leg]),
                };
                let tx = Transaction::new_signed_with_payer(
                    &[ix],
                    Some(&payer.pubkey()),
                    &[&payer],
                    svm.latest_blockhash(),
                );
                svm.send_transaction(tx)
                    .unwrap_or_else(|e| panic!("route failed from={from}, amount={amount}: {e:?}"));
                let balance = |key| {
                    spl_token::state::Account::unpack(&svm.get_account(key).unwrap().data)
                        .unwrap()
                        .amount
                };
                assert_eq!(balance(&user_atas[0]), 0);
                assert_eq!(balance(&user_atas[1]), expected);
                assert_eq!(balance(&titan_atas[0]), 17);
                assert_eq!(balance(&titan_atas[1]), 29);
                let initial_supply = spl_token::state::Mint::unpack(&f.accounts[&f.lp_mint].data)
                    .unwrap()
                    .supply;
                let supply =
                    spl_token::state::Mint::unpack(&svm.get_account(&f.lp_mint).unwrap().data)
                        .unwrap()
                        .supply;
                assert_eq!(supply, initial_supply + expected);
            }
        }
    }
}
