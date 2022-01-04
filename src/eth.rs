use anyhow::Result;
use async_trait::async_trait;
use ethers::{
    providers::{DevRpcMiddleware, Middleware},
    types::U256,
};
use std::ops::Deref;

use crate::hooks::*;

// Convenience type for Eth DevRpc hooks (snapshot and reset between tests)
pub type EthRunner<Ctx> = HookRunner<DevRpcHooks<Ctx>>;

// Should be implemented by the starting state to allow the runner to be built in start()
pub trait DevRpcCtx {
    type Client: Clone; // Clone required for DevRpcHooks to derive Clone
    fn client(self) -> Self::Client;
}

#[derive(Clone)]
pub struct DevRpcHooks<Ctx: DevRpcCtx> {
    snap_id: U256,
    client: <Ctx as DevRpcCtx>::Client,
}
#[async_trait]
impl<M, I, Ctx> Hooks for DevRpcHooks<Ctx>
where
    M: Deref<Target = DevRpcMiddleware<I>> + Send + Sync + Clone,
    I: Middleware + Clone + 'static,
    Ctx: DevRpcCtx<Client = M> + Send + Sync + Clone,
{
    type Base = Ctx;
    fn new(base: Self::Base) -> Self {
        Self {
            snap_id: 0usize.into(),
            client: base.client(),
        }
    }
    // async fn before_each(&mut self) -> Result<()> {
    //     self.snap_id = self.client.snapshot().await?;
    //     Ok(())
    // }
    // async fn after_each(&mut self) -> Result<()> {
    //     self.client.revert_to_snapshot(self.snap_id).await?;
    //     Ok(())
    // }
}
