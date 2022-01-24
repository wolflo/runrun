use async_trait::async_trait;
use fuel_gql_client::client::FuelClient;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use fuels_rs::contract::Contract;
use fuel_tx::Salt;
use anyhow::Result;
use std::sync::Arc;

use fuel_core::service::{Config, FuelService};

use crate::utils::TestContract;

use runrun::{run_ctx, run_test, core::Ctx, types::*, TList};

#[derive(Clone)]
pub struct Ctx0 {
    pub client: FuelClient,
    pub contract: Arc<TestContract>,
}

#[run_ctx]
#[async_trait]
impl Ctx for Ctx0 {
    type Base = ();
    async fn build(_base: Self::Base) -> Self {
        let server = FuelService::new_node(Config::local_node()).await.unwrap();
        let client = FuelClient::from(server.bound_address);

        // Build the contract
        let rng = &mut StdRng::seed_from_u64(2322u64);
        let salt: [u8; 32] = rng.gen();
        let salt = Salt::from(salt);
        let compiled = Contract::compile_sway_contract("./", salt).unwrap();

        // Deploy the contract
        let contract_id = Contract::deploy(&compiled, &client).await.unwrap();
        let contract = Arc::new(TestContract::new(compiled, client.clone()));

        Self {
            client,
            contract,
        }
    }
}

#[run_test]
async fn test_initialize_counter(ctx: Ctx0) -> Result<()> {
    println!("Running test_init");
    let result = ctx.contract.initialize_counter(42).call().await.unwrap();
    assert_eq!(42, result);
    Ok(())
}

