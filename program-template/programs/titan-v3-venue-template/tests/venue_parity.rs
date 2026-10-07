use anchor_lang::{AnchorSerialize, InstructionData};
use titan_integration_template::reflect_junior::interface as rlp;
use titan_integration_template::swap_route::Venue as RouteVenue;
use titan_v3_venue_template::{instructions::venues::reflect_junior, state::Venue};

#[test]
fn venue_enum_matches_route_builder() {
    let cases = [
        (Venue::RaydiumAmm, RouteVenue::RaydiumAmm),
        (
            Venue::ReflectJuniorDeposit { min_lp_tokens: 1 },
            RouteVenue::ReflectJuniorDeposit { min_lp_tokens: 1 },
        ),
        (
            Venue::ReflectJuniorDeposit {
                min_lp_tokens: u64::MAX,
            },
            RouteVenue::ReflectJuniorDeposit {
                min_lp_tokens: u64::MAX,
            },
        ),
    ];
    for (program, builder) in cases {
        assert_eq!(program.try_to_vec().unwrap(), builder.to_borsh_bytes());
    }
}

#[test]
fn mint_cpi_data_matches_bundled_anchor_instruction() {
    use anchor_lang::prelude::*;
    let mut accounts = (0..24)
        .map(|_| AccountMeta::new_readonly(Pubkey::new_unique(), false))
        .collect::<Vec<_>>();
    accounts[15].pubkey = reflect_junior::PROGRAM_ID;
    for amount in [1, 1_000_000, u64::MAX] {
        let ix = reflect_junior::deposit(amount, 9, 123, &accounts)
            .unwrap()
            .remove(0);
        let expected = rlp::instruction::Deposit {
            args: rlp::instructions::DepositArgs {
                liquidity_pool_index: 9,
                amount,
                min_lp_tokens: 123,
            },
        }
        .data();
        assert_eq!(ix.data, expected);
        assert_eq!(ix.accounts, accounts);
        assert_eq!(ix.program_id, rlp::ID);
    }
    assert!(reflect_junior::deposit(0, 9, 1, &accounts).is_err());
    assert!(reflect_junior::deposit(1, 9, 0, &accounts).is_err());
    assert!(reflect_junior::deposit(1, 9, 1, &accounts[..19]).is_err());
}
