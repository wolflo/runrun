#![allow(unused_variables)]
#![allow(dead_code)]

pub mod core;
pub mod eth;
pub mod hooks;
mod ty;

#[cfg(test)]
mod tests {
    #![allow(unused_imports)]
    use async_trait::async_trait;
    use ethers::{
        core::{k256::ecdsa::SigningKey, rand::thread_rng},
        prelude::{
            DevRpcMiddleware, Http, LocalWallet, Middleware, Provider, SignerMiddleware, Wallet,
        },
        signers::{coins_bip39::English, MnemonicBuilder},
        utils::{Ganache, GanacheInstance},
    };
    use std::{convert::TryFrom, sync::Arc, time::Duration};

    use crate::core::DebugTrait;
    use crate::{
        core::{start, Ctx, TestSet},
        eth::{DevRpcCtx, DevRpcHooks},
        hooks::HookRunner,
        ty::*,
    };

    type Innerware = SignerMiddleware<Provider<Http>, Wallet<SigningKey>>;
    type Client = Arc<DevRpcMiddleware<Innerware>>;
    #[derive(Clone)]
    struct Ctx0 {
        client: Client,
        accts: Vec<Wallet<SigningKey>>,
    }
    #[async_trait]
    impl Ctx for Ctx0 {
        type Base = String;
        async fn build(endpoint: Self::Base) -> Self {
            let provider = Provider::<Http>::try_from(endpoint)
                .unwrap()
                .interval(Duration::from_millis(1));
            let mnemonic = MnemonicBuilder::<English>::default();
            let mut rng = thread_rng();
            let accts = [0; 5]
                .map(|_| mnemonic.build_random(&mut rng).unwrap())
                .to_vec();
            let inner = SignerMiddleware::new(provider, accts[0].clone());
            let client = Arc::new(DevRpcMiddleware::new(inner));
            Self { client, accts }
        }
    }
    impl DevRpcCtx for Ctx0 {
        type Inner = Innerware;
        fn client(self) -> Arc<DevRpcMiddleware<Self::Inner>> {
            self.client
        }
    }

    #[async_trait]
    impl DebugTrait for Ctx0 {
        async fn debug(&self) {
            println!("ahhhhh im debugggggging");
        }
    }

    //  -- Macro
    impl ChildTypesFn for Ctx0 {
        type Out = TNil;
    }
    impl TestSet for Ctx0 {
        fn tests() -> &'static [fn(Self)] {
            &[|_| {
                println!("test 1");
            }]
        }
    }
    //  --

    #[tokio::test]
    async fn test() {
        let node = Ganache::new().port(8547u16).spawn();
        start::<HookRunner<DevRpcHooks<Client, Ctx0>>, Ctx0, String>(node.endpoint()).await;
    }
}
