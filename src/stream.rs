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

use crate::{ty::{Succ, Zero, Pred, PredFn, FnTMut, FnTSync, TCons, TNil, TailFn, Tail, HeadFn, Head}, core::Ctx};

pub type AsyncOutput<Y> = Pin<Box<dyn Future<Output = Y> + Send>>;
pub type AsyncFn<X, Y> = dyn Fn(X) -> AsyncOutput<Y> + Send + Sync;

// pub trait Runner: Stream<Item = TestResult> {}
pub trait TestSet<'a> {
    fn tests() -> &'a [&'a dyn Test<'a, Self>];
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
        // If the hook failed, propogate it as the test result
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
    Ctx: Clone,
{
    type Item = TestRes<'a>;
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let next = self.tests.next();
        if let Some(t) = next {
            let res = ready!(t.run(self.ctx.clone()).poll_unpin(cx));
            Poll::Ready(Some(res))
        } else {
            Poll::Ready(None)
        }
    }
}

async fn runrun<T, Args>(args: Args) -> ()
where
    T: Ctx<Base = Args> + TestSet<'static> + Unpin + Clone + Send + Sync + 'static,
    Args: Send + 'static
{
    let ctx = T::build(args).await;
    let tests = T::tests();
    let mut stream = TestStream::new(tests.iter(), ctx);
    while let Some(t) = stream.next().await {
        println!("test: {:?}", t.trace);
    }
}
#[derive(Debug, Clone)]
struct RunrunBuilder;
impl<T, Args> FnTSync<T, ()> for RunrunBuilder
where
    T: Ctx<Base = Args> + TestSet<'static> + Unpin + Clone + Send + Sync + 'static,
    Args: Send + 'static
{
    type Out = &'static AsyncFn<Args, ()>;
    // Note that the builder args are different than the args passed to the generated fn
    fn call(&self, _bargs: ()) -> Self::Out {
        &|args| Box::pin(runrun::<T, Args>(args))
    }
}
pub type App<F, Args> = <F as BuilderT<Args>>::Out;
pub trait BuilderT<Args> {
    type Out;
    fn build<T>(&self, args: Args) -> Self::Out;
    // type Out<T>;
    // fn build<T>(args: Args) -> Self::Out<T>;
}
#[derive(Clone)]
pub struct FnBuilder<'a, X, Y> {
    f: &'a AsyncFn<X, Y>,
}
// impl<B, T, Args> FnTSync<T, Args> for B
// where
//     B: BuilderT<Args>
// {
//     type Out = B;
//     fn call(&self, args: Args) -> Self::Out {
//         B::build::<T>(args)
//     }
// }


