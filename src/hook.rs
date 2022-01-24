use async_trait::async_trait;

use crate::core::{Builder, Built, Runner, Test, TestRes};

#[async_trait]
pub trait Hook<T> {
    async fn pre(&mut self) -> T;
    async fn post(&mut self) -> T;
}

#[derive(Clone)]
pub struct HookRunner<I, H> {
    inner: I,
    hook: H,
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

pub struct HookRunnerBuilder<IB, HB> {
    pub inner_builder: IB,
    pub hook_builder: HB,
}
impl<IB, HB, T> Builder<T> for HookRunnerBuilder<IB, HB>
where
    HB: Builder<T>,
    IB: Builder<T>,
{
    type Built = HookRunner<IB::Built, HB::Built>;
    fn build(self, base: &T) -> Self::Built {
        Self::Built::new(
            self.inner_builder.build(base),
            self.hook_builder.build(base),
        )
    }
}
impl<I, H> Built for HookRunner<I, H>
where
    I: Built,
    H: Built,
{
    type Builder = HookRunnerBuilder<I::Builder, H::Builder>;
    fn builder() -> Self::Builder {
        Self::Builder {
            inner_builder: I::builder(),
            hook_builder: H::builder(),
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
impl<T> Builder<T> for NoHook {
    type Built = NoHook;
    fn build(self, _: &T) -> Self::Built {
        Self::Built {}
    }
}
impl Built for NoHook {
    type Builder = NoHook;
    fn builder() -> Self::Builder {
        Self::Builder {}
    }
}
