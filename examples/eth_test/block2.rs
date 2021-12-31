use anyhow::Result;
use async_trait::async_trait;

use runrun::{run_ctx, run_test, core::Ctx};

use crate::{utils::ERC20MinterPauser, init::{Ctx0, Client}};

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
        Self {
            token: base.token,
        }
    }
}

#[run_test]
async fn test_pause(ctx: Ctx2) -> Result<()> {
    println!("test_pause");
    let paused = ctx.token.paused().call().await?;
    assert!(paused);
    Ok(())
}
