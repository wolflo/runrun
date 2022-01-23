use async_trait::async_trait;
use ethers::{
    providers::{DevRpcMiddleware, Middleware},
    types::U256,
};
use futures::{stream, stream::StreamExt};
use std::ops::Deref;

use crate::{
    core::{Ctx, TestRes, TestSet},
    hook::{Hook, Driver},
    types::{ChildTypes, ChildTypesFn, MapStep, MapT, TList},
};
use crate::{hook::HookRun, core::Base};
pub async fn start_eth<'a, M, I, C, Args>(args: Args)
where
    C: Ctx<Base = Args> + TestSet<'a> + ChildTypesFn + DevRpcCtx + Unpin + Clone + Send + 'static,
    ChildTypes<C>: MapStep<Driver<HookRun<Base, DevRpcHook<C>>>, C> + TList,
    C::Client: Deref<Target = DevRpcMiddleware<I>> + Unpin + Send + Sync,
    I: Middleware + Clone + 'static,
    Args: Send + 'static,
{
    let init_ctx = C::build(args).await;
    let hooks = DevRpcHook::new(init_ctx.clone());
    let hooks = HookRun::new(Base, hooks);
    let runner = Driver::new(hooks);
    let iter = MapT::new::<ChildTypes<C>>(&runner, init_ctx);
    let mut stream = stream::iter(iter);
    while let Some(set) = stream.next().await {
        set.await;
    }
}

// Should be implemented by the starting state to allow the runner to be built in start()
pub trait DevRpcCtx {
    type Client: Clone;
    fn client(self) -> Self::Client;
}

#[derive(Clone)]
pub struct DevRpcHook<Ctx: DevRpcCtx> {
    snap_id: U256,
    client: <Ctx as DevRpcCtx>::Client,
}

impl<M, I, Ctx> DevRpcHook<Ctx>
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
impl<M, I, Ctx> Hook<TestRes> for DevRpcHook<Ctx>
where
    M: Deref<Target = DevRpcMiddleware<I>> + Send + Sync + Clone,
    I: Middleware + Clone + 'static,
    Ctx: DevRpcCtx<Client = M> + Clone,
{
    async fn pre(&mut self) -> TestRes {
        self.snap_id = self.client.snapshot().await.unwrap();
        println!("pre hook dev rpc");
        Default::default()
    }

    async fn post(&mut self) -> TestRes {
        self.client.revert_to_snapshot(self.snap_id).await.unwrap();
        println!("post hook dev rpc");
        Default::default()
    }
}
