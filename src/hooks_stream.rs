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
pub struct HookRunner<H> {
    hook: H,
}
impl<H> HookRunner<H> {
    pub fn new(hook: H) -> Self {
        Self { hook }
    }
}

// run() -> Stream<Item = TestRes>
// skip() -> Stream<Item = TestRes>
// A Driver aggregates tests.
// A Runner turns an iterator of tests into a stream of test reseults (running hooks if needed).
//  but then how to run hooks before a batch of tests?
//  so, A Run/Crank/Block implements Stream and Skip
//  I kind of want everything to return a stream of TestRes, but I want to decide
//  at each level whether that is from skip() or run()
// A Stat takes a stream of test results and agregates them into some report.
#[async_trait]
impl<Args, H> FnT<Args> for HookRunner<H>
where
    Args: Send + 'static,
    H: Hook<TestRes<'static>> + Unpin + Clone + Send + Sync,
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

        let mut inner = BaseRunner::new(tests.iter());
        let mut runner = HookRunner2 {
            inner,
            hook: self.hook.clone(),
        };
        let mut pass = 0;
        let mut fail = 0;
        let mut skip = 0;
        while let Some(res) = runner.next(Mode::Run, ctx.clone()).await {
            match res.status {
                Status::Pass => pass += 1,
                Status::Fail => fail += 1,
                Status::Skip => skip += 1,
            }
        }
        println!("tests passed : {}", pass);
        println!("tests failed : {}", fail);
        println!("tests skipped: {}", skip);

        let child_iter = MapT::new::<ChildTypes<T>>(self, ctx.clone());
        let mut child_stream = stream::iter(child_iter);
        while let Some(_) = child_stream.next().await {}
    }
}

use crate::core_stream::{Runner, ExactSize, Mode, BaseRunner};
struct HookRunner2<I, H> {
    inner: I,
    hook: H,
}
#[async_trait]
impl<'a, I, H, In> Runner<In, TestRes<'a>> for HookRunner2<I, H>
where
    I: Runner<In, TestRes<'a>> + ExactSize + Send,
    H: Hook<TestRes<'a>> + Send,
    In: Send + 'static,
{
    async fn next(&mut self, mode: Mode, args: In) -> Option<TestRes<'a>> {
        if self.inner.is_empty() {
            return self.inner.next(mode, args).await;
        }
        let pre = self.hook.pre().await;
        if pre.status.is_fail() {
            return Some(pre);
        }
        let test = self.inner.next(mode, args).await.unwrap();
        let post = self.hook.post().await;
        if !test.status.is_fail() && post.status.is_fail() {
            Some(post)
        } else {
            Some(test)
        }
    }
}
impl<I, H> ExactSize for HookRunner2<I, H>
where
    I: ExactSize,
{
    fn len(&self) -> usize {
        self.inner.len()
    }
}

// struct Skip;
// struct Run;
// #[pin_project]
// struct HookRunner1<'a, I, Args, H> {
//     iter: I,
//     args: Args,
//     skip: bool,
//     #[pin]
//     state: State1<'a, H>,
// }
// #[pin_project(project = StateProj1)]
// enum State1<'a, H> {
//     Init(Once<&'a mut H>),
//     Wait(#[pin] Wait1<'a, H>),
// }
// enum Once<T> {
//     Just(T),
//     Unreachable,
// }
// impl<T> Once<T> {
//     pub fn take(&mut self) -> T {
//         std::mem::replace(self, Self::Unreachable)
//             .expect("Attempted to take a Once::Unreachable. This is bad.")
//     }
//     pub fn expect(self, msg: &str) -> T {
//         match self {
//             Self::Just(val) => val,
//             Self::Unreachable => panic!("{}", msg),
//         }
//     }
// }
// #[pin_project(project = WaitProj1)]
// enum Wait1<'a, H> {
//     Pre(#[pin] BoxFuture<'a, (TestRes<'a>, &'a mut H)>),
//     Test(#[pin] BoxFuture<'a, TestRes<'a>>, Once<&'a mut H>),
//     Post(
//         #[pin] BoxFuture<'a, (TestRes<'a>, &'a mut H)>,
//         Once<TestRes<'a>>,
//     ),
// }
// impl<'a, I, Args, H> Stream for HookRunner1<'a, I, Args, H>
// where
//     I: Iterator<Item = &'a dyn Test<'a, Args>>,
//     H: HookT<TestRes<'a>>,
//     Args: Clone + 'a,
// {
//     type Item = TestRes<'a>;
//     fn poll_next(
//         mut self: std::pin::Pin<&mut Self>,
//         cx: &mut std::task::Context<'_>,
//     ) -> Poll<Option<Self::Item>> {
//         let test = match self.iter.next() {
//             Some(next) => next,
//             None => return Poll::Ready(None),
//         };
//         if self.skip {
//             return Poll::Ready(Some(test.skip()));
//         }

