use async_trait::async_trait;
use ethers::{
    core::k256::ecdsa::SigningKey,
    prelude::{DevRpcMiddleware, Http, LocalWallet, Provider, SignerMiddleware, Wallet},
};
use std::{convert::TryFrom, sync::Arc, time::Duration};

use runrun::run_ctx;
use runrun::{core::Ctx, eth::DevRpcCtx, types::*, TList};

use crate::utils::{make_factory, ERC20MinterPauser};

pub type Client = DevRpcMiddleware<SignerMiddleware<Provider<Http>, Wallet<SigningKey>>>;
pub type Inner = SignerMiddleware<Provider<Http>, Wallet<SigningKey>>;

#[derive(Clone)]
pub struct Ctx0 {
    pub client: Arc<Client>,
    pub accts: Vec<LocalWallet>,
    pub token: ERC20MinterPauser<Client>,
}

#[run_ctx]
#[async_trait]
impl Ctx for Ctx0 {
    type Base = (String, Vec<LocalWallet>);
    async fn build(args: Self::Base) -> Self {
        let endpoint = args.0;
        let accts = args.1;

        // connect to test rpc endpoint
        let provider = Provider::<Http>::try_from(endpoint)
            .unwrap()
            .interval(Duration::from_millis(1));
        let inner = SignerMiddleware::new(provider, accts[0].clone());
        let client = Arc::new(DevRpcMiddleware::new(inner));

        // deploy token contract
        let factory = make_factory("ERC20MinterPauser", &client).unwrap();
        let deployed = factory
            .deploy(("Token".to_string(), "TOK".to_string()))
            .unwrap()
            .send()
            .await
            .unwrap();
        let token = ERC20MinterPauser::new(deployed.address(), client.clone());

        println!("token address: {:?}", token.address());

        Self {
            client,
            accts,
            token,
        }
    }
}

// Expose the client for generating DevRpcHooks (initial Ctx only)
impl DevRpcCtx for Ctx0 {
    type Client = Arc<Client>;
    fn client(&self) -> Self::Client {
        self.client.clone()
    }
}

// -- Macro generated
// use crate::block1::Ctx1;
// impl ChildTypesFn for Ctx0 {
//     type Out = TList!(Ctx1);
// }
