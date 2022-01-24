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

// Should be implemented by the starting state to allow the runner to be built in start()
pub trait DevRpcCtx {
    type Client: Clone;
    fn client(&self) -> Self::Client;
}

#[derive(Clone)]
pub struct DevRpcHook<M> {
    snap_id: U256,
    client: M,
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