#[derive(Debug, Clone, Copy)]
pub enum Status {
    Pass,
    Fail,
    Skip,
}
#[derive(Debug, Clone)]
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
    async fn run(&self, args: Args) -> TestRes<'a>;
    fn skip(&self) -> TestRes<'a> {
        TestRes::default()
    }
}
#[async_trait]
impl<'a, T, E, Args> Test<'a, Args> for Result<T, E>
where
    T: Test<'a, Args> + Send + Sync,
    E: Into<&'a dyn Debug> + Clone + Send + Sync + 'a,
    Args: Send + Sync + 'static,
{
    async fn run(&self, args: Args) -> TestRes<'a> {
        match self {
            Ok(r) => r.run(args).await,
            Err(e) => TestRes {
                status: Status::Fail,
                trace: (*e).clone().into(),
            },
        }
    }
}
#[async_trait]
impl<'a, T, Args> Test<'a, Args> for &T
where
    T: Test<'a, Args> + ?Sized + Send + Sync,
    Args: Send + Sync + 'static,
{
    async fn run(&self, args: Args) -> TestRes<'a> {
        (**self).run(args).await
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

// If I build my own iterator (stream) type, then can I map a TList into
// it without evaluating the Ctx::build() methods?

type Len<T> = <T as TList>::Len;
pub trait TList {
    type Len;
    const LEN: usize;
}
impl<H, T> TList for TCons<H, T>
where
    T: TList,
{
    type Len = Succ<<T as TList>::Len>;
    const LEN: usize = 1 + <T as TList>::LEN;
}
// Undefined for TNil
type Nth<N, T> = <T as NthFn<N>>::Out;
pub trait NthFn<N> {
    type Out;
}
impl<H, T> NthFn<Zero> for TCons<H, T> {
    type Out = H;
}
impl<N, H, T> NthFn<Succ<N>> for TCons<H, T> where T: NthFn<N> {
    type Out = Nth<N, T>;
}

// Instead of "storing" the next elem in generics of the fn, we store
// it in generics of the trait, giving us more flexibility in type-level logic
// pub type NthMap<N, F, T> = <T as NthMapFn<F, N>>::Out;
// impld on the list? impld on the starting list, for defining the type of the mutable IterT arg
// Lst is the current running list
// pub type MapStore<F, Args, Lst, T> = <T as MapStoreFn<F, Args, Lst>>::Out;
pub trait MapStoreFn<F, Args, Lst>
where
    Self: HeadFn,
    F: BuilderT<Args>,

{
    fn map_store(it: &mut IterT<F, Args, Self>) -> Option<App<F, Args>>; // f is aka builder
}
// Self is the original list
impl<F, Args, H, T> MapStoreFn<F, Args, TNil> for TCons<H, T>
where
    // Iter: Iterator<Item = F::Out<H>>,
    F: BuilderT<Args>
{
    // type Ret = F::Out<Self>;
    fn map_store(it: &mut IterT<F, Args, Self>) -> Option<App<F, Args>> {
        None
    }
}
impl<F, Args, H, T, S> MapStoreFn<F, Args, TCons<H, T>> for S
where
    // Lst: TailFn,
    F: BuilderT<Args>,
    // S: MapStoreFn<F, Args, T> + HeadFn<Out = H>, // TODO: we def don't want this
    S: MapStoreFn<F, Args, T>,
    Args: Clone,
{
    // type Ret = F::Out<Self>;
    fn map_store(it: &mut IterT<F, Args, Self>) -> Option<App<F, Args>> {
        it.find = <Self as MapStoreFn<F, Args, T>>::map_store;
        Some(F::build::<H>(&it.f, it.args.clone()))
    }
}
// impl<N, H, T> NthFn<N> for TCons<H, T>
// where
//     N: PredFn,
//     T: NthFn<Pred<N>>,
// {
//     type Out = Nth<Pred<N>, T>;
// }
// impl<N, Lst> NthFn<Pred<N>> for Lst
// where
//     N: PredFn,
//     Lst: NthFn<N>
// {
//     type Out = usize;
// }

// For all n, for all 0 <= x < n, if n is a valid index into a list, so is x

// If I'm Valid, then my tail is valid
// How to assert that every valid list eventually ends in TNil?
pub trait Valid {}
impl Valid for TNil {}
impl<H, T> Valid for TCons<H, T> where T: Valid {}


#[marker] pub trait InBounds<N> {}
impl<N, H, T> InBounds<N> for TCons<H, T>
where
    N: PredFn,
    T: InBounds<Pred<N>>
{}
impl<H, T> InBounds<Zero> for TCons<H, T> {}
impl<H, T> InBounds<Succ<Zero>> for TCons<H, T> {}



impl TList for TNil {
    type Len = Zero;
    const LEN: usize = 0;
}
pub trait Idx {
    type Out;
}
pub struct IterT<F, Args, Lst> where Self: Iterator, Lst: ?Sized {
    pub f: F,
    pub find: fn(&mut Self) -> Option<<Self as Iterator>::Item>,
    pub args: Args,
}
impl<F, Args, LST> IterT<F, Args, LST>
where
    Self: Iterator,
    F: BuilderT<Args>,
{
    // we store an associated fn with the entire list,
    // then next time through store an associated fn with the tail of that list
    pub fn find<N, Lst>(&mut self) -> Option<<Self as Iterator>::Item>
    where
        N: PredFn,
        Lst: NthFn<N> + TailFn + Valid,
        // Tail<Lst>: NthFn<Pred<N>> + TailFn + Valid,
    {
        let item = (self.find)(self);
        // self.find = IterT::find::<Succ<N>>;
        // let _ = IterT::find::<Pred<N>, Tail<Lst>>;
        // let x: Nth<Pred<Pred<N>>, Lst> = ();
        // self.f.build::<Nth<N, Self>>();
        // Nth should return TNil for N > Len, then impl BuilderT for TNil to return None
        // <F as BuilderT<Args>>::build::<Nth<N, Lst>>(&self.f, self.args)();
        todo!()
    }
}
impl<F, Args, Lst> Iterator for IterT<F, Args, Lst>
    where
    F: BuilderT<Args>,
    Lst: HeadFn + ?Sized,
{
    type Item = F::Out;
    fn next(&mut self) -> Option<Self::Item> {
        // <F as BuilderT<Args>>::build::<Nth>(self.args);
        // self.find::<usize>();
        todo!()
    }
}
// const fn len<Lst>() -> usize {
// }
// What if the iterator stored a fn that took a generic param and returned itself
// with Succ<Param>?
// struct TMap<F, Lst> {
//     f: F, // builder
// }
// pub trait BuildIter<B, Args> {
//     fn build_iter(builder: B, args: Args);
// }

// Need to combine MapFn and IntoIter impls to go from TList (types only) -> iterator
// via mapping a function that acts on types + args. Should probably just impl
// my own iterator, then a map from F, TList into that iterator.
pub trait MapToIter<F, Args> {
    type Item;
    type IntoIter: Iterator;
    fn map_to_iter(f: F, args: Args) -> Self::IntoIter;
}
impl<F, Args, H, T> MapToIter<F, Args> for TCons<H, T>
where
    F: FnTSync<H, Args> + FnTSync<Head<T>, Args>,
    T: MapToIter<F, Args, Item = <F as FnTSync<H, Args>>::Out> + HeadFn,
    <T as MapToIter<F, Args>>::IntoIter: Iterator<Item = <F as FnTSync<H, Args>>::Out>,
    Args: Send + Sync + Clone + 'static,
{
    type Item = <F as FnTSync<H, Args>>::Out;
    type IntoIter =
        iter::Chain<iter::Once<Self::Item>, <T as MapToIter<F, Args>>::IntoIter>;
    fn map_to_iter(f: F, args: Args) -> Self::IntoIter {
        let head = <F as FnTSync<H, Args>>::call(&f, args.clone());
        let rest = <T as MapToIter<F, Args>>::map_to_iter(f, args);
        iter::once(head).chain(rest)
    }
}
impl <F, Args, H> MapToIter<F, Args> for TCons<H, TNil>
where
    F: FnTSync<H, Args>,
    Args: Send + Sync + Clone + 'static,
{
    type Item = <F as FnTSync<H, Args>>::Out;
    type IntoIter = iter::Once<Self::Item>;
    fn map_to_iter(f: F, args: Args) -> Self::IntoIter {
        let head = <F as FnTSync<H, Args>>::call(&f, args.clone());
        iter::once(head)
    }
}
#[cfg(test)]
mod test {
    use super::*;
    use anyhow::Result;
    use linkme::distributed_slice;
    use crate::{TList};

    #[derive(Debug, Clone)]
    pub struct NullCtx;
    #[derive(Debug, Clone)]
    pub struct Ctx0;
    #[derive(Debug, Clone)]
    pub struct Ctx1;
    #[async_trait]
    impl Ctx for Ctx0 { type Base = NullCtx; async fn build(_: Self::Base) -> Self { println!("building 0"); Self } }
    #[async_trait]
    impl Ctx for Ctx1 { type Base = NullCtx; async fn build(_: Self::Base) -> Self { println!("building 1"); Self } }

    #[derive(Debug, Clone)]
    struct UnitTest<F: Fn(Args) -> AsyncOutput<Out> + Send + Sync, Args: Clone + Send + Sync, Out: Send + Sync>(F, PhantomData<(Args, Out)>);
    #[async_trait]
    impl<'a, F, Args, Out> Test<'a, Args> for UnitTest<F, Args, Out>
    where
        F: Fn(Args) -> AsyncOutput<Out> + Send + Sync,
        Args: Clone + Send + Sync,
        Out: Send + Sync,
    {
        async fn run(&self, args: Args) -> TestRes<'a> {
            (self.0)(args.clone());
            TestRes::default()
        }
    }
    #[derive(Clone)]
    pub struct TestCase<'a, T> {
        pub name: &'static str,
        pub test: &'a AsyncFn<T, Result<()>>,
    }
    #[async_trait]
    impl<'a, Args> Test<'a, Args> for TestCase<'_, Args>
    where
        Args: Send + Sync + 'static,
    {
        async fn run(&self, args: Args) -> TestRes<'a> {
            println!("{}", self.name);
            (self.test)(args).await;
            TestRes::default()
        }
    }
    async fn test_01(_: Ctx0) -> Result<()> { println!("running test_01"); Ok(()) }
    async fn test_11(_: Ctx1) -> Result<()> { println!("running test_11"); Ok(()) }

    #[distributed_slice]
    pub static TESTS_ON_CTX0: [&'static dyn Test<'static, Ctx0>] = [..];
    #[distributed_slice(TESTS_ON_CTX0)]
    pub static __T01: &dyn Test<Ctx0> = &TestCase { name: "test_01", test: &|x| Box::pin(test_01(x)) };
    #[distributed_slice]
    pub static TESTS_ON_CTX1: [&'static dyn Test<'static, Ctx1>] = [..];
    #[distributed_slice(TESTS_ON_CTX1)]
    pub static __T11: &dyn Test<Ctx1> = &TestCase { name: "test_11", test: &|x| Box::pin(test_11(x)) };

    impl TestSet<'static> for Ctx0 {
        fn tests() -> &'static [&'static dyn Test<'static, Self>] {
            let _ = &UnitTest( |x| Box::pin(test_01(x)), PhantomData );
            &TESTS_ON_CTX0
        }
    }
    impl TestSet<'static> for Ctx1 {
        fn tests() -> &'static [&'static dyn Test<'static, Self>] {
            &TESTS_ON_CTX1
        }
    }

    fn noop() {}
    struct Unit;
    impl<T, Args> FnTSync<T, Args> for Unit {
        type Out = ();
        fn call(&self, args: Args) -> Self::Out { }
    }

    // Can't just map the driver, need to map into a function pointer with a provided generic
    #[tokio::test]
    async fn test() {
        type Ctxs = TList!(Ctx0, Ctx1);
        // let x: Nth<Succ<Succ<Zero>>, Ctxs> = ();
        let null = NullCtx;
        let mut iter = <Ctxs as MapToIter<_, ()>>::map_to_iter(RunrunBuilder, ());
        for f in iter {
            f(null.clone()).await;
        }
    }
}
