use anyhow::Result;
use async_trait::async_trait;
use futures::{future::Future, FutureExt};
use std::{fmt, fmt::Debug, panic::AssertUnwindSafe, sync::Arc};

use crate::ty::{tmap, ChildTypes, ChildTypesFn, FnT, MapFn};

pub type AsyncOutput<R> = std::pin::Pin<Box<dyn Future<Output = R> + Send>>;
pub type AsyncFn<X, Y> = &'static (dyn Fn(X) -> AsyncOutput<Y> + Sync);
pub trait DebugTest<T>: Test<T> + Debug + Send + Sync {}
impl<T, U> DebugTest<T> for U where U: Test<T> + Debug + Send + Sync {}
pub type Testable<T> = &'static dyn DebugTest<T>;

#[async_trait]
pub trait Runner {
    type Out;
    type Base;
    fn new(base: Self::Base) -> Self;
    async fn run<C, T>(&mut self, ctx: &C, tests: &'static [&T]) -> Self::Out
    where
        C: Send + Sync + Clone,
        T: Test<C> + Debug + ?Sized + Send + Sync;
}
#[async_trait]
pub trait Ctx {
    type Base;
    async fn build(base: Self::Base) -> Self;
}
pub trait TestSet {
    fn tests() -> &'static [Testable<Self>];
}

pub struct TestResult {
    pub status: Status,
}
pub enum Status {
    Pass,
    Fail,
}
#[async_trait]
pub trait Test<Args> {
    async fn run(&self, args: Args) -> TestResult;
}
#[async_trait]
impl<Args, Y> Test<Args> for AsyncFn<Args, Y>
where
    Args: Send + Sync,
    Y: Test<()> + Send + Sync,
{
    async fn run(&self, args: Args) -> TestResult {
        let res = AssertUnwindSafe(self(args))
            .catch_unwind()
            .await
            .map_err(|e| anyhow::anyhow!("Test panic: {:?}", e));
        res.run(()).await
    }
}
// Any Result resolves either to a Failing test case or to the result of
// testing the contained value
#[async_trait]
impl<T, E, Args> Test<Args> for Result<T, E>
where
    Args: Send + Sync + 'static,
    T: Test<Args> + Sync,
    E: Sync + Debug,
{
    async fn run(&self, args: Args) -> TestResult {
        match self {
            Ok(r) => r.run(args).await,
            Err(e) => {
                println!("Test failure! Err: {:?}", e);
                TestResult {
                    status: Status::Fail,
                }
            }
        }
    }
}
// () is a trivially passing Test
#[async_trait]
impl<Args> Test<Args> for ()
where
    Args: Send + 'static,
{
    async fn run(&self, _args: Args) -> TestResult {
        TestResult {
            status: Status::Pass,
        }
    }
}

pub struct TestCase<X: 'static> {
    pub name: &'static str,
    pub test: AsyncFn<X, Result<()>>,
}
#[async_trait]
impl<Args> Test<Args> for TestCase<Args>
where
    Args: Send + Sync,
{
    async fn run(&self, args: Args) -> TestResult {
        self.test.run(args).await
    }
}
impl<Args> Debug for TestCase<Args> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}

// enables building of runner from init_ctx
pub async fn start<R, C, Args>(args: Args)
where
    C: Ctx<Base = Args> + TestSet + ChildTypesFn + Send + Sync + Clone + 'static,
    R: Runner<Base = C, Out = Result<()>> + Send + Sync,
    ChildTypes<C>: MapFn<Driver<R>, C>,
{
    let init_ctx = C::build(args).await;
    let runner = R::new(init_ctx.clone());
    let mut driver = Driver::new(runner);
    driver.run_ctx(init_ctx).await;
}

pub struct Driver<R> {
    runner: R,
}
impl<R> Driver<R>
where
    R: Runner<Out = Result<()>> + Send + Sync,
{
    fn new(runner: R) -> Self {
        Self { runner }
    }
    async fn run_ctx<C>(&mut self, ctx: C)
    where
        C: TestSet + ChildTypesFn + Send + Sync + Clone + 'static,
        ChildTypes<C>: MapFn<Self, C>,
    {
        let tests = C::tests();
        self.runner.run(&ctx, tests).await.unwrap();
        tmap::<Self, C, ChildTypes<C>>(self, ctx).await;
    }
}
#[async_trait]
impl<T, R, C> FnT<T, C> for Driver<R>
where
    T: Ctx<Base = C> + TestSet + ChildTypesFn + Send + Sync + Clone + 'static,
    R: Runner<Out = Result<()>> + Send + Sync,
    C: Ctx + Send + 'static,
    ChildTypes<T>: MapFn<Self, T>,
{
    type Out = Arc<Result<()>>;
    async fn call(&mut self, base: C) -> Self::Out {
        let ctx = T::build(base).await;
        self.run_ctx(ctx).await;
        Arc::new(Ok(()))
    }
}
