use async_trait::async_trait;
use futures::{
    ready,
    stream::{Stream, StreamExt},
    FutureExt,
};
use std::{
    pin::Pin,
    task::{Context, Poll},
};

use crate::{types::{ChildTypes, MapStep, FnT, FnOut, MapT}, core_stream::{MapBounds, Status, TestRes, TestStream}};

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

impl<S, F, Ctx> HookStream<S, F, Ctx> 
{
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
        Self {
            hooks
        }
    }
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
        Self: FnT<T> + Clone,
        T: MapBounds<Args>,
        ChildTypes<T>: MapStep<Self, T>,
    {
        let ctx = T::build(args).await;
        let tests = T::tests();
        let test_stream = TestStream::new(tests.iter(), ctx.clone());
        let mut test_stream = HookStream::new(test_stream, self.hooks.clone(), ctx.clone());

        let mut pass = 0;
        let mut fail = 0;
        let mut skip = 0;
        while let Some(test_res) = test_stream.next().await {
            match test_res.status {
                Status::Pass => pass += 1,
                Status::Fail => fail += 1,
                Status::Skip => skip += 1,
            }
        }
        println!("tests passed : {}", pass);
        println!("tests failed : {}", fail);
        println!("tests skipped: {}", skip);

        let child_stream = MapT::<_, _, ChildTypes<T>>::new(self.clone(), ctx);
        let _c = child_stream.collect::<Vec<_>>();
    }
}
