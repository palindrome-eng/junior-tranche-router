//! Optional live-state suite; deterministic validation is in `tests/junior.rs`.
mod common;
use common::SuiteConfig;
use titan_integration_template::reflect_junior::{RLP_PROGRAM_ID, ReflectJuniorVenue};
#[cfg(debug_assertions)]
#[global_allocator]
static A: assert_no_alloc::AllocDisabler = assert_no_alloc::AllocDisabler;
fn config() -> Option<SuiteConfig> {
    let Ok(pool) = std::env::var("RLP_POOL") else {
        eprintln!("SKIP live RLP suite: set RLP_POOL, RLP_ASSET_MINTS and SOLANA_RPC_URL");
        return None;
    };
    if std::env::var("RLP_ASSET_MINTS").is_err() {
        eprintln!("SKIP live RLP suite: set RLP_ASSET_MINTS");
        return None;
    }
    Some(SuiteConfig {
        pool: pool.parse().expect("invalid RLP_POOL"),
        programs: vec![RLP_PROGRAM_ID],
    })
}
#[tokio::test]
async fn construction() {
    if let Some(config) = config() {
        common::construction::<ReflectJuniorVenue>(&config).await;
    }
}
#[tokio::test]
async fn zero_input_spot_price() {
    if let Some(config) = config() {
        common::zero_input_spot_price::<ReflectJuniorVenue>(&config).await;
    }
}
#[tokio::test]
async fn bound_simulation() {
    if let Some(config) = config() {
        common::bound_simulation::<ReflectJuniorVenue>(&config).await;
    }
}
#[tokio::test]
async fn random_samples() {
    if let Some(config) = config() {
        common::random_samples::<ReflectJuniorVenue>(&config).await;
    }
}
#[tokio::test]
async fn monotone() {
    if let Some(config) = config() {
        common::monotone::<ReflectJuniorVenue>(&config).await;
    }
}
#[tokio::test]
async fn quoting_speed() {
    if let Some(config) = config() {
        common::quoting_speed::<ReflectJuniorVenue>(&config).await;
    }
}
#[tokio::test]
async fn price_monotone() {
    if let Some(config) = config() {
        common::price_monotone::<ReflectJuniorVenue>(&config).await;
    }
}
#[tokio::test]
async fn mean_value_theorem() {
    if let Some(config) = config() {
        common::mean_value_theorem::<ReflectJuniorVenue>(&config).await;
    }
}
