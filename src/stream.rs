use async_trait::async_trait;
use futures::{
    ready, stream,
    stream::{Stream, StreamExt},
    Future, FutureExt,
};
use std::{
    fmt::Debug,
    iter,
    marker::PhantomData,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

use crate::{
    core::Ctx,
    ty::{
        ChildTypes, ChildTypesFn, FnTMut, FnTSync, Head, HeadFn, Pred, PredFn, Succ, TCons, TNil,
        Tail, TailFn, Zero,
    },
};

pub type AsyncOutput<Y> = Pin<Box<dyn Future<Output = Y> + Send>>;
pub type AsyncOutputTick<'fut, Y> = Pin<Box<dyn Future<Output = Y> + Send + 'fut>>;
pub type AsyncFn<X, Y> = dyn Fn(X) -> AsyncOutput<Y> + Send + Sync;

// pub trait Runner: Stream<Item = TestResult> {}
pub trait TestSet<'a> {
    fn tests() -> &'a [&'a dyn Test<'a, Self>];
}

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

// TestStream takes an iterator over tests and returns a running stream
// Filters can act on the input iterator before the TestStream turns
// it into a Stream.
// TODO: split this into a starter and a runner to make it easier to
// go between them?
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

pub type ApplySync<F, Args> = <F as BuilderTSync<Args>>::Out;
pub trait BuilderTSync<Args> {
    type Out;
    fn build<T>(&self, args: Args) -> Self::Out;
}
pub type Apply<F, Args> = <F as BuilderT<Args>>::Out;
pub type ApplyFut<'a, F, Args> = AsyncOutputTick<'a, <F as BuilderT<Args>>::Out>;

pub trait MapBounds<Args>:
    Ctx<Base = Args> + TestSet<'static> + ChildTypesFn + Unpin + Clone + Send + Sync + 'static
{
}
impl<T, Args> MapBounds<Args> for T where
    T: Ctx<Base = Args> + TestSet<'static> + ChildTypesFn + Unpin + Clone + Send + Sync + 'static
{
}

#[async_trait]
pub trait BuilderT<Args>: Sized {
    type Out;
    async fn build<T>(&self, args: Args) -> <Self as BuilderT<Args>>::Out
    where
        Self: BuilderT<T>,
        T: MapBounds<Args>,
        // T: Ctx<Base = Args>
        //     + TestSet<'static>
        //     + ChildTypesFn
        //     + Unpin
        //     + Clone
        //     + Send
        //     + Sync
        //     + 'static,
        ChildTypes<T>: MapNext<Self, T, ChildTypes<T>>;
}

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
        Default::default()
    }
}
#[async_trait]
pub trait Test<'a, Args>: Send + Sync {
    async fn run(&self, args: Args) -> TestRes<'a>;
    fn skip(&self) -> TestRes<'a> {
        Default::default()
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
            status: Default::default(),
            trace: &"",
        }
    }
}

