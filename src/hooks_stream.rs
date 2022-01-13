use async_trait::async_trait;
use futures::{
    stream::StreamExt,
    stream,
};

use crate::{
    core_stream::{MapBounds, Status, TestRes, Test},
    types::{ChildTypes, FnOut, FnT, MapStep, MapT},
};

#[derive(Debug, Clone, Copy, Default)]
pub struct NoHook;
#[async_trait]
impl<'a> Hook<'a> for NoHook {
    async fn pre(&mut self) -> TestRes<'a> {
        Default::default()
    }
    async fn post(&mut self) -> TestRes<'a> {
        Default::default()
    }
}
impl NoHook {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
pub trait Hook<'a> {
    async fn pre(&mut self) -> TestRes<'a>;
    async fn post(&mut self) -> TestRes<'a>;
}
#[derive(Debug, Clone)]
pub struct HookRunner<H> {
    hooks: H,
}
impl<H> HookRunner<H> {
    pub fn new(hooks: H) -> Self {
        Self { hooks }
    }
}

pub async fn run_test<'a, T, Args, H>(t: T, args: Args, mut hooks: H) -> TestRes<'a>
where
    H: Hook<'static>,
    T: Test<'a, Args>,
{
    let pre_res = hooks.pre().await;
    // If the pre hook failed, return it as the test result, skipping test and post hook
    if let Status::Fail = pre_res.status {
        return pre_res
    }
    let test_res = t.run(args).await;
    // If the test failed, return the result and don't run the post hook
    if let Status::Fail = test_res.status {
        return test_res
    }
    let post_res = hooks.post().await;
    // If the test passed but the post hook failed, return the post hook failure
    if let Status::Fail = post_res.status {
        return post_res
    }
    // Everything passed. Return the test result
    test_res
}
#[async_trait]
impl<Args, H> FnT<Args> for HookRunner<H>
where
    Args: Send + 'static,
    H: Hook<'static> + Unpin + Clone + Send + Sync,
{
    type Output = ();
    async fn call<T>(&self, args: Args) -> FnOut<Self, Args>
    where
        Self: FnT<T>,
        T: MapBounds<Args>,
        ChildTypes<T>: MapStep<Self, T>,
    {
        let ctx = T::build(args).await;
        let tests = T::tests();

        let mut test_res = tests.iter().map(|&t| run_test(t, ctx.clone(), self.hooks.clone()));

        let mut pass = 0;
        let mut fail = 0;
        let mut skip = 0;
        while let Some(test_res) = test_res.next() {
            match test_res.await.status {
                Status::Pass => pass += 1,
                Status::Fail => fail += 1,
                Status::Skip => skip += 1,
            }
        }
        println!("tests passed : {}", pass);
        println!("tests failed : {}", fail);
        println!("tests skipped: {}", skip);

        let child_iter = MapT::<_, _, ChildTypes<T>>::new(self, ctx);
        let mut child_stream = stream::iter(child_iter);
        while let Some(_) = child_stream.next().await { }
    }
}
