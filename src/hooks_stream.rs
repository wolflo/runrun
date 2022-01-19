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
    core_stream::{MapBounds, Status, Test, TestRes},
    types::{ChildTypes, ExactSizeStream, FnOut, FnT, MapStep, MapT, TList},
};

#[derive(Debug, Clone, Copy, Default)]
pub struct NoHook;
#[async_trait]
impl<'a> Hook<'a> for NoHook {
    async fn pre(&mut self) -> TestRes<'a> {
        Default::default()
    }
    async fn post(&mut self) -> TestRes<'a> {
        Default::default()
    }
}
#[async_trait]
impl<T> HookT<T> for NoHook
where
    T: Default,
{
    async fn pre(&mut self) -> (T, &mut Self) {
        println!("Running NoHook pre.");
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        (Default::default(), self)
    }
    async fn post(&mut self) -> (T, &mut Self) {
        println!("Running NoHook post.");
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        (Default::default(), self)
    }
}
impl NoHook {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
pub trait Hook<'a> {
    async fn pre(&mut self) -> TestRes<'a>;
    async fn post(&mut self) -> TestRes<'a>;
}
#[derive(Debug, Clone)]
pub struct HookRunner<H> {
    hook: H,
}
impl<H> HookRunner<H> {
    pub fn new(hook: H) -> Self {
        Self { hook }
    }
}

pub async fn run_test<'a, T, Args, H>(t: T, args: Args, mut hook: H) -> TestRes<'a>
where
    H: Hook<'a>,
    T: Test<'a, Args>,
{
    // let (_, pre_res) = hook.pre().await;
    let pre_res = hook.pre().await;
    // If the pre hook failed, return it as the test result, skipping test and post hook
    if let Status::Fail = pre_res.status {
        return pre_res;
    }
    let test_res = t.run(args).await;
    // If the test failed, return the result and don't run the post hook
    if let Status::Fail = test_res.status {
        return test_res;
    }
    let post_res = hook.post().await;
    // If the test passed but the post hook failed, return the post hook failure
    if let Status::Fail = post_res.status {
        return post_res;
    }
    // Everything passed. Return the test result
    test_res
}

#[async_trait]
impl<Args, H> FnT<Args> for HookRunner<H>
where
    Args: Send + 'static,
    H: Hook<'static> + Unpin + Clone + Send + Sync,
    // H: Hook<TestRes<'static>> + Unpin + Clone + Send + Sync,
{
    type Output = ();
    async fn call<T>(&self, args: Args) -> FnOut<Self, Args>
    where
        Self: FnT<T>,
        T: MapBounds<Args>,
        ChildTypes<T>: MapStep<Self, T> + TList,
    {
        let ctx = T::build(args).await;
        let tests = T::tests();

        // let mut test_res = stream::iter(
        //     tests.iter().map(|&t| t.run(ctx.clone())), // .map(|&t| run_test(t, ctx.clone(), self.hook.clone())),
        // );
        // turn an iterator of Tests into a stream of TestRes. Probably a cleaner way to do this
        let test_res = tests.iter().map(|t| t.run(ctx.clone()));
        let mut stream = stream::unfold(test_res, |mut iter| async move {
            match iter.next() {
                Some(t) => Some((t.await, iter)),
                None => None,
            }
        })
        .boxed();

        let mut hook = NoHook::new();
        let mut hooks = HookStream::new(stream, &mut hook);
        if let State::Init(_) = hooks.state {
            println!("Some!");
        }
        let foo = hooks.next().await;
        match hooks.state {
            State::Init(_) => println!("state is init"),
            State::Wait(_) => println!("state is wait"),
        }

        // let mut wrap = Wrap::new(stream, || async { tokio::time::sleep(tokio::time::Duration::from_secs(3)).await; Default::default() }).boxed();
        // let mut wrap = Wrap::new(stream, NoHook, || async { tokio::time::sleep(tokio::time::Duration::from_secs(3)).await; Default::default() }).boxed();

        let mut pass = 0;
        let mut fail = 0;
        let mut skip = 0;
        // while let Some(res) = wrap.next().await {
        //     match res.status {
        //         Status::Pass => pass += 1,
        //         Status::Fail => fail += 1,
        //         Status::Skip => skip += 1,
        //     }
        // }
        println!("tests passed : {}", pass);
        println!("tests failed : {}", fail);
        println!("tests skipped: {}", skip);

        let child_iter = MapT::new::<ChildTypes<T>>(self, ctx.clone());
        let mut child_stream = stream::iter(child_iter);
        while let Some(_) = child_stream.next().await {}
    }
}