// pub type ApplySync2<F, Args> = <F as BuilderTSync2<Args>>::Out;
// pub trait BuilderTSync2<Args> {
//     type Out;
//     type El<T>;
//     fn build2(&self, args: Args) -> Self::Out;
// }
// pub trait BuilderTGat<X, Args> {
//     type Out<T>;
//     fn build<T>(&self, args: Args) -> Self::Out<T>;
// }
// pub struct MapTSync2<F, Args, Lst>
// where
//     F: BuilderTSync2<Args>,
//     Lst: ?Sized,
// {
//     pub f: F,
//     pub args: Args,
//     pub next: fn(&mut Self) -> Option<F::Out>,
// }
// pub trait MapNextSync2<T, F, Args, Lst>
// where
//     F: BuilderTSync2<Args>,
//     Lst: ?Sized,
// {
//     fn map_next(iter: &mut MapTSync2<F, Args, Self>) -> Option<ApplySync2<F, Args>>;
// }
// impl<T, F, Args, X, XS> MapNextSync2<T, F, Args, TNil> for TCons<X, XS>
// where
//     F: BuilderTSync2<Args>,
// {
//     fn map_next(_iter: &mut MapTSync2<F, Args, Self>) -> Option<ApplySync2<F, Args>> {
//         None
//     }
// }
// impl<T, F, Args, X, XS, Me> MapNextSync2<T, F, Args, TCons<X, XS>> for Me
// where
//     Self: MapNextSync2<T, F, Args, XS>,
//     F: BuilderTSync2<Args>,
//     Args: Clone,
// {
//     fn map_next(iter: &mut MapTSync2<F, Args, Self>) -> Option<ApplySync2<F, Args>> {
//         iter.next = <Self as MapNextSync2<T, F, Args, XS>>::map_next;
//         Some(F::build2(&iter.f, iter.args.clone()))
//     }
// }
//@wol
// We can afford to have the original Lst bleed into the type sig for MapT (in fact,
// its already there). What if BuilderT<Lst, Args> then build<T> just takes S<Z> etc.
// We would have to impl BuilderT<Lst Args> where all elems in Lst impl BuilderTAux.
// Damn, same problem as before though.
// Basically, can we make a BuilderT that is impld for an entire list if it can act on
// each element of that list?
// impl<T, F, Args, Lst> Iterator for MapTSync2<F, Args, Lst>
// where
//     F: BuilderTSync2<T, Args>>,
//     // F: BuilderTGat<T, Args>,
//     Lst: ?Sized,
// {
//     type Item = F::Out;
//     fn next(&mut self) -> Option<Self::Item> {
//         (self.next)(self)
//     }
// }
// where
// F: BuilderTSyncHiddenT<Args> for all T in Lst

// Implemented for the "original" list
pub trait MapNextSync<F, Args, Lst>
where
    F: BuilderTSync<Args>,
    Lst: ?Sized,
{
    fn map_next(iter: &mut MapTSync<F, Args, Self>) -> Option<ApplySync<F, Args>>;
}
impl<F, Args, H, T> MapNextSync<F, Args, TNil> for TCons<H, T>
where
    F: BuilderTSync<Args>,
{
    fn map_next(_iter: &mut MapTSync<F, Args, Self>) -> Option<ApplySync<F, Args>> {
        None
    }
}
impl<F, Args, H, T, Me> MapNextSync<F, Args, TCons<H, T>> for Me
where
    Self: MapNextSync<F, Args, T>,
    F: BuilderTSync<Args>,
    Args: Clone,
{
    fn map_next(iter: &mut MapTSync<F, Args, Self>) -> Option<ApplySync<F, Args>> {
        iter.next = <Self as MapNextSync<F, Args, T>>::map_next;
        Some(F::build::<H>(&iter.f, iter.args.clone()))
        // F::<H>::build()
        // where F<H>: BuilderTSync<Args>  --- GATs- F applied to H is a type that impls BuilderTSync<Args>
        // F<T> -> X - impl BuilderTSync<Args> for X { fn do_build() { T::build(); }}
        // F<U> -> Y - impl BuilderTSync<Args> for Y { fn do_build() { U::build(); }}
        // so F<T> -> Builder1, F<U> -> Builder2
        // Still can't map arbitrary functions over T and U though, I think
        // What if do_build takes a generic fn?
        // Different BuilderTSyncs should be different functions, all taking types
        // of different domains as arguments
        // F::Family<H, Args>::build();
        // but can we restrict H in the Family impl? - don't think so
        // We want to have functions from Type -> val, but we want them to be partial,
        // may not be possibl
        // re HKTs: This BuilderTSync<Args> that actually gives you a BuilderTSync<T, Args>
        // allowing the caller? to specify T. Really we want the caller to be able to specify
        // the trait bounds on each T in the TList they map over
    }
}
impl<F, Args, Lst> Iterator for MapTSync<F, Args, Lst>
where
    F: BuilderTSync<Args>,
    Lst: ?Sized,
{
    type Item = F::Out;
    fn next(&mut self) -> Option<Self::Item> {
        (self.next)(self)
    }
}

