use anyhow::Result;
use async_trait::async_trait;
use ethers::{
    signers::Signer,
    types::{Address, U256},
};

use runrun::{core::Ctx, run_ctx, run_test};

use crate::{block1::Ctx1, init::Client, utils::ERC20MinterPauser};

#[derive(Clone)]
pub struct Ctx3 {
    pub token: ERC20MinterPauser<Client>,
    pub minted_amt: U256,
    pub mint_receiver: Address,
    pub transfer_amt: U256,
    pub transfer_receiver: Address,
}

#[run_ctx]
#[async_trait]
impl Ctx for Ctx3 {
    type Base = Ctx1;
    async fn build(base: Self::Base) -> Self {
        let transfer_amt = base.minted_amt / 2;
        let transfer_receiver = base.accts[2].address();
        base.token
            .transfer(transfer_receiver, transfer_amt)
            .from(base.mint_receiver)
            .send()
            .await
            .unwrap();
        Self {
            token: base.token,
            minted_amt: base.minted_amt,
            mint_receiver: base.mint_receiver,
            transfer_amt,
            transfer_receiver,
        }
    }
}

#[run_test]
async fn test_transfer(ctx: Ctx3) -> Result<()> {
    // let paused = ctx.token.paused().call().await?;
    // assert!(!paused);
    Ok(())
}