//         let mut this = self.as_mut().project();
//         Poll::Ready(loop {
//             match this.state.as_mut().project() {
//                 StateProj1::Init(ref mut h) => {
//                     let fut = h.take().pre();
//                     this.state.set(State1::Wait(Wait1::Pre(fut)));
//                 }
//                 StateProj1::Wait(ref mut wait) => {
//                     match wait.as_mut().project() {
//                         WaitProj1::Pre(fut) => {
//                             let (res, h) = ready!(fut.poll(cx));
//                             if res.status.is_fail() {
//                                 let _skip = test.skip();
//                                 break Some(res);
//                             }
//                             let run = test.run(this.args.clone());
//                             this.state
//                                 .set(State1::Wait(Wait1::Test(run, Once::Just(h))));
//                         }
//                         WaitProj1::Test(fut, ref mut h) => {
//                             let res = ready!(fut.poll(cx));
//                             let post = h.take().post();
//                             this.state
//                                 .set(State1::Wait(Wait1::Post(post, Once::Just(res))));
//                         }
//                         WaitProj1::Post(fut, test_res) => {
//                             let (post_res, h) = ready!(fut.poll(cx));
//                             // Setting state before touching test_res angers the borrow checker
//                             let test_res = test_res.take();
//                             this.state.set(State1::Init(Once::Just(h)));
//                             if !test_res.status.is_fail() && post_res.status.is_fail() {
//                                 break Some(post_res);
//                             } else {
//                                 break Some(test_res);
//                             }
//                         }
//                     }
//                 }
//             }
//         })
//     }
// }

// #[async_trait]
// pub trait Block<Args, Res> {
//     async fn runs(&mut self, args: Args) -> &dyn Stream<Item = Res>;
//     async fn skips(&mut self, args: Args) -> &dyn Stream<Item = Res>;
// }

// #[async_trait]
// pub trait HookT<T> {
//     async fn pre(&mut self) -> (T, &mut Self);
//     async fn post(&mut self) -> (T, &mut Self);
// }

// #[pin_project(project = HookFutProj)]
// enum HookFut<Fut> {
//     None,
//     Pre(#[pin] Fut),
//     Test(#[pin] Fut),
//     Post(#[pin] Fut),
// }
// impl<Fut> Default for HookFut<Fut> {
//     fn default() -> Self {
//         Self::None
//     }
// }
// #[pin_project]
// #[derive(Debug)]
// struct HookStream<'a, S, H, Fut> {
//     #[pin]
//     stream: S,
//     #[pin]
//     state: State<'a, H, Fut>,
// }
// #[pin_project(project = StateProj)]
// #[derive(Debug)]
// enum State<'a, H, Fut> {
//     Init(Option<&'a mut H>),
//     Wait(#[pin] Wait<'a, H, Fut>),
// }
// #[pin_project(project = WaitProj)]
// #[derive(Debug)]
// enum Wait<'a, H, Fut> {
//     Pre(#[pin] Fut),
//     Stream(Option<&'a mut H>),
//     Post(#[pin] Fut),
// }
// impl<'a, S, H, Fut> HookStream<'a, S, H, Fut> {
//     pub fn new(stream: S, hook: &'a mut H) -> Self {
//         Self {
//             stream,
//             state: State::Init(Some(hook)),
//         }
//     }
// }
// impl<'a, S, H> Stream for HookStream<'a, S, H, BoxFuture<'a, (S::Item, &'a mut H)>>
// where
//     S: Stream,
//     H: HookT<S::Item>,
// {
//     type Item = S::Item;

//     fn poll_next(
//         mut self: std::pin::Pin<&mut Self>,
//         cx: &mut std::task::Context<'_>,
//     ) -> Poll<Option<Self::Item>> {
//         let mut this = self.as_mut().project();
//         Poll::Ready(loop {
//             match this.state.as_mut().project() {
//                 StateProj::Init(ref mut h) => {
//                     let fut = h.take().unwrap().pre();
//                     this.state.set(State::Wait(Wait::Pre(fut)));
//                 }
//                 StateProj::Wait(ref mut wait) => match wait.as_mut().project() {
//                     WaitProj::Pre(fut) => {
//                         let (_res, h) = ready!(fut.poll(cx));
//                         this.state.set(State::Wait(Wait::Stream(Some(h))));
//                     }
//                     WaitProj::Stream(ref mut h) => {
//                         let res = ready!(this.stream.as_mut().poll_next(cx));
//                         let fut = h.take().unwrap().post();
//                         this.state.set(State::Wait(Wait::Post(fut)));
//                     }
//                     WaitProj::Post(fut) => {
//                         let (res, h) = ready!(fut.poll(cx));
//                         this.state.set(State::Init(Some(h)));
//                         break Some(res);
//                     }
//                 },
//             }
//         })
//     }
// }

