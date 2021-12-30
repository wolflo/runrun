use anyhow::Result;
use async_trait::async_trait;

use crate::core::*;

#[async_trait]
pub trait Hooks: Clone + Send + Sync {
    type Base;
    fn new(base: &Self::Base) -> Self;
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
    fn new(base: &Self::Base) -> Self {
        let hooks = H::new(base);
        Self { hooks }
    }
    async fn run<T>(&mut self, ctx: &T, tests: &'static [fn(T)]) -> Self::Out
    where
        T: Sync + Clone,
    {
        self.hooks.before().await?;
        for t in tests {
            self.hooks.before_each().await?;
            t(ctx.clone());
            self.hooks.after_each().await?;
        }
        self.hooks.after().await?;
        Ok(())
    }
}