// If I stick with build<T>, I want the builder of MapT to be able to specify trait bounds
// on that T, which every element of Lst must meet.
// We need to say that forall T in Lst, F is implemented / can operate on T
// T needs to remain unspecified because it is only determined when each element of the TList
// is reached by the iterator. So we have a type that must remain unspecified by F/BuilderT,
// But the creator of the BuilderT knows some bounds that it needs to meet (e.g. T must be a pointer/Arc/Rc).
// We're thinking about this in the wrong way. The "caller" is the iterator calling F with
// T. So instead of moving T into the build<T>() method, what can we do?
// So when we create a BuilderT, we don't yet know what T we want to use (bc it's given to us
// during iteration), but we know it must meet some trait bounds.
// A BuilderT is something that can build X from a T, but we don't yet know which T, so we can't call
// it a BuilderT<T>.
// HKTs allow you to split up a type - you decide one part of the type in one place, and another part of
// type in another place. We want to decide the trait bounds on a type in one place, and the type itself in
// another place. Can we do this?
// trait Trait<'a> {}
// trait Foo where for<'a> Self::Assoc<'a>: Trait<'a> {
//     type Assoc<'a>;
// }
// This could work. Would need to impl any trait we're interested
// for the list itself if all elements impl though?
#[marker]
trait Elem<Lst> {}
impl<H, T> Elem<TCons<H, T>> for H {}
impl<X, H, T> Elem<TCons<H, T>> for X where X: Elem<T> {}

// trait F<Lst> where for<T: Elem<Lst>> Self::Assoc<T>: ... {}
trait F<Lst> {
    // Associated type must impl Trait<T> for all T in Lst
    type Assoc<T: Elem<Lst>>: Trait<T>;
    // type Assoc<T: Debug>: Trait<T>;
}
struct Builder1;
struct Builder2;
impl<T> Trait<T> for Builder1 where T: Mark {}
type L = crate::TList!(String, usize);
type L2 = crate::TList!(String, u8);
// impl Mark for String {} impl Mark for u8 {}
impl F<L2> for Builder1 {
    type Assoc<T: Elem<L2>> = Builder1;
    // type Assoc<T: Mark> = Builder1;
}
pub trait Mark {}
impl<T> Mark for T where T: Elem<L2> {}
// impl<T> Mark for T where T: Elem<L> {}

fn elem<T, Lst>()
where
    T: Elem<Lst>,
{
}
fn test_elem() {
    type X = crate::TList!(String, usize);
    elem::<String, X>();
}

trait BuilderTest {}
trait BuilderTestAux {}
impl<H, T> BuilderTest for TCons<H, T>
where
    H: BuilderTestAux,
    T: BuilderTest,
{
}
impl BuilderTest for TNil {}

trait Trait<T> {}
struct F1;
struct F2;
impl<T> Trait<T> for F1 where T: Debug {}
impl<T> Trait<T> for F2 where T: Default {}
trait Foo {
    type Assoc<T>: Trait<T>;
}

trait Func<T> {
    type Out;
    fn foo();
}

struct Error;
// but we still can't restrict T outside of this fn.
fn call_fn<T, Builder>() {
    // Builder::run<T>();
    // Builder::This<T>::run();
}
fn call_fn2<T, F>(f: F) {
    // f::<T>();
}

// How would I say "for all T in TList, T impls trait X"?
// impl<T> Func<T> for Builder1 where T: !Debug { }
// passing trait bounds around is the same as Will's higher-order type functions,
// bc he thinks of traits as functions on types. This means we need specialization?
// impl<T> Func<T> for Builder2 where T: Default {
//     type Out = usize;
//     fn foo() { let t = T::default(); }
// }
trait Foo2 {
    type Assoc<T: Debug>: Func<T>;
}
struct Type;
// impl Foo2 for Type {
//     type Assoc<T> = <Builder1 as Func<T>>::Out;
// }
#[derive(Debug)]
struct Type2;
struct Baz<P: Foo2> {
    x: P::Assoc<Type2>,
}
// fn boo2() {
//     let x: Baz<
// }
// so Func<T> is our BuilderT. What's Foo?
// trait Foo where for<T> Self::Assoc<T>: Trait<T> {
//     type Assoc<T>;
// }

