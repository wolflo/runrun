use async_trait::async_trait;
use futures::{
    ready,
    stream::{Stream, StreamExt},
    FutureExt,
};
use std::{
    fmt::Debug,
    marker::PhantomData,
    pin::Pin,
    task::{Context, Poll},
};

use crate::core_stream::{Status, TestRes};

#[derive(Debug, Clone, Copy)]
struct Pre;
#[derive(Debug, Clone, Copy)]
struct Post;

struct Hook<When, S, F, Ctx> {
    stream: S,
    hook: F,
    ctx: Ctx,
    _when: PhantomData<When>,
}

#[async_trait]
pub trait TestMut<'a> {
    async fn run(&mut self) -> TestRes<'a>;
    fn skip(&self) -> TestRes<'a> {
        Default::default()
    }
}

impl<'a, S, F, Ctx> Stream for Hook<Pre, S, F, Ctx>
where
    Self: Unpin,
    S: Stream<Item = TestRes<'a>> + Unpin,
    F: TestMut<'a>,
    Ctx: 'a,
{
    type Item = S::Item;
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let hook_res = ready!(self.hook.run().poll_unpin(cx));
        // If the hook failed, propogate it as the test result
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
    F: TestMut<'a>,
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
        let hook_res = ready!(self.hook.run().poll_unpin(cx));
        if let Status::Fail = hook_res.status {
            // test passed or skipped, but hook failed. Propogate hook result
            Poll::Ready(Some(hook_res))
        } else {
            Poll::Ready(test_res)
        }
    }
}
