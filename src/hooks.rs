use anyhow::Result;
use async_trait::async_trait;
use std::fmt::Debug;

use crate::core::{Runner, Status, Test};

#[async_trait]
pub trait Hooks: Clone + Send + Sync {
    type Base;
    fn new(base: Self::Base) -> Self;
    async fn before(&mut self) -> Result<()> {
        Ok(())
    }
    async fn before_each(&mut self) -> Result<()> {
        Ok(())
    }
    async fn after_each(&mut self) -> Result<()> {
        Ok(())
    }
    async fn after(&mut self) -> Result<()> {
        Ok(())
    }
}

pub struct HookRunner<H> {
    pub hooks: H,
}
#[async_trait]
impl<H: Hooks> Runner for HookRunner<H> {
    type Out = Result<()>;
    type Base = H::Base;
    fn new(base: Self::Base) -> Self {
        let hooks = H::new(base);
        Self { hooks }
    }
    async fn run<C, T>(&mut self, ctx: &C, tests: &'static [&T]) -> Self::Out
    where
        C: Send + Sync + Clone,
        T: Test<C> + Debug + ?Sized + Send + Sync,
    {
        let mut pass = 0;
        let mut fail = 0;
        self.hooks.before().await?;
        for t in tests {
            self.hooks.before_each().await?;
            let res = t.run(ctx.clone()).await;
            match res.status {
                Status::Pass => {
                    pass += 1;
                }
                Status::Fail => {
                    fail += 1;
                    println!("Failing test: {:?}", t);
                }
            };
            self.hooks.after_each().await?;
        }
        self.hooks.after().await?;
        println!("Passing tests: {}", pass);
        println!("Failing tests: {}", fail);
        Ok(())
    }
}
