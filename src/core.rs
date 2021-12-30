use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;

use crate::ty::*;

#[async_trait]
pub trait Runner {
    type Out;
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

pub struct Driver<R> {
    runner: R,
}
impl<R> Driver<R> {
    fn new(runner: R) -> Self {
        Self { runner }
    }
}
#[async_trait]
impl<T, R, C> Func<T, C> for Driver<R>
where
    T: Ctx<Base = C> + TestSet + ChildTypesFn + Send + Sync + 'static,
    R: Runner + Send + Sync,
    C: Ctx + Send + Clone + 'static,
    ChildTypes<T>: MapFn<Self, T>,
{
    type Out = Arc<Result<()>>;
    async fn call(&mut self, base: C) -> Self::Out {
        let ctx = T::build(base).await;
        let tests = T::tests();
        self.runner.run(&ctx, tests).await;
        ChildTypes::<T>::map(self, ctx).await;
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
