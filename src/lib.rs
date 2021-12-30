#![allow(unused_variables)]
#![allow(dead_code)]

pub mod core;
pub mod eth;
pub mod hooks;
mod ty;

#[cfg(test)]
mod tests {
    #![allow(unused_imports)]
    use std::{sync::Arc, convert::TryFrom, time::Duration};
    use async_trait::async_trait;
    use crate::{ty::*, eth::{DevRpcCtx, DevRpcHooks}, core::{Ctx, TestSet, start}, hooks::HookRunner};
    use ethers::{prelude::{SignerMiddleware, DevRpcMiddleware, Provider, Http, Wallet, LocalWallet}, core::k256::ecdsa::SigningKey, utils::Ganache};

    type Innerware = SignerMiddleware<Provider<Http>, Wallet<SigningKey>>;
    type Client = DevRpcMiddleware<Innerware>;
    #[derive(Clone)]
    struct Ctx0 {
        client: Arc<Client>,
    }
    #[async_trait]
    impl Ctx for Ctx0 {
        type Base = ();
        async fn build(base: Self::Base) -> Self {
            let node = Ganache::new().spawn();
            let provider = Provider::<Http>::try_from(node.endpoint()).unwrap().interval(Duration::from_millis(1));
            let accts: Vec<LocalWallet> = node.keys()[..5].iter().map(|x| x.clone().into()).collect();
            let client = Arc::new(DevRpcMiddleware::new(SignerMiddleware::new(
                provider,
                accts[0].clone(),
            )));
            Self { client }
        }
    }
    impl DevRpcCtx for Ctx0 {
        type Inner = Innerware;
        fn client(self) -> Arc<DevRpcMiddleware<Self::Inner>> { self.client }
    }

    //  -- Macro
    impl ChildTypesFn for Ctx0 {
        type Out = TNil;
    }
    impl TestSet for Ctx0 {
        fn tests() -> &'static [fn(Self)] { &[] }
    }
    //  --

    #[tokio::test]
    async fn test() {
        start::<HookRunner<DevRpcHooks<Arc<Client>, Ctx0>>, Ctx0>().await;
        println!("foo");
    }
}