// A RcPtr is something that wraps a given T, but we don't yet know which T, so we can't call it a RcPtr<T>.
// struct FnOnCtxs<T: RcPtr> {
//     _: T<>,
// }
// gat means caller decides whether to give me a BuilderT<T, ..> or a BuilderT<U ...>?
// But BuilderT is a trait not a type
// pub struct MapTSync<F, Args, Lst> where Self: Iterator, Lst: ?Sized {

pub type StreamItem<T> = <T as Stream>::Item;
pub trait Next<'a, F, Args> {
    type Map: Stream;
    fn next(&self, map: &mut Self::Map) -> StreamItem<Self::Map>;
}
// F: FnTMut
pub struct MapTExp<'a, F, Args, Lst> {
    pub f: F,
    pub args: Args,
    pub next: &'a dyn Next<'a, F, Args, Map = Self>,
}
pub trait MapAll<Args, Lst> {}
impl<F, Args> MapAll<Args, TNil> for F {}
impl<Args, F, H, T> MapAll<Args, TCons<H, T>> for F where F: FnTMut<H, Args> + MapAll<Args, T> {}
impl<'a, F, Args, Lst> Stream for MapTExp<'a, F, Args, Lst>
where
    F: MapAll<Args, Lst> + FnTMut<Head<Lst>, Args> + Unpin,
    Args: Unpin + Clone,
    Lst: HeadFn,
{
    type Item = F::Out;
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let args = self.args.clone();
        let mut fut = self.get_mut().f.call(args);
        let out = ready!(fut.poll_unpin(cx));
        Poll::Ready(Some(out))
    }
}
pub trait U<const N: usize> {
    type Nat;
}
pub struct Converter;
impl U<0> for Converter {
    type Nat = Zero;
}
impl U<1> for Converter {
    type Nat = Succ<Zero>;
}
impl U<2> for Converter {
    type Nat = Succ<Succ<Zero>>;
}
pub trait Nat {
    const N: usize;
}
impl Nat for Zero {
    const N: usize = 0;
}
impl<Pred: Nat> Nat for Succ<Pred> {
    const N: usize = 1 + Pred::N;
}

pub fn take_type<T>() {
    println!("take type");
}
pub enum FooFoo {
    Foo1,
    Foo2,
}
pub trait X {
    type Out;
}
pub fn take<T>(x: usize) {
    match x {
        1 => println!("1"),
        2 => println!("2"),
        _ => panic!(""),
    }
}
// pub trait Bat<const N: usize>: HeadFn + TailFn
// where
//     Tail<Self>: Bat<{N - 1}>,
//     Tail<Tail<Self>>: Bat<{N - 2}>,
//     [(); {N - 1}]: Sized,
// {}
// pub trait InBoundsSuper<N>: TailFn where N: PredFn, Tail<Self>: InBoundsSuper<Pred<N>> {}
// pub trait InBounds<N> {}
// impl<Lst> InBounds<Zero> for Lst {}
// impl<N, Lst> InBounds<N> for Lst where Lst: TailFn, N: PredFn, Tail<Lst>: InBounds<Pred<N>> {}
// pub fn map<Lst, N>() where Lst: HeadFn + TailFn + InBounds<N>, N: PredFn {
//     take_type::<Head<Lst>>();
//     map::<Tail<Lst>, Pred<N>>();
// }
// pub fn map<Lst, const N: usize>() where Lst: HeadFn + TailFn, Tail<Lst>: HeadFn + TailFn {
//     take_type::<Head<Lst>>();
//     map::<Tail<Lst>, {N - 1}>();
// }

