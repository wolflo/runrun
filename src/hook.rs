use async_trait::async_trait;
use futures::{
    future::BoxFuture,
    ready, stream,
    stream::{Stream, StreamExt},
    Future, FutureExt,
};
use pin_project::pin_project;
use std::{marker::PhantomData, task::Poll};

use crate::{
    core_stream::{MapBounds, Status, Test, TestRes, Base, Run},
    types::{ChildTypes, ExactSizeStream, FnOut, FnT, MapStep, MapT, TList},
};

#[derive(Debug, Clone, Copy, Default)]
pub struct NoHook;
#[async_trait]
impl<T> Hook<T> for NoHook
where
    T: Default + Send,
{
    async fn pre(&mut self) -> T {
        Default::default()
    }
    async fn post(&mut self) -> T {
        Default::default()
    }
}
impl NoHook {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
pub trait Hook<T> {
    async fn pre(&mut self) -> T;
    async fn post(&mut self) -> T;
}
#[derive(Debug, Clone)]
pub struct Driver<R> {
    runner: R,
}
impl<R> Driver<R> {
    pub fn new(runner: R) -> Self {
        Self { runner }
    }
}

#[async_trait]
impl<Base, R> FnT<Base> for Driver<R>
where
    Base: Send + 'static,
    R: Run<TestRes, Out = TestRes> + Send + Sync + Clone,
{
    type Output = ();
    async fn call<T>(&self, args: Base) -> FnOut<Self, Base>
    where
        Self: FnT<T>,
        T: MapBounds<Base>,
        ChildTypes<T>: MapStep<Self, T> + TList,
    {
        let ctx = T::build(args).await;
        let tests = T::tests().iter();
        let mut runner = self.runner.clone();

        let mut pass = 0;
        let mut fail = 0;
        let mut skip = 0;
        for t in tests {
            let res = runner.run(t, ctx.clone()).await;
            match res.status {
                Status::Pass => pass += 1,
                Status::Fail => fail += 1,
                Status::Skip => skip += 1,
            }
        }

        println!("tests passed : {}", pass);
        println!("tests failed : {}", fail);
        println!("tests skipped: {}", skip);

        map::<ChildTypes<T>, _, _>(self, ctx).await;
    }
}

async fn map<Lst, F, Args>(f: &F, args: Args)
where
    F: FnT<Args>,
    Lst: TList + MapStep<F, Args>,
{
    let map = MapT::new::<Lst>(f, args);
    let mut stream = stream::iter(map);
    while let Some(_) = stream.next().await {}
}

#[derive(Clone)]
pub struct HookRun<I, H> {
    pub inner: I,
    pub hook: H,
}
impl<I, H> HookRun<I, H> {
    pub fn new(inner: I, hook: H) -> Self {
        Self {
            inner,
            hook,
        }
    }
}
#[async_trait]
impl<I, H, TRes> Run<TRes> for HookRun<I, H>
where
    I: Run<TRes, Out = TestRes> + Send,
    H: Hook<TestRes> + Send,
    TRes: Default,
{
    type Out = TestRes;
    async fn run<T, Args>(&mut self, t: T, args: Args) -> Self::Out
    where
        T: Test<Args, TRes>,
        Args: Send,
    {
        let pre = self.hook.pre().await;
        if pre.status.is_fail() {
            // self.inner.skip(t, args);
            return pre;
        }
        let test = self.inner.run(t, args).await;
        let post = self.hook.post().await;
        if !test.status.is_fail() && post.status.is_fail() {
            post
        } else {
            test
        }
    }
}
