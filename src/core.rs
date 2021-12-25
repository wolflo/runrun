use anyhow::Result;
use async_trait::async_trait;
use futures::future::Future;

pub type AsyncRes<R> = std::pin::Pin<Box<dyn Future<Output = R> + Send>>;
pub type Act<T> = fn(T) -> AsyncRes<Result<()>>;
pub type Move<B> = fn(B, Builder);


pub trait ActSet {
    type Ctx: Ctx;
    fn acts(&self) -> &'static [Act<Self::Ctx>] {
        &[]
    }
    fn spurs(&self) -> &'static [Move<Self::Ctx>] { &[] }
}

#[async_trait]
pub trait Ctx: Clone + Send + Sync {
    type Base: Ctx; // the Ctx to build from
    async fn create(base: Self::Base) -> Result<Self>;
}

// A runner runs a single set of actions
#[async_trait]
pub trait Runner<C: Ctx> {
    async fn run(&self, ctx: C, acts: &[Act<C>]) -> Result<()>;
    // fn new(self, ctx: C) -> &'static dyn Runner<C>;
}
// Runner<B> -> C -> Runner<C>
pub trait RunnerBuilder {
    type Runner<T: Ctx>: Runner<T> + 'static;
    // fmap . const
    fn build<B, C>(&mut self, prev: &Self::Runner<B>, ctx: C) -> Self::Runner<C>
    where
        B: Ctx,
        C: Ctx<Base = B>;
}

pub struct NullRunner;
pub struct NullRunnerBuilder;
#[async_trait]
impl<C: Ctx> Runner<C> for NullRunner { async fn run(&self, ctx: C, acts: &[Act<C>]) -> Result<()> { todo!() }}
impl RunnerBuilder for NullRunnerBuilder {
    type Runner<T: Ctx> = NullRunner;
    fn build<B, C>(&mut self, prev: &Self::Runner<B>, ctx: C) -> Self::Runner<C>
    where
        B: Ctx,
        C: Ctx<Base = B>
    {
        NullRunner
    }
}

// Driver controls transitions between contexts and dispatches Runners
struct DriverStruct<RB> {
    builder: RB,
}
impl<RB: RunnerBuilder> DriverStruct<RB> {
    async fn drive<B, C>(&mut self, runner: &RB::Runner<B>, ctx: B, moves: &[Move<B>])
    where
        B: Ctx + Clone + Send + Sync + ActSet + 'static,
        C: Ctx<Base = B> + ActSet<Ctx=C> + 'static,
    {
    }
}
// need to take in a ctx and do all dispatch for it
impl<C: Ctx, RB: RunnerBuilder> FnOnce<(C,)> for DriverStruct<RB> {
    type Output = ();
    extern "rust-call" fn call_once(self, ctx: (C,)) -> Self::Output {
    }
}

// What I want to pass in to runrun
// https://stackoverflow.com/questions/37606035/pass-generic-function-as-argument
pub fn passmein<C: Ctx>() {}
pub struct Builder;
pub trait Zoo {}
impl Builder {
    // need to be able to set one generic type before passing in and one on build
    pub fn build<C: Ctx>(&self) -> &dyn FnOnce(C) {
        // fn built<C: Ctx>(ctx: C) {
        // }
        // built
        &DriverStruct { builder: NullRunnerBuilder }
    }
}


// If we can somehow make something with the type signature fn() that
// takes the destination Ctx as a generic and already knows about
// the driver, that's what we need.
struct Foo<T>(T);
impl<T> FnOnce<()> for Foo<T> {
    type Output = ();
    extern "rust-call" fn call_once(self, args: ()) -> Self::Output {}
}

pub async fn runrun<B, C>(base: B, builder: Builder) -> Result<()>
where
    C: Ctx<Base = B>,
{
    // let ctx = C::create(base).await?;
    // f: fn(C). So this allows us to create a fn which takes in a C without
    // polluting the type signature of runrun
    let f = builder.build::<C>();
    // driver(ctx);
    Ok(())
}


#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn it_works() {}
}