pub struct MapTSync<F, Args, Lst>
where
    F: BuilderTSync<Args>,
    Lst: ?Sized,
{
    pub f: F,
    pub args: Args,
    pub next: fn(&mut Self) -> Option<F::Out>,
}

impl<F, Args, Lst> MapTSync<F, Args, Lst>
where
    F: BuilderTSync<Args>,
    Lst: ?Sized + MapNextSync<F, Args, Lst>,
{
    pub fn new(f: F, args: Args) -> Self {
        Self {
            f,
            args,
            next: <Lst as MapNextSync<F, Args, Lst>>::map_next,
        }
    }
}

pub struct MapT<F, Args, Lst>
where
    F: BuilderT<Args>,
    Lst: ?Sized,
{
    pub f: F,
    pub args: Args,
    pub next: fn(&mut Self) -> Option<ApplyFut<F, Args>>,
}
impl<F, Args, Lst> MapT<F, Args, Lst>
where
    F: BuilderT<Args>,
    Lst: ?Sized + MapNext<F, Args, Lst>,
{
    pub fn new(f: F, args: Args) -> Self {
        Self {
            f,
            args,
            next: <Lst as MapNext<F, Args, Lst>>::map_next,
        }
    }
}
// Implemented for the "original" list
pub trait MapNext<F, Args, Lst>
where
    F: BuilderT<Args>,
    Lst: ?Sized,
{
    fn map_next(iter: &mut MapT<F, Args, Self>) -> Option<ApplyFut<F, Args>>;
}
impl<F, Args, H, T> MapNext<F, Args, TNil> for TCons<H, T>
where
    F: BuilderT<Args>,
{
    fn map_next(_iter: &mut MapT<F, Args, Self>) -> Option<ApplyFut<F, Args>> {
        None
    }
}
impl<F, Args> MapNext<F, Args, TNil> for TNil
where
    F: BuilderT<Args>,
{
    fn map_next(_iter: &mut MapT<F, Args, Self>) -> Option<ApplyFut<F, Args>> {
        None
    }
}
impl<'a, F, Args, H, T, Lst> MapNext<F, Args, TCons<H, T>> for Lst
where
    Self: MapNext<F, Args, T>,
    F: BuilderT<Args> + BuilderT<H>,
    Args: Clone,
    // H: Ctx<Base = Args> + TestSet<'static> + ChildTypesFn + Unpin + Clone + Send + Sync + 'static,
    H: MapBounds<Args>,
    ChildTypes<H>: MapNext<F, H, ChildTypes<H>>,
{
    fn map_next(iter: &mut MapT<F, Args, Self>) -> Option<ApplyFut<F, Args>> {
        iter.next = <Self as MapNext<F, Args, T>>::map_next;
        Some(F::build::<H>(&iter.f, iter.args.clone()))
    }
}

impl<F, Args, Lst> Stream for MapT<F, Args, Lst>
where
    F: BuilderT<Args> + Unpin,
    Args: Unpin,
    Lst: ?Sized,
{
    type Item = F::Out;
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let maybe_fut = (self.next)(self.get_mut());
        match maybe_fut {
            Some(mut fut) => {
                let res = ready!(fut.poll_unpin(cx));
                Poll::Ready(Some(res))
            }
            None => Poll::Ready(None),
        }
    }
}

pub struct Runner;
#[async_trait]
impl<Args> BuilderT<Args> for Runner
where
    Args: Send + 'static,
{
    type Out = ();
    async fn build<T>(&self, args: Args) -> Self::Out
    where
        Self: BuilderT<T>,
        T: MapBounds<Args>,
        // T: Ctx<Base = Args>
        //     + TestSet<'static>
        //     + ChildTypesFn
        //     + Unpin
        //     + Clone
        //     + Send
        //     + Sync
        //     + 'static,
        ChildTypes<T>: MapNext<Self, T, ChildTypes<T>>,
    {
        let ctx = T::build(args).await;
        let tests = T::tests();
        let mut test_stream = TestStream::new(tests.iter(), ctx.clone());
        while let Some(t) = test_stream.next().await {
            // println!("trace: {:?}", t.trace);
        }
        let mut child_stream = MapT::<_, _, ChildTypes<T>>::new(Self, ctx);
        while let Some(child) = child_stream.next().await {
            println!("child");
        }
        // let _: ChildTypes<T>;
    }
}

