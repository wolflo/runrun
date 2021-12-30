use anyhow::Result;
use async_trait::async_trait;
use ethers::{
    providers::{DevRpcMiddleware, Middleware},
    types::U256,
};
use std::{marker::PhantomData, sync::Arc};

use crate::hooks::*;

// Convenience type for Eth DevRpc hooks (snapshot and reset between tests)
pub type EthRunner<Ctx> =
    HookRunner<DevRpcHooks<Arc<DevRpcMiddleware<<Ctx as DevRpcCtx>::Inner>>, Ctx>>;

// Should be implemented by the starting state to allow the runner to be built in start()
pub trait DevRpcCtx {
    type Inner: Middleware;
    fn client(self) -> Arc<DevRpcMiddleware<Self::Inner>>;
}

#[derive(Clone)]
pub struct DevRpcHooks<Client, Ctx> {
    snap_id: U256,
    client: Client,
    phantom: PhantomData<Ctx>, // Constrains the Ctx type that gives us the client
}
#[async_trait]
impl<M, Ctx> Hooks for DevRpcHooks<Arc<DevRpcMiddleware<M>>, Ctx>
where
    M: Middleware + Clone + 'static,
    Ctx: DevRpcCtx<Inner = M> + Send + Sync + Clone,
{
    type Base = Ctx;
    fn new(base: Self::Base) -> Self {
        Self {
            snap_id: 0usize.into(),
            client: base.client(),
            phantom: PhantomData,
        }
    }
    async fn before_each(&mut self) -> Result<()> {
        self.snap_id = self.client.snapshot().await?;
        Ok(())
    }
    async fn after_each(&mut self) -> Result<()> {
        self.client.revert_to_snapshot(self.snap_id).await?;
        Ok(())
    }
}
