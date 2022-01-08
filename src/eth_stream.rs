use anyhow::Result;
use async_trait::async_trait;
use ethers::{
    providers::{DevRpcMiddleware, Middleware},
    types::U256,
};
use std::ops::Deref;
use futures::{
    ready,
    stream::{Stream, StreamExt},
    FutureExt,
};

use crate::{core_stream::{Ctx, TestRes, TestSet}, hooks_stream::{Hooks, HookRunner}, types::{MapT, MapStep, ChildTypesFn, ChildTypes}};

pub async fn start_eth<'a, M, I, C, Args>(args: Args)
where
    C: Ctx<Base = Args> + TestSet<'a> + ChildTypesFn + DevRpcCtx + Unpin + Clone + Send + 'static,
    ChildTypes<C>: MapStep<HookRunner<DevRpcHooks<C>>, C>,
    C::Client: Deref<Target = DevRpcMiddleware<I>> + Unpin + Send + Sync,
    I: Middleware + Clone + 'static,
    Args: Send + 'static,
{
    let init_ctx = C::build(args).await;
    let hooks = DevRpcHooks::new(init_ctx.clone());
    let runner = HookRunner::new(hooks);
    let mut stream = MapT::<_, _, ChildTypes<C>>::new(runner, init_ctx);
    while let Some(_) = stream.next().await {}
}

// Should be implemented by the starting state to allow the runner to be built in start()
pub trait DevRpcCtx {
    type Client: Clone;
    fn client(self) -> Self::Client;
}

#[derive(Clone)]
pub struct DevRpcHooks<Ctx: DevRpcCtx> {
    snap_id: U256,
    client: <Ctx as DevRpcCtx>::Client,
}

impl<M, I, Ctx> DevRpcHooks<Ctx>
where
    M: Deref<Target = DevRpcMiddleware<I>> + Send + Sync + Clone,
    I: Middleware + Clone + 'static,
    Ctx: DevRpcCtx<Client = M> + Clone,
{
    fn new(base: Ctx) -> Self {
        Self {
            snap_id: 0usize.into(),
            client: base.client(),
        }
    }
}

#[async_trait]
impl<'a, M, I, Ctx> Hooks<'a> for DevRpcHooks<Ctx>
where
    M: Deref<Target = DevRpcMiddleware<I>> + Send + Sync + Clone,
    I: Middleware + Clone + 'static,
    Ctx: DevRpcCtx<Client = M> + Clone,
{
    async fn pre(&mut self) -> TestRes<'a> {
        self.snap_id = self.client.snapshot().await.unwrap();
        Default::default()
    }

    async fn post(&mut self) -> TestRes<'a> {
        self.client.revert_to_snapshot(self.snap_id).await.unwrap();
        Default::default()
    }
}