// How would I define a function for which each type P has a different, possibly overlapping domain?
// I can't do this because T ends up either bleeding into the type of MapT or being unconstrained. Why is this?
// P::act(t) t element of Dom(P)
// Q::act(t) t element of Dom(Q)
// trait Foo {} trait Bar {} trait Baz{}
// trait Trait<T> { fn act(t: T); }
// struct P; struct Q;
// impl<T> Trait<T> for P where T: Foo { fn act(t: T) {}}
// impl<T> Trait<T> for Q where T: Bar { fn act(t: T) {}}

// (overlapping impls)
// struct Op<T>(PhantomData<T>);
// impl<T> Foo for Op<T> where T: Bar {}
// impl<U> Foo for Op<U> where T: Baz {}
// self.next is an associated fn so that it can have diff behavior for diff input types
fn foo<T>()
where
    T: Default,
{
    let x: T = Default::default();
    let x: fn() -> T = Default::default;
    boo(x);
}
fn boo<T>(f: fn() -> T) {
    f();
    // Want to be able to pass a type arg to an arbitrary function, but can't do
    // f<T>();
    // Can make f a trait associated fn, then we know the "pointer" takes a gen param.
    // But the we can't set our own bounds on the generic param for each trait impl
    // Can I do this with type families?
    // Imagine if I wanted to be generic over Arc<i32>, Rc<i32>, but only Rc should be able to be instantiated for u32
}
// https://users.rust-lang.org/t/emulating-type-families-with-relational-traits/29463
// impl Builder<Lst: TList, N: Nat> for ...
// What about generating an enum for each list, then matching on the element discrimant

// Same as the Iterator impl except:
// - F is async, and the Stream Item is not the future but the resolved type.
// - Need to propogate the None if out of items, else unwrap to get the future
// - Need to poll the future and return it's result
// impl<F, Args, Res, Lst> Stream for MapTSync<F, Args, Lst>
// where
//     F: BuilderTSync<Args, Out = AsyncOutput<Res>> + Unpin,
//     Args: Unpin,
//     Lst: ?Sized,
// {
//     type Item = Res;
//     fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
//         let maybe_fut = (self.next)(self.get_mut());
//         if let Some(mut fut) = maybe_fut {
//             let res = ready!(fut.poll_unpin(cx));
//             Poll::Ready(Some(res))
//         } else {
//             Poll::Ready(None)
//         }
//     }
// }

#[cfg(test)]
mod test {
    use super::*;
    use crate as runrun;
    use crate::{register_ctx, TList};
    use anyhow::Result;
    use linkme::distributed_slice;

    #[derive(Debug, Clone)]
    pub struct NullCtx;
    #[derive(Debug, Clone)]
    pub struct Ctx0;
    #[derive(Debug, Clone)]
    pub struct Ctx1;
    #[derive(Debug, Clone)]
    pub struct Ctx2;
    #[async_trait]
    impl Ctx for Ctx0 {
        type Base = NullCtx;
        async fn build(_: Self::Base) -> Self {
            println!("building 0");
            Self
        }
    }
    #[async_trait]
    impl Ctx for Ctx1 {
        type Base = NullCtx;
        async fn build(_: Self::Base) -> Self {
            println!("building 1");
            Self
        }
    }
    #[async_trait]
    impl Ctx for Ctx2 {
        type Base = Ctx0;
        async fn build(_: Self::Base) -> Self {
            println!("building 2");
            Self
        }
    }

