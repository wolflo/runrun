use async_trait::async_trait;
use futures::stream::Stream;
use std::fmt::Debug;

use crate::types::{AsyncFn, ChildTypesFn, FnT, MapT};

// Used by the MapT type to bound the types that can be mapped over. Ideally
// we would be able to map an arbitrary FnT, but unfortunately we can only map
// FnT's that share the same trait bounds.
pub trait MapBounds<Args>:
    Ctx<Base = Args> + TestSet<'static> + ChildTypesFn + Unpin + Clone + Send + Sync + 'static
{
}
impl<T, Args> MapBounds<Args> for T where
    T: Ctx<Base = Args> + TestSet<'static> + ChildTypesFn + Unpin + Clone + Send + Sync + 'static
{
}

pub enum Mode {
    Run,
    Skip,
}
impl Mode {
    pub fn is_run(&self) -> bool {
        match self {
            Self::Run => true,
            _ => false,
        }
    }
    pub fn is_skip(&self) -> bool {
        match self {
            Self::Skip => true,
            _ => false,
        }
    }
}
#[async_trait]
impl<T, In, Out> Runner<In, Out> for T
where
    T: Iterator + Send,
    T::Item: Test<In, Out>,
    In: Send + 'static,
    Out: Default,
{
    // type Out = <<T as Iterator>::Item as Test<In>>::Out;
        async fn step(&mut self, mode: Mode, args: In) -> Option<Out> {
        match self.next() {
            Some(t) => match mode {
                Mode::Run => Some(t.run(args).await),
                Mode::Skip => Some(t.skip()),
            },
            None => None,
        }
    }
}
impl<T> ExactSize for T
where
    T: ExactSizeIterator,
{
    fn len(&self) -> usize {
        self.len()
    }
}

#[async_trait]
pub trait Runner<In, Out> {
    async fn step(&mut self, mode: Mode, args: In) -> Option<Out>;
}
// An iterator where the type of the thing it returns is tied to the type of the args passed to next()
// Every Runner wraps another Runner (with the innermost Runner being an iterator).
// Given a new runner, make a new version of myself that wraps this new runner
// pub trait RunnerBuilder<Out> {
//     type This<A, B>: Send
//     where
//         A: Runner<B, Out> + Send;
//     fn build<A, B>(&self, a: A) -> Self::This<A, B>
//     where
//         A: Runner<B, Out> + Send;
//     // type This<A, B>: Runner<B, Out> + Send where A: Runner<B, Out> + ExactSize + Send, B: Send + 'static;
//     // fn build<A, B>(&self, a: A) -> Self::This<A, B> where A: Runner<B, Out> + ExactSize + Send, B: Send + 'static;
// }
// Given a set of tests (or an inner Runner), build a Runner
#[async_trait]
pub trait RunnerBuilder<IOut, Out> {
    type R<I, In>: Runner<In, Out> + Send
    where
        I: Runner<In, IOut> + ExactSize + Send,
        In: Send + 'static;

    async fn new<I, In>(&self, inner: I) -> Self::R<I, In>
    where
        I: Runner<In, IOut> + ExactSize + Send,
        In: Send + 'static;
}
pub trait ExactSize {
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[async_trait]
pub trait Ctx {
    type Base;
    async fn build(base: Self::Base) -> Self;
}
pub trait TestSet<'a> {
    fn tests() -> &'a [&'a dyn Test<Self, TestRes>];
}
#[async_trait]
pub trait Test<In, Out>: Send + Sync where Out: Default
{
    async fn run(&self, args: In) -> Out;
    fn skip(&self) -> Out {
        Default::default()
    }
}
#[derive(Debug)]
pub struct TestRes {
    pub status: Status,
    pub trace: Box<dyn Debug + Send + Sync>,
    // pub trace: &'a (dyn Debug + Sync),
}
#[derive(Debug, Clone, Copy)]
pub enum Status {
    Pass,
    Fail,
    Skip,
}
impl Status {
    pub fn is_pass(&self) -> bool {
        match self {
            Self::Pass => true,
            _ => false,
        }
    }
    pub fn is_fail(&self) -> bool {
        match self {
            Self::Fail => true,
            _ => false,
        }
    }
    pub fn is_skip(&self) -> bool {
        match self {
            Self::Skip => true,
            _ => false,
        }
    }
}

impl Default for Status {
    fn default() -> Self {
        Self::Skip
    }
}
impl Default for TestRes {
    fn default() -> Self {
        Self {
            status: Default::default(),
            trace: Box::new(""),
        }
    }
}

