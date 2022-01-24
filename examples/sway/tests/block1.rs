use runrun::{run_ctx, run_test, core::Ctx, types::*, TList};
use fuel_gql_client::client::FuelClient;
use anyhow::Result;
use std::sync::Arc;
use async_trait::async_trait;

use crate::{utils::TestContract, init::Ctx0};


#[derive(Clone)]
pub struct Ctx1 {
    pub client: FuelClient,
    pub contract: Arc<TestContract>,
}

#[run_ctx]
#[async_trait]
impl Ctx for Ctx1 {
    type Base = Ctx0;
    async fn build(base: Self::Base) -> Self {
        Self {
            client: base.client,
            contract: base.contract,
        }
    }
}

// this currently relies on the Ctx0 test_initialize_counter running
// before, since we aren't resetting the state between tests
#[run_test]
async fn test_increment_counter(ctx: Ctx1) -> Result<()> {
    println!("Running test_init");
    let result = ctx.contract.increment_counter(10).call().await.unwrap();
    assert_eq!(52, result);
    Ok(())
}
