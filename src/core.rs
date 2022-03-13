use async_trait::async_trait;
use futures::FutureExt;
use std::{fmt::Debug, panic::AssertUnwindSafe};

use crate::types::{tmap, AsyncFn, ChildTypes, ChildTypesFn, FnOut, FnT, MapStep, TList};

// Used by the MapT type to bound the types that can be mapped over. Ideally
// we would be able to map an arbitrary FnT, but we can only map FnT's that
// share the same trait bounds.
pub trait MapBounds<Args>:
    Ctx<Base = Args> + TestSet<'static> + ChildTypesFn + Clone + Send + Sync + 'static
{
}
impl<T, Args> MapBounds<Args> for T where
    T: Ctx<Base = Args> + TestSet<'static> + ChildTypesFn + Clone + Send + Sync + 'static
{
}

pub async fn start<T, RB, Args>(runner_builder: RB, args: Args)
where
    RB: Builder<T>,
    RB::Built: Runner + Send + Clone,
    T: Ctx<Base = Args> + ChildTypesFn + TestSet<'static> + Send + Sync + Clone + 'static,
    ChildTypes<T>: MapStep<Driver<RB::Built>, T> + TList,
{
    let init_ctx = T::build(args).await;
    let runner = runner_builder.build(&init_ctx);
    // run tests on init_ctx
    run_ctx(runner.clone(), init_ctx.clone()).await;
    let driver = Driver::new(runner);
    tmap::<T, _, _>(&driver, init_ctx).await;
}

// Bidirectional relationship between a builder and the object it builds.
pub trait Builder<T> {
    type Built;
    fn build(self, base: &T) -> Self::Built;
}
pub trait Built {
    type Builder;
    fn builder() -> Self::Builder;
}

#[derive(Debug, Clone)]
pub struct Driver<R> {
    runner: R,
}
impl<R> Driver<R> {
    pub fn new(runner: R) -> Self {
        Self { runner }
    }
}

pub async fn run_ctx<R, T>(mut runner: R, ctx: T)
where
    T: Ctx + TestSet<'static> + Clone + Send + Sync + 'static,
    R: Runner + Send,
{
    let tests = T::tests().iter();

    let mut pass = 0;
    let mut fail = 0;
    let mut skip = 0;
    for t in tests {
        let res = runner.run(t, ctx.clone()).await;
        match res.status {
            Status::Pass => pass += 1,
            Status::Fail => fail += 1,
            Status::Skip => skip += 1,
        }
    }

    println!("tests passed : {}", pass);
    println!("tests failed : {}", fail);
    println!("tests skipped: {}", skip);
}

#[async_trait]
impl<Base, R> FnT<Base> for Driver<R>
where
    Base: Send + 'static,
    R: Runner + Send + Sync + Clone,
{
    type Output = ();
    async fn call<T>(&self, args: Base) -> FnOut<Self, Base>
    where
        Self: FnT<T>,
        T: MapBounds<Base>,
        ChildTypes<T>: MapStep<Self, T> + TList,
    {
        let ctx = T::build(args).await;

        // run tests on current ctx
        run_ctx(self.runner.clone(), ctx.clone()).await;

        // map self over child ctxs
        tmap::<T, _, _>(self, ctx).await;
    }
}

#[async_trait]
pub trait Runner: Sync {
    type Inner: Runner + Send + Sync;
    fn inner(&mut self) -> &mut Self::Inner;

    async fn run<T, Args>(&mut self, t: T, args: Args) -> TestRes
    where
        T: Test<Args>,
        Args: Send,
    {
        self.inner().run(t, args).await
    }
    fn skip<T, Args>(&mut self, t: T) -> TestRes
    where
        T: Test<Args>,
        Args: Send,
    {
        self.inner().skip(t)
    }
}

#[derive(Debug, Clone)]
pub struct BaseRunner;
#[async_trait]
impl Runner for BaseRunner {
    type Inner = Self;
    fn inner(&mut self) -> &mut Self::Inner {
        self
    }

    async fn run<T, Args>(&mut self, t: T, args: Args) -> TestRes
    where
        T: Test<Args>,
        Args: Send,
    {
        t.run(args).await
    }
    fn skip<T, Args>(&mut self, t: T) -> TestRes
    where
        T: Test<Args>,
        Args: Send,
    {
        t.skip()
    }
}
// BaseRunner is its own builder
impl<T> Builder<T> for BaseRunner {
    type Built = BaseRunner;
    fn build(self, _base: &T) -> Self::Built {
        Self::Built {}
    }
}
impl Built for BaseRunner {
    type Builder = BaseRunner;
    fn builder() -> Self::Builder {
        Self::Builder {}
    }
}

