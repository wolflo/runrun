use async_trait::async_trait;
use futures::{
    ready, stream,
    stream::{Stream, StreamExt},
    Future, FutureExt,
};
use std::{
    iter,
    sync::Arc,
    fmt::Debug,
    marker::PhantomData,
    pin::Pin,
    task::{Context, Poll},
};

use crate::{ty::{FnTMut, FnTSync, TMap, TCons, TNil, TailFn}, core::Ctx};

pub type AsyncOutput<Y> = Pin<Box<dyn Future<Output = Y> + Send>>;
pub type AsyncFn<X, Y> = dyn Fn(X) -> AsyncOutput<Y> + Send + Sync;

// pub trait Runner: Stream<Item = TestResult> {}
pub trait TestSet<'a> {
    // fn tests() -> &'a [&'a dyn Test<'a, Self>];
    fn tests() -> &'a [Box<dyn Test<'a, Self>>];
}

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
    F: TestMut<'a>,
    Ctx: 'a,
{
    type Item = S::Item;
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let hook_res = ready!(self.hook.run().poll_unpin(cx));
        // If the hook failed propogate it as the test result
        if matches!(hook_res.status, Status::Fail) {
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
        if matches!(hook_res.status, Status::Fail) {
            // test passed or skipped, but hook failed. Propogate hook result
            Poll::Ready(Some(hook_res))
        } else {
            Poll::Ready(test_res)
        }
    }
}

// TestStream takes an iterator over tests and returns a running stream
// Filters will act on the input iterator before the TestStream turns
// it into a Stream
struct TestStream<'a, I, Ctx> {
    tests: I,
    ctx: Ctx,
    _tick: PhantomData<&'a I>,
}
impl<'a, T, I, Ctx> TestStream<'a, I, Ctx>
where
    Self: Unpin,
    I: Iterator<Item = T> + 'a,
    T: Test<'a, Ctx> + Unpin,
{
    pub fn new(tests: I, ctx: Ctx) -> Self {
        Self {
            tests,
            ctx,
            _tick: PhantomData,
        }
    }
}
impl<'a, I, T, Ctx> Stream for TestStream<'a, I, Ctx>
where
    Self: Unpin,
    I: Iterator<Item = T> + 'a,
    T: Test<'a, Ctx> + Unpin + 'static,
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

// A DriverBuilder will impl FnTSync and map a Ctx type to a Driver of that test set.
// A Driver will impl FnTMut, build the Ctx, turn the tests into a TestStream,
// wrap that stream as needed (e.g. hooks), then consume the stream
struct Driver;
impl Driver {
    pub fn new() -> Self { Self }
}
#[async_trait]
impl<'a, T, Args> FnTMut<T, Args> for Driver
where
    T: Ctx<Base = Args> + TestSet<'static> + Unpin + Clone + Send + Sync + 'static,
    Args: Send + 'static,
{
    type Out = ();
    async fn call(&mut self, args: Args) -> Self::Out {
        let ctx = T::build(args).await;
        let tests = T::tests();
        let mut stream = TestStream::new(tests.iter(), ctx);
        while let Some(t) = stream.next().await {
            println!("test: {:?}", t.trace);
        }
    }
}
struct DriverBuilder;
impl<T> FnTSync<T, ()> for DriverBuilder {
    type Out = Driver;
    fn call(&self, _args: ()) -> Self::Out {
        Driver::new()
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
    pub status: Status,
    pub trace: &'a dyn Debug,
}
#[async_trait]
pub trait TestMut<'a> {
    async fn run(&mut self) -> TestRes<'a>;
    fn skip(&self) -> TestRes<'a> {
        TestRes::default()
    }
}
#[async_trait]
pub trait Test<'a, Args>: Send + Sync {
    async fn run(self, args: &Args) -> TestRes<'a>;
    fn skip(&self) -> TestRes<'a> {
        TestRes::default()
    }
}
#[async_trait]
impl<'a, T, E, Args> Test<'a, Args> for Result<T, E>
where
    T: Test<'a, Args> + Send + Sync,
    E: Into<&'a dyn Debug> + Send + Sync + 'a,
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
#[async_trait]
impl<'a, T, Args> Test<'a, Args> for &Box<T>
where
    T: Test<'a, Args> + ?Sized + Send + Sync,
    Args: Sync,
{
    async fn run(self, args: &Args) -> TestRes<'a> {
        self.clone().run(args).await
    }
    fn skip(&self) -> TestRes<'a> {
        (**self).skip()
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

