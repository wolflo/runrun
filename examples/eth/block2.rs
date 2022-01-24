use anyhow::Result;
use async_trait::async_trait;

use runrun::{core::Ctx, run_ctx, run_test};

use crate::{
    init::{Client, Ctx0},
    utils::ERC20MinterPauser,
};

#[derive(Clone)]
pub struct Ctx2 {
    pub token: ERC20MinterPauser<Client>,
}

#[run_ctx]
#[async_trait]
impl Ctx for Ctx2 {
    type Base = Ctx0;
    async fn build(base: Self::Base) -> Self {
        base.token.pause().send().await.unwrap();
        Self { token: base.token }
    }
}

#[run_test]
async fn test_pause(ctx: Ctx2) -> Result<()> {
    let paused = ctx.token.paused().call().await?;
    assert!(paused);
    Ok(())
}
