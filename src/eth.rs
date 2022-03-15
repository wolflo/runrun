use async_trait::async_trait;
use ethers::{
    providers::{DevRpcMiddleware, Middleware},
    types::U256,
};
use std::{marker::PhantomData, ops::Deref};

use crate::{core::BaseRunner, hook::HookRunner};
use crate::{
    core::{Builder, Built, TestRes},
    hook::Hook,
};

pub type Eth<M> = HookRunner<BaseRunner, DevRpcHook<M>>;

/// This trait should be implemented by the initial state used for Ethereum tests
/// to expose the test rpc client and allow the runner to be built in the start()
/// function.
///
/// # Example
///
///```
/// use std::sync::Arc;
/// use async_trait::async_trait;
/// use ethers::{
///     core::k256::ecdsa::SigningKey,
///     prelude::{DevRpcMiddleware, Http, LocalWallet, Provider, SignerMiddleware, Wallet},
/// };
/// use runrun::{run_ctx, core::Ctx, eth::DevRpcCtx};
///
/// pub type Client = DevRpcMiddleware<SignerMiddleware<Provider<Http>, Wallet<SigningKey>>>;
///
/// #[derive(Clone)]
/// pub struct InitialCtx {
///     pub client: Arc<Client>,
/// }
///
/// #[run_ctx]
/// #[async_trait]
/// impl Ctx for InitialCtx {
///     type Base = Arc<Client>;
///     async fn build(client: Self::Base) -> Self {
///         Self { client }
///     }
/// }
///
/// // Expose the client for generating DevRpcHooks (initial Ctx only)
/// impl DevRpcCtx for InitialCtx {
/// type Client = Arc<Client>;
///     fn client(&self) -> Self::Client {
///         self.client.clone()
///     }
/// }
///```
pub trait DevRpcCtx {
    type Client: Clone;
    fn client(&self) -> Self::Client;
}

/// An Ethereum-specific hook that leverages test rpc methods to save and reset the client
/// state between each test.
///
/// The hook calls `evm_snapshot` before a test or block is run, saving the returned snapshot
/// id. After the test is run, the hook calls `evm_revert` to reset the state.
#[derive(Clone)]
pub struct DevRpcHook<M> {
    snap_id: U256,
    client: M,
}
impl<M> DevRpcHook<M> {
    pub fn new(client: M) -> Self {
        Self {
            snap_id: Default::default(),
            client,
        }
    }
}

pub struct DevRpcBuilder<M>(PhantomData<M>);
impl<M> Built for DevRpcHook<M> {
    type Builder = DevRpcBuilder<M>;
    fn builder() -> Self::Builder {
        DevRpcBuilder::new()
    }
}

impl<M> DevRpcBuilder<M> {
    pub fn new() -> Self {
        Self(PhantomData)
    }
}
impl<M> Default for DevRpcBuilder<M> {
    fn default() -> Self {
        Self::new()
    }
}
impl<M, I, Ctx> Builder<Ctx> for DevRpcBuilder<M>
where
    M: Deref<Target = DevRpcMiddleware<I>> + Send + Sync + Clone,
    I: Middleware + Clone + 'static,
    Ctx: DevRpcCtx<Client = M> + Clone,
{
    type Built = DevRpcHook<M>;
    fn build(self, base: &Ctx) -> Self::Built {
        Self::Built {
            snap_id: 0usize.into(),
            client: base.client(),
        }
    }
}

#[async_trait]
impl<M, I> Hook<TestRes> for DevRpcHook<M>
where
    M: Deref<Target = DevRpcMiddleware<I>> + Send + Sync + Clone,
    I: Middleware + Clone + 'static,
{
    async fn pre(&mut self) -> TestRes {
        self.snap_id = self.client.snapshot().await.unwrap();
        Default::default()
    }

    async fn post(&mut self) -> TestRes {
        self.client.revert_to_snapshot(self.snap_id).await.unwrap();
        Default::default()
    }
}
