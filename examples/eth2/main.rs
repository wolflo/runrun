use anyhow::Result;
use async_trait::async_trait;
use ethers::{
    core::k256::ecdsa::SigningKey,
    prelude::{
        DevRpcMiddleware, Http, LocalWallet, Middleware, Provider, SignerMiddleware, Wallet,
    },
    utils::Ganache,
};
use linkme::distributed_slice;
use std::{convert::TryFrom, sync::Arc, time::Duration};

use runrun::{core_stream::*, eth_stream::*, register_ctx};
use runrun_derive::*;

pub type Client = DevRpcMiddleware<SignerMiddleware<Provider<Http>, Wallet<SigningKey>>>;
pub type Inner = SignerMiddleware<Provider<Http>, Wallet<SigningKey>>;

#[derive(Clone)]
pub struct Ctx0 {
    pub client: Arc<Client>,
    pub accts: Vec<LocalWallet>,
}
#[async_trait]
impl Ctx for Ctx0 {
    type Base = (String, Vec<LocalWallet>);
    async fn build(args: Self::Base) -> Self {
        println!("building 0");
        let endpoint = args.0;
        let accts = args.1;

        // connect to test rpc endpoint
        let provider = Provider::<Http>::try_from(endpoint)
            .unwrap()
            .interval(Duration::from_millis(1));
        let inner = SignerMiddleware::new(provider, accts[0].clone());
        let client = Arc::new(DevRpcMiddleware::new(inner));
        Self { client, accts }
    }
}
impl DevRpcCtx for Ctx0 {
    type Client = Arc<Client>;
    fn client(self) -> Self::Client {
        self.client
    }
}
impl TestSet<'static> for Ctx0 {
    fn tests() -> &'static [&'static dyn Test<'static, Self>] {
        &TESTS_ON_CTX0
    }
}
impl TestSet<'static> for Ctx2 {
    fn tests() -> &'static [&'static dyn Test<'static, Self>] {
        &TESTS_ON_CTX2
    }
}
#[derive(Debug, Clone)]
pub struct Ctx2 {}
#[async_trait]
impl Ctx for Ctx2 {
    type Base = Ctx0;
    async fn build(ctx: Self::Base) -> Self {
        println!("building 2");
        Self {}
    }
}
#[derive(Debug, Clone)]
pub struct Ctx1 {}
#[async_trait]
impl Ctx for Ctx1 {
    type Base = Ctx0;
    async fn build(ctx: Self::Base) -> Self {
        println!("building 1");
        Self {}
    }
}
impl TestSet<'static> for Ctx1 {
    fn tests() -> &'static [&'static dyn Test<'static, Self>] {
        &TESTS_ON_CTX1
    }
}
async fn test_11(_: Ctx1) -> Result<()> {
    println!("running test_11");
    Ok(())
}
async fn test_12(_: Ctx1) -> Result<()> {
    println!("running test_12");
    Ok(())
}
#[distributed_slice]
pub static TESTS_ON_CTX1: [&'static dyn Test<'static, Ctx1>] = [..];
#[distributed_slice]
pub static TESTS_ON_CTX2: [&'static dyn Test<'static, Ctx2>] = [..];
#[distributed_slice(TESTS_ON_CTX1)]
pub static __T11: &dyn Test<Ctx1> = &TestCase {
    name: "test_11",
    test: &|x| Box::pin(test_11(x)),
};
#[distributed_slice(TESTS_ON_CTX1)]
pub static __T12: &dyn Test<Ctx1> = &TestCase {
    name: "test_12",
    test: &|x| Box::pin(test_12(x)),
};

#[distributed_slice]
pub static TESTS_ON_CTX0: [&'static dyn Test<'static, Ctx0>] = [..];

#[derive(Debug, Clone)]
pub struct Ctx3;

#[run_ctx]
#[async_trait]
impl Ctx for Ctx3 {
    type Base = Ctx2;
    async fn build(ctx: Self::Base) -> Self {
        println!("building 3");
        Self {}
    }
}
#[run_test]
async fn test_31(_: Ctx3) -> Result<()> {
    println!("running test_31");
    Ok(())
}
// #[distributed_slice(TESTS_ON_CTX3)]
// pub static __T31: &dyn Test<'static, Ctx3> = &TestCase {
//     name: "test_31",
//     test: &|x| Box::pin(test_31(x)),
// };
// impl TestSet<'static> for Ctx3 {
//     fn tests() -> &'static [&'static dyn Test<'static, Self>] {
//         &TESTS_ON_CTX3
//     }
// }
// #[distributed_slice]
// pub static TESTS_ON_CTX3: [&'static dyn Test<'static, Ctx3>] = [..];

register_ctx!(Ctx0, [Ctx1, Ctx2]);
register_ctx!(Ctx1);
register_ctx!(Ctx2, [Ctx3]);
register_ctx!(Ctx3);

#[tokio::main]
async fn main() {
    // Start a ganache node
    let node = Ganache::new().spawn();
    let accts: Vec<LocalWallet> = node.keys()[..5].iter().map(|x| x.clone().into()).collect();

    start_eth::<Inner, _, Ctx0, _>((node.endpoint(), accts)).await;
}
