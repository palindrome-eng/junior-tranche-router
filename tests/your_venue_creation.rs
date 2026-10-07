use anchor_lang::{InstructionData, ToAccountMetas};
use solana_pubkey::Pubkey;
use titan_integration_template::{
    reflect_junior::{
        RLP_PROGRAM_ID, asset_address, interface as rlp, parse_pool_creations, settings_address,
    },
    trading_venue::venue_creation::ParsedInstruction,
};

fn reserve(mint: Pubkey) -> ParsedInstruction {
    let pool = Pubkey::find_program_address(&[b"liquidity_pool", &[0]], &RLP_PROGRAM_ID).0;
    let accounts = rlp::accounts::InitializePoolReserve {
        signer: Pubkey::new_unique(),
        permissions: Pubkey::new_unique(),
        settings: settings_address(),
        liquidity_pool: pool,
        asset: asset_address(&mint),
        asset_mint: mint,
        pool_asset_account: spl_associated_token_account::get_associated_token_address(
            &pool, &mint,
        ),
        system_program: solana_sdk_ids::system_program::ID,
        token_program: spl_token::ID,
        associated_token_program: spl_associated_token_account::ID,
    }
    .to_account_metas(None)
    .into_iter()
    .map(|a| a.pubkey)
    .collect();
    ParsedInstruction {
        program_id: RLP_PROGRAM_ID,
        accounts,
        data: rlp::instruction::InitializePoolReserve {
            _liquidity_pool_id: 0,
        }
        .data(),
    }
}

#[test]
fn parses_pool_reserves_and_merges_mints_without_duplicates() {
    let a = Pubkey::new_unique();
    let b = Pubkey::new_unique();
    let found = parse_pool_creations(&[reserve(a), reserve(b), reserve(a)]);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].mints, vec![a, b]);
}

#[test]
fn initialize_lp_discovers_output_receipt_mint() {
    let mut ix = reserve(Pubkey::new_unique());
    ix.accounts.resize(11, Pubkey::new_unique());
    let lp_mint = Pubkey::new_unique();
    ix.accounts[4] = lp_mint;
    ix.data = rlp::instruction::InitializeLp {
        args: rlp::instructions::InitializeLiquidityPoolArgs {
            cooldown_duration: 86400,
            deposit_cap: None,
            assets: vec![0, 1],
            protected_vault: None,
        },
    }
    .data();
    let found = parse_pool_creations(&[ix.clone()]);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].mints, vec![lp_mint]);
    let a = Pubkey::new_unique();
    let b = Pubkey::new_unique();
    for instructions in [
        vec![ix.clone(), reserve(a), reserve(b)],
        vec![reserve(a), reserve(b), ix],
    ] {
        let found = parse_pool_creations(&instructions);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].mints, vec![a, b, lp_mint]);
    }
}

#[test]
fn rejects_unrelated_truncated_and_inconsistent_instructions() {
    let original = reserve(Pubkey::new_unique());
    for len in 0..original.data.len() {
        let mut ix = original.clone();
        ix.data.truncate(len);
        assert!(parse_pool_creations(&[ix]).is_empty());
    }
    for index in [2, 3, 4, 6] {
        let mut ix = original.clone();
        ix.accounts[index] = Pubkey::new_unique();
        assert!(parse_pool_creations(&[ix]).is_empty());
    }
    let mut ix = original;
    ix.program_id = Pubkey::new_unique();
    assert!(parse_pool_creations(&[ix]).is_empty());
}