    #[derive(Clone)]
    pub struct TestCase<'a, T> {
        // pub name: &'static str,
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
            let status = match (self.test)(args).await {
                Ok(_) => Status::Pass,
                _ => Status::Fail,
            };
            TestRes {
                status,
                trace: &"todo",
            }
        }
    }
    async fn test_01(_: Ctx0) -> Result<()> {
        println!("running test_01");
        Ok(())
    }
    async fn test_02(_: Ctx0) -> Result<()> {
        println!("running test_02");
        Ok(())
    }
    async fn test_11(_: Ctx1) -> Result<()> {
        println!("running test_11");
        Ok(())
    }
    async fn test_21(_: Ctx2) -> Result<()> {
        println!("running test_21");
        Ok(())
    }

    #[distributed_slice]
    pub static TESTS_ON_CTX0: [&'static dyn Test<'static, Ctx0>] = [..];
    #[distributed_slice(TESTS_ON_CTX0)]
    pub static __T01: &dyn Test<Ctx0> = &TestCase {
        name: "test_01",
        test: &|x| Box::pin(test_01(x)),
    };
    #[distributed_slice(TESTS_ON_CTX0)]
    pub static __T02: &dyn Test<Ctx0> = &TestCase {
        name: "test_02",
        test: &|x| Box::pin(test_02(x)),
    };
    #[distributed_slice]
    pub static TESTS_ON_CTX1: [&'static dyn Test<'static, Ctx1>] = [..];
    #[distributed_slice(TESTS_ON_CTX1)]
    pub static __T11: &dyn Test<Ctx1> = &TestCase {
        name: "test_11",
        test: &|x| Box::pin(test_11(x)),
    };
    #[distributed_slice]
    pub static TESTS_ON_CTX2: [&'static dyn Test<'static, Ctx2>] = [..];
    #[distributed_slice(TESTS_ON_CTX2)]
    pub static __T21: &dyn Test<Ctx2> = &TestCase {
        name: "test_21",
        test: &|x| Box::pin(test_21(x)),
    };

    impl TestSet<'static> for Ctx0 {
        fn tests() -> &'static [&'static dyn Test<'static, Self>] {
            &TESTS_ON_CTX0
        }
    }
    impl TestSet<'static> for Ctx1 {
        fn tests() -> &'static [&'static dyn Test<'static, Self>] {
            &TESTS_ON_CTX1
        }
    }
    impl TestSet<'static> for Ctx2 {
        fn tests() -> &'static [&'static dyn Test<'static, Self>] {
            &TESTS_ON_CTX2
        }
    }

    fn noop() {}
    struct Unit;
    impl<T, Args> FnTSync<T, Args> for Unit {
        type Out = ();
        fn call(&self, args: Args) -> Self::Out {}
    }

    register_ctx!(NullCtx, [Ctx1, Ctx0]);
    register_ctx!(Ctx0, [Ctx2]);
    register_ctx!(Ctx1);
    register_ctx!(Ctx2);

    struct NoopBuilder;
    impl BuilderTSync<()> for NoopBuilder {
        type Out = usize;
        fn build<T>(&self, _args: ()) -> Self::Out {
            1
        }
    }
    #[async_trait]
    impl BuilderT<()> for NoopBuilder {
        type Out = usize;
        async fn build<T>(&self, _args: ()) -> Self::Out {
            2
        }
    }

    // Can't just map the driver, need to map into a function pointer with a provided generic
    #[tokio::test]
    async fn test() {
        // Get a stream of types from NullCtx:
        // type Foo = TList!(Ctx1, Ctx2);
        // let mut m = MapTSync::<_, _, Foo>::new(NoopBuilder, ());
        // dbg!(m.next());
        // dbg!(m.next());
        // dbg!(m.next());

        // let mut s = MapT::<_, _, Foo>::new(NoopBuilder, ());
        // dbg!(s.next().await);
        // dbg!(s.next().await);
        // dbg!(s.next().await);

        type Ctxs = TList!(Ctx0);
        // let _ = <ChildTypes<NullCtx> as MapNext<Runner, NullCtx, ChildTypes<NullCtx>>>::map_next;

        let init_ctx = NullCtx;
        let mut s = MapT::<_, _, ChildTypes<NullCtx>>::new(Runner, init_ctx);
        while let Some(x) = s.next().await {
            println!("x: {:?}", x);
        }
    }
}

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
    type IntoIter = iter::Chain<iter::Once<Self::Item>, <T as MapToIter<F, Args>>::IntoIter>;
    fn map_to_iter(f: F, args: Args) -> Self::IntoIter {
        let head = <F as FnTSync<H, Args>>::call(&f, args.clone());
        let rest = <T as MapToIter<F, Args>>::map_to_iter(f, args);
        iter::once(head).chain(rest)
    }
}
impl<F, Args, H> MapToIter<F, Args> for TCons<H, TNil>
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