// impl<S, F, Fut> Wrap<S, F, Fut> {
//     pub fn new(stream: S, hook: F) -> Self {
//         Self {
//             stream,
//             fut: Default::default(),
//             hook,
//         }
//     }
// }
// // TODO: turn Wrap into something that holds an H: Hook. change Fut to Option<HookFut<Fut>>
// // so we can distinguish what we're waiting on.
// // Want to take an async closure which, given a Test<'a, Args> and Args, produces a TestRes<'a>.
// // Actually, next() gives me a Future<Output = TestRes>. Try just wrapping this first.
// // Would a Fuzzing wrapper wrap tests.iter() and produce a stream where each test has multiple TestRes?
// #[pin_project]
// struct Wrap<S, F, Fut> {
//     #[pin]
//     stream: S,
//     #[pin]
//     fut: Option<Fut>,
//     // fut: HookFut<Fut>,
//     hook: F,
// }
// impl<St, F, Out> Stream for Wrap<St, F, BoxFuture<'_, Out>>
// where
//     St: Stream<Item = Out>,
//     F: HookT<Out>,
//     // Fut: Future<Output = TestRes<'a>>,
// {
//     type Item = St::Item;
//     fn poll_next(
//         self: std::pin::Pin<&mut Self>,
//         cx: &mut std::task::Context<'_>,
//     ) -> Poll<Option<Self::Item>> {
//         let mut this = self.project();
//         // want to return when our future has resolved _and_ the stream has resolved, returning the stream result
//         // let _ :() = this.fut.as_mut().as_pin_mut();
//         Poll::Ready(loop {
//             // match this.fut.as_mut().as_pin_mut() {
//             match this.fut.as_mut().as_pin_mut() {
//                 None => {
//                     // this.fut.set(Some(this.hook.pre()));
//                     break None
//                 }
//                 _ => todo!()
//                 // HookFutProj::None => {
//                 //     // this.fut.set(HookFut::Pre(this.hook.pre()));
//                 //     // this.fut.set(HookFut::None);
//                 // },
//                 // HookFutProj::Pre(fut) => break None,
//                 // HookFutProj::Test(fut) => break None,
//                 // HookFutProj::Post(fut) => break None,
//             }
//         })

//         // if we're waiting on a fut, poll it
//         // if let Some(fut) = this.fut.as_mut().as_pin_mut() {
//         //     println!("Polling Wrap's future.");
//         //     let _f_res = ready!(fut.poll(cx));
//         //     println!("Wrap's future resolved.");
//         //     this.fut.set(None);
//         //     *this.fut_resolved = true;
//         // } else if *this.fut_resolved {
//         //     println!("Polling Wrap's stream.");
//         //     let s_res = ready!(this.stream.as_mut().poll_next(cx));
//         //     *this.fut_resolved = false;
//         //     println!("Wrap's stream resolved.");
//         //     break s_res
//         // } else {
//         //     println!("Storing Wrap's future");
//         //     // this.fut.set(Some((this.f)()));
//         //     // this.fut.set(Some(this.hook.pre()));
//         // }
//     }
// }

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
// #[async_trait]
// impl<T> HookT<T> for NoHook
// where
//     T: Default,
// {
//     async fn pre(&mut self) -> (T, &mut Self) {
//         println!("Running NoHook pre.");
//         tokio::time::sleep(std::time::Duration::from_secs(1)).await;
//         (Default::default(), self)
//     }
//     async fn post(&mut self) -> (T, &mut Self) {
//         println!("Running NoHook post.");
//         tokio::time::sleep(std::time::Duration::from_secs(1)).await;
//         (Default::default(), self)
//     }
// }

// pub async fn run_test<'a, T, Args, H>(t: T, args: Args, mut hook: H) -> TestRes<'a>
// where
//     H: Hook<'a>,
//     T: Test<'a, Args>,
// {
//     // let (_, pre_res) = hook.pre().await;
//     let pre_res = hook.pre().await;
//     // If the pre hook failed, return it as the test result, skipping test and post hook
//     if let Status::Fail = pre_res.status {
//         return pre_res;
//     }
//     let test_res = t.run(args).await;
//     // If the test failed, return the result and don't run the post hook
//     if let Status::Fail = test_res.status {
//         return test_res;
//     }
//     let post_res = hook.post().await;
//     // If the test passed but the post hook failed, return the post hook failure
//     if let Status::Fail = post_res.status {
//         return post_res;
//     }
//     // Everything passed. Return the test result
//     test_res
// }