// #[async_trait]
// impl<'a, T, Args> Test<'a, Args> for &T
// where
//     T: Test<'a, Args> + ?Sized + Send + Sync,
//     Args: Sync,
// {
//     async fn run(self, args: &Args) -> TestRes<'a> {
//         self.run(args).await
//     }
// }
// pub trait Sluice<I, Ctx> {
//     fn new(iter: I, ctx: Ctx) -> Self;
// }
// pub struct Driver2<S> {
//     pub stream: S,
// }
// impl<'a, S> Driver2<S>
// where
//     S: Stream<Item = TestRes<'a>>,
//     // S: Sluice,
// {
//     async fn run_ctx<C>(&mut self, ctx: C)
//     where
//         C: TestSet<'a> + Unpin + Sync + 'static,
//     {
//         let tests = C::tests();
//         let stream = Streamer::new(tests.iter(), ctx);
//         // let stream = S::build(tests.iter(), ctx);
//         let res: Vec<_> = stream.collect().await;
//     }
// }
// Need something that given an initial state will generate a stream of streams of tests
// struct Driver<'a, TList> {
//     _tick: PhantomData<&'a TList>,
// }
// impl<'a, S> Stream for Driver<'a, S>
// where
//     Self: Unpin,
// {
//     type Item = &'a dyn Stream<Item = TestRes<'a>>;
//     fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
//         // Take1<>;
//         Poll::Pending
//     }
// }

// Need to combine MapFn and IntoIter impls to go from TList (types only) -> iterator
// via mapping a function that acts on types + args
pub trait TIter {
    type Item;
    type IntoIter: Iterator;
    fn into_iter(self) -> Self::IntoIter;
}
impl<F, Args, H, T> TIter for TMap<F, Args, TCons<H, T>>
where
    F: FnTSync<H, Args>,
    T: TailFn,
    TMap<F, Args, T>: IntoIterator<Item = F::Out>,
    Args: Clone,
{
    type Item = F::Out;
    type IntoIter =
        iter::Chain<iter::Once<Self::Item>, <TMap<F, Args, T> as IntoIterator>::IntoIter>;
    fn into_iter(self) -> Self::IntoIter {
        let node = <F as FnTSync<H, Args>>::call(&self.f, self.args.clone());
        let rest = TMap {
            lst: self.lst.tail(),
            f: self.f,
            args: self.args,
        };
        iter::once(node).chain(rest.into_iter())
    }
}
#[cfg(test)]
mod test {
    use super::*;
    use crate::{TList, tlist};

    struct Ctx0;
    struct Ctx1;
    #[async_trait]
    impl Ctx for Ctx0 { type Base = (); async fn build(_: Self::Base) -> Self { Self } }
    #[async_trait]
    impl Ctx for Ctx1 { type Base = Ctx0; async fn build(_: Self::Base) -> Self { Self } }
    impl<'a> TestSet<'a> for Ctx0 {
        fn tests() -> &'a [Box<dyn Test<'a, Self>>] {
            &[]
        }
    }
    impl<'a> TestSet<'a> for Ctx1 {
        fn tests() -> &'a [Box<dyn Test<'a, Self>>] {
            &[]
        }
    }


    #[tokio::test]
    async fn test() {
        // type Ctxs = tlist!(Ctx0, Ctx1);
        let init_ctx = Ctx0::build(()).await;
    }
}
