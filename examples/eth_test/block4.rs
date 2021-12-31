use anyhow::Result;
use async_trait::async_trait;
use ethers::{
    signers::Signer,
    types::{Address, U256},
};

use runrun::{core::Ctx, run_ctx, run_test};

use crate::{block1::Ctx1, init::Client, utils::ERC20MinterPauser};

#[derive(Clone)]
pub struct Ctx4;

#[run_ctx]
#[async_trait]
impl Ctx for Ctx4 {
    type Base = Ctx1;
    async fn build(base: Self::Base) -> Self {
        Self
    }
}

#[run_test]
async fn test_foo(ctx: Ctx4) -> Result<()> {
    println!("test_foo");
    Ok(())
}