#[derive(Clone)]
pub struct TestCase<'a, In, Out> {
    pub name: &'static str,
    pub test: &'a AsyncFn<'a, In, Out>,
}
#[async_trait]
impl<'a, In, Out, Y> Test<In, Out> for TestCase<'_, In, Y>
where
    In: Send + Sync + 'static,
    Y: Test<(), Out>, // could also pass args to result.run()
    Out: Default,
{
    async fn run(&self, args: In) -> Out {
        println!("{}", self.name);
        (self.test)(args).await.run(()).await
    }
}
// () is a passing Test
#[async_trait]
impl<'a, In> Test<In, TestRes> for ()
where
    In: Send + 'static,
{
    async fn run(&self, _args: In) -> TestRes {
        TestRes {
            status: Status::Pass,
            trace: Box::new(""),
        }
    }
}
// true is a passing Test, false is a failing Test
#[async_trait]
impl<In> Test<In, TestRes> for bool
where
    In: Send + 'static,
{
    async fn run(&self, _args: In) -> TestRes {
        let status = if *self { Status::Pass } else { Status::Fail };
        TestRes { status, trace: Box::new("") }
    }
}
#[async_trait]
impl<'a, T, E, In> Test<In, TestRes> for Result<T, E>
where
    T: Test<In, TestRes> + Send + Sync,
    E: Into<Box<dyn std::error::Error>> + Send + Sync + 'a,
    In: Send + Sync + 'static,
{
    async fn run(&self, args: In) -> TestRes {
        match self {
            Ok(r) => r.run(args).await,
            Err(e) => TestRes {
                status: Status::Fail,
                trace: Box::new("Test of Err value. We should provide real traces for these."), // trace: (*e).clone().into(),
            },
        }
    }
    fn skip(&self) -> TestRes {
        match self {
            Ok(r) => r.skip(),
            Err(e) => TestRes {
                status: Status::Fail,
                trace: Box::new("Skip of Err value. We should provide real traces for these."), // trace: (*e).clone().into(),
            },
        }
    }
}
#[async_trait]
impl<T, In, Out> Test<In, Out> for &'_ T
where
    T: Test<In, Out> + ?Sized + Send + Sync,
    In: Send + Sync + 'static,
    Out: Default,
{
    async fn run(&self, args: In) -> Out {
        (**self).run(args).await
    }
    fn skip(&self) -> Out {
        (**self).skip()
    }
}
#[async_trait]
impl<T, In, Out> Test<In, Out> for &'_ mut T
where
    T: Test<In, Out> + ?Sized + Send + Sync,
    In: Send + Sync + 'static,
    Out: Default,
{
    async fn run(&self, args: In) -> Out {
        (**self).run(args).await
    }
    fn skip(&self) -> Out {
        (**self).skip()
    }
}
#[async_trait]
impl<T, In, Out> Test<In, Out> for Box<T>
where
    T: Test<In, Out> + ?Sized + Send + Sync,
    In: Send + Sync + 'static,
    Out: Default,
{
    async fn run(&self, args: In) -> Out {
        (**self).run(args).await
    }
    fn skip(&self) -> Out {
        (**self).skip()
    }
}
#[async_trait]
impl<T, In, Out> Test<In, Out> for std::panic::AssertUnwindSafe<T>
where
    T: Test<In, Out> + Send + Sync,
    In: Send + Sync + 'static,
    Out: Default,
{
    async fn run(&self, args: In) -> Out {
        self.0.run(args).await
    }
    fn skip(&self) -> Out {
        self.0.skip()
    }
}

// * -> *
// pub trait Builder {
//     type This<A>; // A is the type the Builder is applied to
// }
// pub trait Functor: Builder {
//     // map (<$>) :: f a -> (a -> b) -> f b
//     fn map<A, B, F>(self, fa: Self::This<A>, f: F) -> Self::This<B>
//     where
//         F: Fn(A) -> B;
//     fn map_const<A, B>(self, fa: Self::This<A>, b: B) -> Self::This<B>;
// }
// pub trait Link<I> {
//     fn link(&mut self, iter: I);
// }
// impl<T> Iterator for BaseRunner<T> where T: Iterator {
//     type Item = T::Item;
//     fn next(&mut self) -> Option<Self::Item> {
//         self.iter.next()
//     }
// }
// impl<T, U> Link<U> for BaseRunner<T> where T: Extend<U::Item>, U: IntoIterator {
//     fn link(&mut self, iter: U) {
//         self.iter.extend(iter);
//     }
// }
