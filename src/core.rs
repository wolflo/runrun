use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;

use crate::ty::*;

#[async_trait]
pub trait Runner {
    type Out;
    type Base;
    fn new(ctx: &Self::Base) -> Self;
    async fn run<T>(&self, ctx: &T, tests: &'static [fn(T)]) -> Self::Out;
}
#[async_trait]
pub trait Ctx {
    type Base;
    async fn build(base: Self::Base) -> Self;
}
pub trait TestSet {
    fn tests() -> &'static [fn(Self)];
}

// enables building of runner from init_ctx
async fn start<R, C>()
where
    C: Ctx<Base = ()> + TestSet + ChildTypesFn + Send + 'static,
    R: Runner<Base=C> + Send + Sync,
    ChildTypes<C>: MapFn<Driver<R>, C>
{
    let init_ctx = C::build(()).await;
    let runner = R::new(&init_ctx);
    let mut driver = Driver::new(runner);
    driver.run_ctx(init_ctx).await;
}

pub struct Driver<R> {
    runner: R,
}
impl<R> Driver<R> where R: Runner + Send + Sync {
    fn new(runner: R) -> Self {
        Self { runner }
    }
    async fn run_ctx<C>(&mut self, ctx: C)
    where
        C: TestSet + ChildTypesFn + 'static,
        ChildTypes<C>: MapFn<Self, C>,
    {
        let tests = C::tests();
        self.runner.run(&ctx, tests).await;
        tmap::<Self, C, ChildTypes::<C>>(self, ctx).await;
    }
}
#[async_trait]
impl<T, R, C> Func<T, C> for Driver<R>
where
    T: Ctx<Base = C> + TestSet + ChildTypesFn + Send + Sync + 'static,
    R: Runner + Send + Sync,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test() {
        println!("test");
    }
}