// If I build my own iterator (stream) type, then can I map a TList into
// it without evaluating the Ctx::build() methods?

// type Len<T> = <T as TList>::Len;
// pub trait TList {
//     type Len;
//     const LEN: usize;
// }
// impl TList for TNil {
//     type Len = Zero;
//     const LEN: usize = 0;
// }
// impl<H, T> TList for TCons<H, T>
// where
//     T: TList,
// {
//     type Len = Succ<<T as TList>::Len>;
//     const LEN: usize = 1 + <T as TList>::LEN;
// }

async fn runrun<T, Args>(args: Args) -> ()
where
    T: Ctx<Base = Args> + TestSet<'static> + Unpin + Clone + Send + Sync + 'static,
    Args: Send + 'static,
{
    let ctx = T::build(args).await;
    let tests = T::tests();
    let mut stream = TestStream::new(tests.iter(), ctx);
    while let Some(t) = stream.next().await {
        println!("test: {:?}", t.trace);
    }
}
// #[derive(Debug, Clone)]
// struct RunrunBuilder;
// impl<T, Args> FnTSync<T, ()> for RunrunBuilder
// where
//     T: Ctx<Base = Args> + TestSet<'static> + Unpin + Clone + Send + Sync + 'static,
//     Args: Send + 'static,
// {
//     type Out = &'static AsyncFn<Args, ()>;
//     // Note that the builder args are different than the args passed to the generated fn
//     fn call(&self, _bargs: ()) -> Self::Out {
//         &|args| Box::pin(runrun::<T, Args>(args))
//     }
// }

///// MapT async impl
// pub struct MapT<'s, F, Args, Lst>
// where
//     F: BuilderT<Args>,
//     Lst: ?Sized,
// {
//     pub f: F,
//     pub args: Args,
//     pub next: fn(&'s mut Self) -> AsyncOutputTick<'s, Option<F::Out>>,
// }
// // Implemented for the "original" list
// #[async_trait]
// pub trait MapNext<F, Args, Lst>
// where
//     Self: HeadFn,
//     F: BuilderT<Args>,
//     Lst: ?Sized,
// {
//     async fn map_next(iter: &mut MapT<F, Args, Self>) -> Option<F::Out>;
// }
// #[async_trait]
// impl<F, Args, H, T> MapNext<F, Args, TNil> for TCons<H, T>
// where
//     F: BuilderT<Args> + Send,
//     Args: Send,
// {
//     async fn map_next(_iter: &mut MapT<F, Args, Self>) -> Option<F::Out> {
//         None
//     }
// }
// #[async_trait]
// impl<F, Args, H, T, Me> MapNext<F, Args, TCons<H, T>> for Me
// where
//     Self: MapNext<F, Args, T>,
//     F: BuilderT<Args> + Send + Sync,
//     Args: Clone + Send + Sync,
// {
//     async fn map_next(iter: &mut MapT<F, Args, Self>) -> Option<F::Out> {
//         iter.next = <Self as MapNext<F, Args, T>>::map_next;
//         Some(F::build::<H>(&iter.f, iter.args.clone()).await)
//     }
// }
/////
