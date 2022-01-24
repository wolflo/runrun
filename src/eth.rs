use async_trait::async_trait;
use ethers::{
    providers::{DevRpcMiddleware, Middleware},
    types::U256,
};
use futures::{stream, stream::StreamExt};
use std::{marker::PhantomData, ops::Deref};

use crate::{core::Base, hook::HookRun};
use crate::{
    core::{Builder, Ctx, Driver, TestRes, TestSet},
    hook::Hook,
    types::{tmap, ChildTypes, ChildTypesFn, MapStep, MapT, TList},
};

// pub async fn start_eth<'a, M, I, C, Args>(args: Args)
// where
//     C: Ctx<Base = Args> + TestSet<'a> + ChildTypesFn + DevRpcCtx + Unpin + Clone + Send + 'static,
//     ChildTypes<C>: MapStep<Driver<HookRun<Base, DevRpcHook<C>>>, C> + TList,
//     C::Client: Deref<Target = DevRpcMiddleware<I>> + Unpin + Send + Sync,
//     I: Middleware + Clone + 'static,
//     Args: Send + 'static,
// {
//     let init_ctx = C::build(args).await;
//     let hooks = DevRpcHook::new(&init_ctx.clone());
//     let hooks = HookRun::new(Base, hooks);
//     let driver = Driver::new(hooks);
//     tmap::<C, _, _>(&driver, init_ctx).await;
// }

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


use crate::hook::{HookRunBuilder, BaseBuilder};
use crate::core::Built;

// pub type Build<T> = <T as Built>::Builder;

pub type Eth<M> = HookRun<Base, DevRpcHook<M>>;
// pub type EthBuilder<M> = Build<EthRunner<M>>;

pub struct EthBuilder<M>(PhantomData<M>);
// impl<M> Built for EthBuilder<M> {
//     // type Builder = HookRunBuilder<BaseBuilder, DevRpcBuilder<M>>;
//     // type Builder = HookRunBuilder<Build<Base>, Build<DevRpcHook<M>>>;
//     type Out = HookRun<Base, DevRpcHook<M>>;
//     type Builder = Build<HookRun<Base, DevRpcHook<M>>>;
//     fn builder() -> Self::Builder {
//         HookRunBuilder {
//             inner_builder: Base::builder(),
//             // hook_builder: DevRpcHook::<M>::builder(),
//             hook_builder: DevRpcHook::<M>::builder(),
//         }
//     }
// }
// impl<M> EthBuilder<M> {
//     pub fn new() -> <Self as Built>::Builder {
//         <Self as Built>::builder()
//     }
// }
impl Built for Base {
    type Builder = BaseBuilder;
    fn builder() -> Self::Builder {
        BaseBuilder
    }
}
impl<I, H> Built for HookRun<I, H> where I: Built, H: Built {
    type Builder = HookRunBuilder<I::Builder, H::Builder>;
    fn builder() -> Self::Builder {
        Self::Builder {
            inner_builder: I::builder(),
            hook_builder: H::builder(),
        }
    }
}

// pub type EthBuilder<M> = HookRunBuilder<BaseBuilder, DevRpcBuilder<M>>;
// // pub type EthBuilder<M> = HookRunBuilder<BaseBuilder, Build<DevRpcHook<M>>>;
// // pub type EthBuilder<M> = HookRunBuilder<BaseBuilder, <DevRpcHook<M> as Built>::Builder>;
// impl<M> EthBuilder<M> {
//     pub fn new() -> Self {
//         HookRunBuilder {
//             inner_builder: Base::builder(),
//             // hook_builder: DevRpcHook::<M>::builder(),
//             hook_builder: DevRpcHook::<M>::builder(),
//         }
//     }
// }
impl<M> Built for DevRpcHook<M> {
    type Builder = DevRpcBuilder<M>;
    fn builder() -> Self::Builder {
        DevRpcBuilder::new()
    }
}

pub struct DevRpcBuilder<M>(PhantomData<M>);
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
    type This = DevRpcHook<M>;
    fn build(self, base: &Ctx) -> Self::This {
        Self::This {
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
        println!("pre hook dev rpc");
        Default::default()
    }

    async fn post(&mut self) -> TestRes {
        self.client.revert_to_snapshot(self.snap_id).await.unwrap();
        println!("post hook dev rpc");
        Default::default()
    }
}
