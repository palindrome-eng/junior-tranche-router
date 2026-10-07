//! Optional live RLP route simulation. Local fixtures live in junior_route.rs.
mod common;
use titan_integration_template::reflect_junior::{ReflectJuniorVenue, RLP_PROGRAM_ID};
#[tokio::test]
async fn reserve_to_lp_mint_routes() {
    let Ok(pool) = std::env::var("RLP_POOL") else {
        eprintln!("SKIP live RLP route: set RLP_POOL, RLP_ASSET_MINTS and SOLANA_RPC_URL");
        return;
    };
    if std::env::var("RLP_ASSET_MINTS").is_err() {
        eprintln!("SKIP live RLP route: set RLP_ASSET_MINTS");
        return;
    }
    common::run_swap_route::<ReflectJuniorVenue>(common::RouteConfig {
        pool: pool.parse().expect("invalid RLP_POOL"),
        venue_programs: vec![RLP_PROGRAM_ID],
    })
    .await;
}
