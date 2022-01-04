use anyhow::Result;
use async_trait::async_trait;
use futures::{future::Future, FutureExt};
use std::{fmt, fmt::Debug, ops::Deref, panic::AssertUnwindSafe, sync::Arc};

use crate::ty::{tmap, ChildTypes, ChildTypesFn, FnTMut, MapFn};

pub type AsyncOutput<R> = std::pin::Pin<Box<dyn Future<Output = R> + Send>>;
pub type AsyncFn<X, Y> = dyn Fn(X) -> AsyncOutput<Y> + Send + Sync;

#[async_trait]
pub trait Runner {
    type Out;
    type Base;
    fn new(base: Self::Base) -> Self;
    async fn run<Args, T>(&mut self, args: &Args, tests: &'static [T]) -> Self::Out
    where
        Args: Send + Sync + Clone + 'static,
        T: Test<Args> + Send + Sync;
}
#[async_trait]
pub trait Ctx {
    type Base;
    async fn build(base: Self::Base) -> Self;
}
pub trait TestSet {
    fn tests() -> &'static [&'static dyn Test<Self>];
}

#[derive(Debug)]
pub struct TestOutput {
    pub status: Status,
}
pub struct TestResult {
    pub output: TestOutput,
    pub trace: Box<dyn Debug + Send + Sync>,
}
#[async_trait]
pub trait Test<Args>: Testable<Args> + Trace + Send + Sync {
    async fn run(&self, args: Args) -> TestResult;
    fn skip(&self, _args: Args) -> TestResult {
        TestResult::default()
    }
}
pub trait Trace {
    fn trace(&self) -> Box<dyn Debug + Send + Sync>;
}
#[async_trait]
impl<T, Args> Test<Args> for T
where
    T: Testable<Args> + Trace + Send + Sync,
    Args: 'static + Send,
{
    async fn run(&self, args: Args) -> TestResult {
        TestResult {
            output: self.resolve(args).await,
            trace: self.trace(),
        }
    }
    fn skip(&self, args: Args) -> TestResult {
        TestResult {
            output: self.as_skip(args),
            trace: self.trace(),
        }
    }
}
#[derive(Debug, Clone, Copy)]
pub enum Status {
    Pass,
    Fail,
    Skip,
}
#[async_trait]
pub trait Testable<Args> {
    async fn resolve(&self, args: Args) -> TestOutput;
    fn as_skip(&self, _args: Args) -> TestOutput {
        TestOutput {
            status: Status::Skip,
        }
    }
}
#[async_trait]
impl<Args, Y> Testable<Args> for AsyncFn<Args, Y>
where
    Args: Send + Sync,
    Y: Testable<()> + Send + Sync,
{
    async fn resolve(&self, args: Args) -> TestOutput {
        let res = AssertUnwindSafe(self(args))
            .catch_unwind()
            .await
            .map_err(|e| anyhow::anyhow!("Test panic: {:?}", e));
        res.resolve(()).await
    }
}
// Any Result resolves either to a Failing test case or to the result of
// testing the contained value
#[async_trait]
impl<T, E, Args> Testable<Args> for Result<T, E>
where
    Args: Send + Sync + 'static,
    T: Testable<Args> + Sync,
    E: Sync + Debug,
{
    async fn resolve(&self, args: Args) -> TestOutput {
        match self {
            Ok(r) => r.resolve(args).await,
            Err(e) => {
                println!("Test failure! Err: {:?}", e);
                TestOutput {
                    status: Status::Fail,
                }
            }
        }
    }
}
// () is a trivially passing Test
#[async_trait]
impl<Args> Testable<Args> for ()
where
    Args: Send + 'static,
{
    async fn resolve(&self, _args: Args) -> TestOutput {
        TestOutput {
            status: Status::Pass,
        }
    }
}
#[async_trait]
impl<Args, T> Testable<Args> for &T
where
    T: Testable<Args> + ?Sized + Send + Sync,
    Args: Send + 'static,
{
    async fn resolve(&self, args: Args) -> TestOutput {
        (**self).resolve(args).await
    }
}
impl<T> Trace for &T
where
    T: Trace + ?Sized,
{
    fn trace(&self) -> Box<dyn Debug + Send + Sync> {
        (**self).trace()
    }
}

pub struct TestCase<X: 'static> {
    pub name: &'static str,
    pub test: AsyncFn<X, Result<()>>,
}
#[async_trait]
impl<Args> Testable<Args> for TestCase<Args>
where
    Args: Send + Sync,
{
    async fn resolve(&self, args: Args) -> TestOutput {
        self.test.resolve(args).await
    }
}
impl<Args> Debug for TestCase<Args> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}
impl<Args> Trace for TestCase<Args> {
    fn trace(&self) -> Box<dyn Debug + Send + Sync> {
        Box::new(self.name)
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
    pub fn new(runner: R) -> Self {
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
impl<T, R, C> FnTMut<T, C> for Driver<R>
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

impl Default for Status {
    fn default() -> Self {
        Self::Skip
    }
}
impl Default for TestOutput {
    fn default() -> Self {
        Self {
            status: Status::default(),
        }
    }
}
impl Default for TestResult {
    fn default() -> Self {
        Self {
            output: TestOutput::default(),
            trace: default_trace(),
        }
    }
}
pub fn default_trace() -> Box<&'static str> {
    Box::new("default trace")
}
pub struct DefaultTest;
impl Trace for DefaultTest {
    fn trace(&self) -> Box<dyn Debug + Send + Sync> {
        default_trace()
    }
}
#[async_trait]
impl<Args> Testable<Args> for DefaultTest
where
    Args: Send + 'static,
{
    async fn resolve(&self, _args: Args) -> TestOutput {
        TestOutput::default()
    }
    fn as_skip(&self, _args: Args) -> TestOutput {
        TestOutput::default()
    }
}
// #[async_trait]
// impl<Args> Test<Args> for DefaultTest {
//     async fn run(&self, _args: Args) -> TestResult {
//         TestResult::Default()
//     }
//     fn skip(&self, _args: Args) -> TestResult {
//         TestResult::Default()
//     }

// }