#[async_trait]
pub trait HookT<T> {
    async fn pre(&mut self) -> (T, &mut Self);
    async fn post(&mut self) -> (T, &mut Self);
}
#[pin_project(project = HookFutProj)]
enum HookFut<Fut> {
    None,
    Pre(#[pin] Fut),
    Test(#[pin] Fut),
    Post(#[pin] Fut),
}
impl<Fut> Default for HookFut<Fut> {
    fn default() -> Self {
        Self::None
    }
}
#[pin_project]
#[derive(Debug)]
struct HookStream<'a, S, H, Fut> {
    #[pin]
    stream: S,
    #[pin]
    state: State<'a, H, Fut>,
}
#[pin_project(project = StateProj)]
#[derive(Debug)]
enum State<'a, H, Fut> {
    Init(Option<&'a mut H>),
    Wait(#[pin] Wait<'a, H, Fut>),
}
#[pin_project(project = WaitProj)]
#[derive(Debug)]
enum Wait<'a, H, Fut> {
    Pre(#[pin] Fut),
    Stream(Option<&'a mut H>),
    Post(#[pin] Fut),
}
impl<'a, S, H, Fut> HookStream<'a, S, H, Fut> {
    pub fn new(stream: S, hook: &'a mut H) -> Self {
        Self {
            stream,
            state: State::Init(Some(hook)),
        }
    }
}
impl<'a, S, H> Stream for HookStream<'a, S, H, BoxFuture<'a, (S::Item, &'a mut H)>>
where
    S: Stream,
    H: HookT<S::Item>,
{
    type Item = S::Item;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        let mut this = self.as_mut().project();
        Poll::Ready(loop {
            match this.state.as_mut().project() {
                StateProj::Init(ref mut h) => {
                    let fut = h.take().unwrap().pre();
                    this.state.set(State::Wait(Wait::Pre(fut)));
                }
                StateProj::Wait(ref mut wait) => match wait.as_mut().project() {
                    WaitProj::Pre(fut) => {
                        let (_res, h) = ready!(fut.poll(cx));
                        this.state.set(State::Wait(Wait::Stream(Some(h))));
                    }
                    WaitProj::Stream(ref mut h) => {
                        let res = ready!(this.stream.as_mut().poll_next(cx));
                        let fut = h.take().unwrap().post();
                        this.state.set(State::Wait(Wait::Post(fut)));
                    }
                    WaitProj::Post(fut) => {
                        let (res, h) = ready!(fut.poll(cx));
                        this.state.set(State::Init(Some(h)));
                        break Some(res);
                    }
                },
            }
        })
        // Poll::Ready(loop {
        //     let mut this = self.as_mut().project();
        //     let mut state = this.state.as_mut().project();
        //     // match this.state.as_ref().project() {
        //     match state {
        //         StateProj::Init(ref mut h) => {
        //             let fut = h.pre();
        //             // state.as_mut().as_pin_mut().set(State::None);
        //             // this.state.set(State::None);
        //             state.set(State::None);
        //             // *this.state = State::Wait(Wait::Pre(fut));
        //         },
        //         StateProj::Wait(ref wait) => {
        //             // match wait.project() {
        //             //     WaitProj::Pre(mut fut) => {
        //             //         let (h, _res) = ready!(fut.as_mut().poll(cx));
        //             //         // *this.state = State::Wait(Wait::Test(h));
        //             //         // back to top
        //             //     },
        //             //     WaitProj::Test(h) => {
        //             //         let res = ready!(this.stream.as_mut().poll_next(cx));
        //             //         let fut = h.post();
        //             //         // *this.state = State::Wait(Wait::Post(fut));
        //             //     },
        //             //     WaitProj::Post(mut fut) => {
        //             //         let (h, res) = ready!(fut.as_mut().poll(cx));
        //             //         // *this.state = State::Init(h);
        //             //         break Some(res)
        //             //     },
        //             // }
        //         },
        //         _ => todo!(),
        //     }
        // })
    }
}

impl<S, F, Fut> Wrap<S, F, Fut> {
    pub fn new(stream: S, hook: F) -> Self {
        Self {
            stream,
            fut: Default::default(),
            hook,
        }
    }
}
// TODO: turn Wrap into something that holds an H: Hook. change Fut to Option<HookFut<Fut>>
// so we can distinguish what we're waiting on.
// Want to take an async closure which, given a Test<'a, Args> and Args, produces a TestRes<'a>.
// Actually, next() gives me a Future<Output = TestRes>. Try just wrapping this first.
// Would a Fuzzing wrapper wrap tests.iter() and produce a stream where each test has multiple TestRes?
#[pin_project]
struct Wrap<S, F, Fut> {
    #[pin]
    stream: S,
    #[pin]
    fut: Option<Fut>,
    // fut: HookFut<Fut>,
    hook: F,
}
impl<St, F, Out> Stream for Wrap<St, F, BoxFuture<'_, Out>>
where
    St: Stream<Item = Out>,
    F: HookT<Out>,
    // Fut: Future<Output = TestRes<'a>>,
{
    type Item = St::Item;
    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        let mut this = self.project();
        // want to return when our future has resolved _and_ the stream has resolved, returning the stream result
        // let _ :() = this.fut.as_mut().as_pin_mut();
        Poll::Ready(loop {
            // match this.fut.as_mut().as_pin_mut() {
            match this.fut.as_mut().as_pin_mut() {
                None => {
                    // this.fut.set(Some(this.hook.pre()));
                    break None
                }
                _ => todo!()
                // HookFutProj::None => {
                //     // this.fut.set(HookFut::Pre(this.hook.pre()));
                //     // this.fut.set(HookFut::None);
                // },
                // HookFutProj::Pre(fut) => break None,
                // HookFutProj::Test(fut) => break None,
                // HookFutProj::Post(fut) => break None,
            }
        })

        // if we're waiting on a fut, poll it
        // if let Some(fut) = this.fut.as_mut().as_pin_mut() {
        //     println!("Polling Wrap's future.");
        //     let _f_res = ready!(fut.poll(cx));
        //     println!("Wrap's future resolved.");
        //     this.fut.set(None);
        //     *this.fut_resolved = true;
        // } else if *this.fut_resolved {
        //     println!("Polling Wrap's stream.");
        //     let s_res = ready!(this.stream.as_mut().poll_next(cx));
        //     *this.fut_resolved = false;
        //     println!("Wrap's stream resolved.");
        //     break s_res
        // } else {
        //     println!("Storing Wrap's future");
        //     // this.fut.set(Some((this.f)()));
        //     // this.fut.set(Some(this.hook.pre()));
        // }
    }
}

// Want to wrap something where next().await returns a Future<Output=TestRes<'a>>.
// pub struct Wrap<S, Fut>

// pub struct Tests<'a, Args, I, Fut> {
//     pub iter: &'a mut I,
//     pub args: Args,
//     pub fut: Option<Fut>,
//     // _tick: PhantomData<&'a u8>,
// }
// impl<'a, Args, I, Fut> Tests<'a, Args, I, Fut>

// where
//     Args: Unpin + Clone,
//     I: Iterator + Unpin,
//     I::Item: Test<'a, Args>,
// {
//     pub fn new(iter: &'a mut I, args: Args) -> Self {
//         Self {
//             iter,
//             args,
//             fut: None,
//             // _tick: PhantomData,
//         }
//     }
// }
// impl<'a, Args, I> Stream for Tests<'a, Args, I, BoxFuture<'a, TestRes<'a>>>
// // impl<'a, Args, I, Fut> Stream for Tests<'a, Args, I, Fut>
// where
//     Args: Unpin + Clone,
//     I: Iterator + Unpin,
//     I::Item: Test<'a, Args> + Clone + 'a,
//     // Fut: Future<Output = TestRes<'a>> + Unpin,
// {
//     type Item = TestRes<'a>;

//     fn poll_next(
//         self: std::pin::Pin<&mut Self>,
//         cx: &mut std::task::Context<'_>,
//     ) -> std::task::Poll<Option<Self::Item>> {
//         let me = self.get_mut();
//         match me.iter.next() {
//             Some(t) => {
//                 println!("getting res");
//                 // let res = ready!(t.run(me.args.clone()).poll_unpin(cx));
//                 let mut fut = std::sync::Arc::new(t).run(me.args.clone());
//                 match fut.poll_unpin(cx) {
//                     Poll::Ready(res) => return Poll::Ready(Some(res)),
//                     _ => {
//                         me.fut = Some(fut);
//                         return Poll::Pending
//                     },
//                 }
//             },
//             None => Poll::Ready(None),
//         }
//     }
// }
// impl<'a, Args, I, Fut> ExactSizeStream for Tests<'a, Args, I, Fut>
// where
//     Self: Stream,
//     I: ExactSizeIterator,
// {
//     fn len(&self) -> usize {
//         self.iter.len()
//     }
// }
