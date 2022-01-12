use anyhow::Result;
use async_trait::async_trait;
use ethers::{
    prelude::LocalWallet,
    signers::Signer,
    types::{Address, U256},
    utils::parse_ether,
};
use std::sync::Arc;

use runrun::{core_stream::Ctx, run_ctx, run_test};

use crate::{
    init::{Client, Ctx0},
    utils::ERC20MinterPauser,
};

#[derive(Clone)]
pub struct Ctx1 {
    pub client: Arc<Client>,
    pub accts: Vec<LocalWallet>,
    pub token: ERC20MinterPauser<Client>,
    pub minted_amt: U256,
    pub mint_receiver: Address,
}

#[run_ctx]
#[async_trait]
impl Ctx for Ctx1 {
    type Base = Ctx0;
    async fn build(base: Self::Base) -> Self {
        let minted_amt = parse_ether(100).unwrap();
        let mint_receiver = base.accts[1].address();
        base.token
            .mint(mint_receiver, minted_amt)
            .send()
            .await
            .unwrap();
        Self {
            client: base.client,
            accts: base.accts,
            token: base.token,
            minted_amt,
            mint_receiver,
        }
    }
}

#[run_test]
async fn test_minted(ctx: Ctx1) -> Result<()> {
    println!("running test_minted");
    let paused = ctx.token.paused().call().await?;
    assert!(!paused);
    Ok(())
}