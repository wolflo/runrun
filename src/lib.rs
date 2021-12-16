#![feature(generic_associated_types)]
#![allow(unused_variables)]
#![allow(dead_code)]
#![allow(unused_imports)]

use anyhow::Result;
use async_trait::async_trait;
use futures::future::Future;

pub type AsyncRes<R> = std::pin::Pin<Box<dyn Future<Output = R> + Send>>;
pub type Act<T> = fn(T) -> AsyncRes<Result<()>>;
pub type Move<B, C> = fn(B, &dyn FnOnce(C) -> Box<dyn Runner<C>>);

#[async_trait]
pub trait Ctx: Clone + Send + Sync {
    type Base: Ctx; // the Ctx to build from
    async fn create(base: Self::Base) -> Result<Self>;
}

// A runner runs a single set of actions
pub trait Runner<C: Ctx> {
    fn run(&self, ctx: C, acts: &[Act<C>]);
}
// Runner<B> -> C -> Runner<C>
pub trait RunnerBuilder {
    type Runner<T: Ctx>: Runner<T> + 'static;
    // fmap . const
    fn build<B, C>(&mut self, prev: &Self::Runner<B>, ctx: C) -> Self::Runner<C>
    where
        B: Ctx,
        C: Ctx;
}


// Driver controls transitions between contexts and dispatches Runners
struct Driver<RB> { builder: RB }
impl<RB: RunnerBuilder> Driver<RB> {
    async fn drive<B, C>(&mut self, runner: &RB::Runner<B>, ctx: B, moves: &[Move<B, C>])
    where
        B: Ctx + Clone + Send + Sync,
        C: Ctx<Base = B>,
    {
        for m in moves {
            // want fn(prev_runner, ctx) -> Runner<ctx>
            // need to partially apply to fn(ctx) -> Runner<ctx>
            let partial = &|ctx| Box::new(self.builder.build(&runner, ctx)) as Box<dyn Runner<C>>;
            m(ctx.clone(), partial);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn it_works() {}
}
