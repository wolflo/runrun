use anyhow::Result;
use async_trait::async_trait;
use futures::future::Future;
use std::sync::Arc;

use crate::ty::{tmap, ChildTypes, ChildTypesFn, MapFn, TFn};

pub struct TestResult {
    status: Status,
}
enum Status {
    Pass,
    Fail,
}
// Testable impl will do things like catch panics and provide diagnostic info
#[async_trait]
pub trait Testable<Args> {
    async fn result(&self, args: Args) -> TestResult;
}
// impl<T, E> Testable for Result<T, E>
// where
//     A: Testable,
//     E: Debug + 'static,
// {

// }
// pub type Test<Args> = &'static (dyn Testable<Args> + Sync);
pub type AsyncResult<R> = std::pin::Pin<Box<dyn Future<Output = R> + Send>>;
pub type Test<T> = &'static (dyn Fn(T) -> AsyncResult<()> + Sync);

#[async_trait]
pub trait DebugTrait {
    async fn debug(&self);
}

#[async_trait]
pub trait Runner {
    type Out;
    type Base;
    fn new(base: Self::Base) -> Self;
    async fn run<T>(&mut self, ctx: &T, tests: &'static [Test<T>]) -> Self::Out
    where
        T: Send + Sync + Clone;
}
#[async_trait]
pub trait Ctx {
    type Base;
    async fn build(base: Self::Base) -> Self;
}
pub trait TestSet {
    fn tests() -> &'static [Test<Self>];
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
impl<T, R, C> TFn<T, C> for Driver<R>
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
