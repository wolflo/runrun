use async_trait::async_trait;
use futures::{
    ready,
    stream::{Stream, StreamExt},
    stream,
    FutureExt,
};
use std::{
    pin::Pin,
    task::{Context, Poll},
};

use crate::{
    core_stream::{MapBounds, Status, TestRes, Test, TestStream},
    types::{ChildTypes, FnOut, FnT, MapStep, MapT},
};

#[derive(Debug, Clone, Copy, Default)]
pub struct NoopHooks;
#[async_trait]
impl<'a> Hooks<'a> for NoopHooks {
    async fn pre(&mut self) -> TestRes<'a> {
        Default::default()
    }
    async fn post(&mut self) -> TestRes<'a> {
        Default::default()
    }
}
impl NoopHooks {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
pub trait Hooks<'a> {
    async fn pre(&mut self) -> TestRes<'a>;
    async fn post(&mut self) -> TestRes<'a>;
}
#[derive(Debug, Clone)]
struct HookStream<S, F, Ctx> {
    stream: S,
    hooks: F,
    ctx: Ctx,
}

impl<S, F, Ctx> HookStream<S, F, Ctx> {
    fn new(stream: S, hooks: F, ctx: Ctx) -> Self {
        Self { stream, hooks, ctx }
    }
}
impl<'a, S, F, Ctx> Stream for HookStream<S, F, Ctx>
where
    Self: Unpin,
    S: Stream<Item = TestRes<'a>> + Unpin,
    F: Hooks<'a>,
    Ctx: 'a,
{
    type Item = S::Item;
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let pre_res = ready!(self.hooks.pre().poll_unpin(cx));
        // If the pre hook failed, propogate it as the test result
        if let Status::Fail = pre_res.status {
            return Poll::Ready(Some(pre_res));
        }
        // Pre hook passed or skipped, so resolve the underlying stream
        let test_res = ready!(self.stream.poll_next_unpin(cx));
        match test_res {
            // test failed, propogate the result and don't run the post hook
            Some(res) if matches!(res.status, Status::Fail) => return Poll::Ready(Some(res)),
            // test passed or skipped, continue
            Some(_) => (),
            // no more tests, don't run the post hook?
            None => return Poll::Ready(None),
        }
        let post_res = ready!(self.hooks.post().poll_unpin(cx));
        if let Status::Fail = post_res.status {
            // test passed or skipped, but hook failed. Propogate post hook result
            Poll::Ready(Some(post_res))
        } else {
            Poll::Ready(test_res)
        }
    }
}

#[derive(Debug, Clone)]
pub struct HookRunner<H> {
    hooks: H,
}
impl<H> HookRunner<H> {
    pub fn new(hooks: H) -> Self {
        Self { hooks }
    }
}
pub async fn run_test<'a, T>(ctx: T, t: &dyn Test<'a, T>) -> TestRes<'a> {
    t.run(ctx).await
}
#[async_trait]
impl<Args, H> FnT<Args> for HookRunner<H>
where
    Args: Send + 'static,
    H: Hooks<'static> + Unpin + Clone + Send + Sync,
{
    type Output = ();
    async fn call<T>(&self, args: Args) -> FnOut<Self, Args>
    where
        Self: FnT<T>,
        T: MapBounds<Args>,
        ChildTypes<T>: MapStep<Self, T>,
    {
        let ctx = T::build(args).await;
        let tests = T::tests();
        // let test_stream = TestStream::new(tests.iter(), ctx.clone());
        // let mut test_stream = HookStream::new(test_stream, self.hooks.clone(), ctx.clone());

        let test_stream = stream::iter(tests.iter());
        let mut res_stream = test_stream.map(|&t| t.run(ctx.clone()));
        // let mut res_stream = HookStream::new(res_stream, self.hooks.clone(), ctx.clone());

        let mut pass = 0;
        let mut fail = 0;
        let mut skip = 0;
        while let Some(test_res) = res_stream.next().await {
            println!("next test");
            match test_res.await.status {
                Status::Pass => pass += 1,
                Status::Fail => fail += 1,
                Status::Skip => skip += 1,
            }
        }
        println!("tests passed : {}", pass);
        println!("tests failed : {}", fail);
        println!("tests skipped: {}", skip);

        let child_iter = MapT::<_, _, ChildTypes<T>>::new(self, ctx);
        let mut child_stream = stream::iter(child_iter);
        while let Some(_) = child_stream.next().await { }
    }
}
