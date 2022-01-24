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
    core::{BaseRunner, Built, Builder, MapBounds, Runner, Status, Test, TestRes},
    types::{tmap, ChildTypes, FnOut, FnT, MapStep, MapT, TList},
};

pub struct HookRunnerBuilder<IB, HB> {
    pub inner_builder: IB,
    pub hook_builder: HB,
}
impl<IB, HB, T> Builder<T> for HookRunnerBuilder<IB, HB>
where
    HB: Builder<T>,
    IB: Builder<T>,
{
    type This = HookRunner<IB::This, HB::This>;
    fn build(self, base: &T) -> Self::This {
        Self::This {
            inner: self.inner_builder.build(base),
            hook: self.hook_builder.build(base),
        }
    }
}

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

#[derive(Clone)]
pub struct HookRunner<I, H> {
    pub inner: I,
    pub hook: H,
}
impl<I, H> HookRunner<I, H> {
    pub fn new(inner: I, hook: H) -> Self {
        Self { inner, hook }
    }
}
#[async_trait]
impl<I, H> Runner for HookRunner<I, H>
where
    I: Runner + Send,
    H: Hook<TestRes> + Send + Sync,
{
    type Inner = I;
    fn inner(&mut self) -> &mut Self::Inner {
        &mut self.inner
    }

    async fn run<T, Args>(&mut self, t: T, args: Args) -> TestRes
    where
        T: Test<Args>,
        Args: Send,
    {
        let pre = self.hook.pre().await;
        if pre.status.is_fail() {
            self.inner.skip(t);
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
impl<I, H> Built for HookRunner<I, H> where I: Built, H: Built {
    type Builder = HookRunnerBuilder<I::Builder, H::Builder>;
    fn builder() -> Self::Builder {
        Self::Builder {
            inner_builder: I::builder(),
            hook_builder: H::builder(),
        }
    }
}
