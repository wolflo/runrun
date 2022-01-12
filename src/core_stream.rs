use async_trait::async_trait;
use std::{
    fmt::Debug,
};

use crate::types::{ChildTypesFn, AsyncFn,};


// Used by the MapT type to bound the types that can be mapped over. Ideally
// we would be able to map an arbitrary FnT, but unfortunately we can only map
// FnT's that share the same trait bounds.
pub trait MapBounds<Args>:
    Ctx<Base = Args> + TestSet<'static> + ChildTypesFn + Unpin + Clone + Send + Sync + 'static
{
}
impl<T, Args> MapBounds<Args> for T where
    T: Ctx<Base = Args> + TestSet<'static> + ChildTypesFn + Unpin + Clone + Send + Sync + 'static
{
}

#[async_trait]
pub trait Ctx {
    type Base;
    async fn build(base: Self::Base) -> Self;
}
pub trait TestSet<'a> {
    fn tests() -> &'a [&'a dyn Test<'a, Self>];
}
#[async_trait]
pub trait Test<'a, Args>: Send + Sync {
    async fn run(&self, args: Args) -> TestRes<'a>;
    fn skip(&self) -> TestRes<'a> {
        Default::default()
    }
}
#[derive(Debug, Clone)]
pub struct TestRes<'a> {
    pub status: Status,
    pub trace: &'a (dyn Debug + Sync),
}
#[derive(Debug, Clone, Copy)]
pub enum Status {
    Pass,
    Fail,
    Skip,
}

impl Default for Status {
    fn default() -> Self {
        Self::Skip
    }
}
impl Default for TestRes<'_> {
    fn default() -> Self {
        Self {
            status: Default::default(),
            trace: &"",
        }
    }
}

#[derive(Clone)]
pub struct TestCase<'a, Args, Res> {
    pub name: &'static str,
    pub test: &'a AsyncFn<'a, Args, Res>,
}
#[async_trait]
impl<'a, Args, Res> Test<'a, Args> for TestCase<'_, Args, Res>
where
    Args: Send + Sync + 'static,
    Res: Test<'a, ()>,  // could also pass args to result.run()
{
    async fn run(&self, args: Args) -> TestRes<'a> {
        println!("{}", self.name);
        (self.test)(args).await.run(()).await
    }
}
// () is a trivially passing Test
#[async_trait]
impl<'a, Args: Send + 'static> Test<'a, Args> for ()
{
    async fn run(&self, _args: Args) -> TestRes<'a> {
        TestRes {
            status: Status::Pass,
            trace: &"",
        }
    }
}
// true is a passing Test, false is a failing Test
#[async_trait]
impl<'a, Args: Send + 'static> Test<'a, Args> for bool
{
    async fn run(&self, _args: Args) -> TestRes<'a> {
        let status = if *self { Status::Pass } else { Status::Fail };
        TestRes {
            status,
            trace: &"",
        }
    }
}
#[async_trait]
impl<'a, T, E, Args> Test<'a, Args> for Result<T, E>
where
    T: Test<'a, Args> + Send + Sync,
    E: Into<Box<dyn std::error::Error>> + Send + Sync + 'a,
    Args: Send + Sync + 'static,
{
    async fn run(&self, args: Args) -> TestRes<'a> {
        match self {
            Ok(r) => r.run(args).await,
            Err(e) => TestRes {
                status: Status::Fail,
                trace: &"Test of Err value. We should provide real traces for these."
                // trace: (*e).clone().into(),
            },
        }
    }
}
#[async_trait]
impl<'a, T, Args> Test<'a, Args> for &T
where
    T: Test<'a, Args> + ?Sized + Send + Sync,
    Args: Send + Sync + 'static,
{
    async fn run(&self, args: Args) -> TestRes<'a> {
        (**self).run(args).await
    }
    fn skip(&self) -> TestRes<'a> {
        (**self).skip()
    }
}