#[async_trait]
pub trait Ctx {
    type Base;
    async fn build(base: Self::Base) -> Self;
}
pub trait TestSet<'a> {
    fn tests() -> &'a [&'a dyn Test<Self>];
}
#[async_trait]
pub trait Test<In>: Send + Sync {
    async fn run(&self, args: In) -> TestRes;
    fn skip(&self) -> TestRes {
        Default::default()
    }
}
#[derive(Debug)]
pub struct TestRes {
    pub status: Status,
    pub trace: Box<dyn Debug + Send + Sync>,
}

#[derive(Debug, Clone, Copy)]
pub enum Status {
    Pass,
    Fail,
    Skip,
}
impl Status {
    pub fn is_pass(&self) -> bool {
        matches!(self, Self::Pass)
    }
    pub fn is_fail(&self) -> bool {
        matches!(self, Self::Fail)
    }
    pub fn is_skip(&self) -> bool {
        matches!(self, Self::Skip)
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
pub struct TestCase<'a, X, Y> {
    pub name: &'static str,
    pub test: &'a AsyncFn<'a, X, Y>,
}
#[async_trait]
impl<'a, X, Y> Test<X> for TestCase<'_, X, Y>
where
    X: Send + Sync + 'static,
    Y: Test<()>, // could also pass args to result.run()
{
    async fn run(&self, args: X) -> TestRes {
        println!("{}", self.name);
        self.test.run(args).await
    }
}
#[async_trait]
impl<X, Y> Test<X> for AsyncFn<'_, X, Y>
where
    X: Send,
    Y: Test<()>,
{
    async fn run(&self, args: X) -> TestRes {
        let res = AssertUnwindSafe(self(args)).catch_unwind().await;
        match res {
            Ok(t) => t.run(()).await,
            _ => {
                println!("Test panic.");
                Default::default()
            }
        }
    }
}
// () is a passing Test
#[async_trait]
impl<'a, In> Test<In> for ()
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
impl<In> Test<In> for bool
where
    In: Send + 'static,
{
    async fn run(&self, _args: In) -> TestRes {
        let status = if *self { Status::Pass } else { Status::Fail };
        TestRes {
            status,
            trace: Box::new(""),
        }
    }
}
#[async_trait]
impl<'a, T, E, In> Test<In> for Result<T, E>
where
    T: Test<In> + Send + Sync,
    E: Into<Box<dyn std::error::Error>> + Send + Sync + 'a,
    In: Send + Sync + 'static,
{
    async fn run(&self, args: In) -> TestRes {
        match self {
            Ok(r) => r.run(args).await,
            Err(_e) => TestRes {
                status: Status::Fail,
                trace: Box::new("Test of Err value. We should provide real traces for these."), // trace: (*e).clone().into(),
            },
        }
    }
    fn skip(&self) -> TestRes {
        match self {
            Ok(r) => r.skip(),
            Err(_e) => TestRes {
                status: Status::Fail,
                trace: Box::new("Skip of Err value. We should provide real traces for these."), // trace: (*e).clone().into(),
            },
        }
    }
}
#[async_trait]
impl<T, In> Test<In> for &'_ T
where
    T: Test<In> + ?Sized + Send + Sync,
    In: Send + Sync + 'static,
{
    async fn run(&self, args: In) -> TestRes {
        (**self).run(args).await
    }
    fn skip(&self) -> TestRes {
        (**self).skip()
    }
}
#[async_trait]
impl<T, In> Test<In> for &'_ mut T
where
    T: Test<In> + ?Sized + Send + Sync,
    In: Send + Sync + 'static,
{
    async fn run(&self, args: In) -> TestRes {
        (**self).run(args).await
    }
    fn skip(&self) -> TestRes {
        (**self).skip()
    }
}
#[async_trait]
impl<T, In> Test<In> for Box<T>
where
    T: Test<In> + ?Sized + Send + Sync,
    In: Send + Sync + 'static,
{
    async fn run(&self, args: In) -> TestRes {
        (**self).run(args).await
    }
    fn skip(&self) -> TestRes {
        (**self).skip()
    }
}
#[async_trait]
impl<T, In> Test<In> for std::panic::AssertUnwindSafe<T>
where
    T: Test<In> + Send + Sync,
    In: Send + Sync + 'static,
{
    async fn run(&self, args: In) -> TestRes {
        self.0.run(args).await
    }
    fn skip(&self) -> TestRes {
        self.0.skip()
    }
}
