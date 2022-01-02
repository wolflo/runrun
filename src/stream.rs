use crate::core::TestResult;
use async_trait::async_trait;
use futures::{
    ready, stream,
    stream::{Stream, StreamExt},
    Future, FutureExt,
};
use std::{
    fmt::Debug,
    marker::PhantomData,
    pin::Pin,
    task::{Context, Poll},
};

pub type AsyncOutput<R> = std::pin::Pin<Box<dyn Future<Output = R> + Send>>;
pub type AsyncFn<X, Y> = dyn Fn(X) -> AsyncOutput<Y> + Send + Sync;

pub trait Runner: Stream<Item = TestResult> {}

struct Pre;
struct Post;
struct Hook<When, S, F, Ctx> {
    stream: S,
    hook: F,
    ctx: Ctx,
    _when: PhantomData<When>,
}
impl<'a, S, F, Ctx> Stream for Hook<Pre, S, F, Ctx>
where
    Self: Unpin,
    S: Stream<Item = TestRes<'a>> + Unpin,
    F: TestRef<'a, Ctx>,
    Ctx: 'a,
{
    type Item = S::Item;
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let hook_res = ready!(self.hook.run(&self.ctx).poll_unpin(cx));
        // If the hook failed propogate it as the test result
        if let Status::Fail = hook_res.status {
            return Poll::Ready(Some(hook_res));
        }
        // Hook passed or skipped, so propogate the original stream
        self.stream.poll_next_unpin(cx)
    }
}
impl<'a, S, F, Ctx> Stream for Hook<Post, S, F, Ctx>
where
    Self: Unpin,
    S: Stream<Item = TestRes<'a>> + Unpin,
    F: TestRef<'a, Ctx>,
    Ctx: 'a,
{
    type Item = S::Item;
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let test_res = ready!(self.stream.poll_next_unpin(cx));
        match test_res {
            // test failed, propogate the result and don't run the hook
            Some(res) if matches!(res.status, Status::Fail) => return Poll::Ready(Some(res)),
            // test passed or skipped, continue
            Some(_) => (),
            // no more tests
            None => return Poll::Ready(None),
        }
        // TODO: skip this hook at times?
        let hook_res = ready!(self.hook.run(&self.ctx).poll_unpin(cx));
        if matches!(hook_res.status, Status::Fail) {
            // test passed or skipped, but hook failed. Propogate hook result
            Poll::Ready(Some(hook_res))
        } else {
            Poll::Ready(test_res)
        }
    }
}

// Filters will act on the input iterator before the Streamer turns
// it into a Stream
struct Streamer<'a, I, Ctx> {
    tests: I,
    ctx: Ctx,
    _tick: std::marker::PhantomData<&'a u8>,
}
impl<'a, I, T, Ctx> Stream for Streamer<'a, I, Ctx>
where
    Self: Unpin,
    I: Iterator<Item = T> + 'a,
    T: Test<'a, Ctx> + Unpin,
{
    type Item = TestRes<'a>;
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let next = self.tests.next();
        if let Some(t) = next {
            let res = ready!(t.run(&self.ctx).poll_unpin(cx));
            Poll::Ready(Some(res))
        } else {
            Poll::Ready(None)
        }
    }
}

#[derive(Clone, Copy)]
pub enum Status {
    Pass,
    Fail,
    Skip,
}
#[derive(Clone)]
pub struct TestRes<'a> {
    status: Status,
    trace: &'a dyn Debug,
}
#[async_trait]
pub trait TestRef<'a, Args> {
    async fn run(&self, args: &Args) -> TestRes<'a>;
    fn skip(&self) -> TestRes<'a> {
        TestRes::default()
    }
}
#[async_trait]
pub trait Test<'a, Args> {
    async fn run(self, args: &Args) -> TestRes<'a>;
    fn skip(&self) -> TestRes<'a> {
        TestRes::default()
    }
}
#[async_trait]
impl<'a, T, E, Args> Test<'a, Args> for Result<T, E>
where
    T: Test<'a, Args> + Send + Sync,
    E: Into<&'a dyn Debug> + Send + Sync + Clone,
    Args: Send + Sync,
{
    async fn run(self, args: &Args) -> TestRes<'a> {
        match self {
            Ok(r) => r.run(args).await,
            Err(e) => TestRes {
                status: Status::Fail,
                trace: e.into(),
            },
        }
    }
}

impl Default for Status {
    fn default() -> Self {
        Self::Skip
    }
}
impl Default for TestRes<'_> {
    fn default() -> Self {
        Self {
            status: Status::default(),
            trace: &"",
        }
    }
}
