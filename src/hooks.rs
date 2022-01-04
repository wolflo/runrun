use anyhow::Result;
use async_recursion::async_recursion;
use async_trait::async_trait;
use std::marker::PhantomData;

use crate::core::{Runner, Status, Test, DefaultTest, TestResult};

#[async_trait]
pub trait Hooks: Clone + Send + Sync {
    type Base;
    fn new(base: Self::Base) -> Self;
    fn before<Args>(&mut self) -> &dyn Test<Args> where Args: Send + 'static { &DefaultTest }
    fn before_each<Args>(&mut self) -> &dyn Test<Args> where Args: Send + 'static { &DefaultTest }
    fn after_each<Args>(&mut self) -> &dyn Test<Args> where Args: Send + 'static { &DefaultTest }
    fn after<Args>(&mut self) -> &dyn Test<Args> where Args: Send + 'static { &DefaultTest }
    // fn after<Args, T>(&mut self) -> T
    // where
    //     T: Test<Args>;
    // fn before_each<Args, T>(&mut self) -> T
    // where
    //     T: Test<Args>;
    // fn after_each<Args, T>(&mut self) -> T
    // where
    //     T: Test<Args>;

    //     async fn before(&mut self) -> TestResult {
    //         TestResult {
    //             output: TestOutput { status: Status::Pass },
    //             trace: Box::new("before hook"),
    //         }
    //     }
    //     async fn before_each(&mut self) -> TestResult {
    //         TestResult {
    //             output: TestOutput { status: Status::Pass },
    //             trace: Box::new("before_each hook"),
    //         }
    //     }
    //     async fn after_each(&mut self) -> TestResult {
    //         TestResult {
    //             output: TestOutput { status: Status::Pass },
    //             trace: Box::new("after_each hook"),
    //         }
    //     }
    //     async fn after(&mut self) -> TestResult {
    //         TestResult {
    //             output: TestOutput { status: Status::Pass },
    //             trace: Box::new("after hook"),
    //         }
    //     }
}

#[derive(Clone)]
pub struct HookRunner<H> {
    pub hooks: H,
}

pub enum Res {
    Test(TestResult),
    Block(Vec<BlockRes>),
}
pub struct BlockRes {
    pub before: TestResult,
    pub test: Res,
    pub after: TestResult,
}

pub enum Action<Args, T, Ret> {
    Test(T),
    Block(Vec<Block<Args, T, Ret>>),
}
pub struct Block<Args, T, Ret> {
    pub before: Box<dyn Test<Args>>,
    pub test: Action<Args, T, Ret>,
    pub after: Box<dyn Test<Args>>,
    pub _phantom: std::marker::PhantomData<(Args, Ret)>,
}

impl<Args, T, Ret> Action<Args, T, Ret>
where
    Args: Send + Sync + Clone + 'static,
    T: Test<Args> + Send + Sync + 'static,
    Ret: Send + Sync,
{
    async fn run(&self, args: &Args) -> Res {
        match self {
            Self::Test(t) => Res::Test(t.run(args.clone()).await),
            Self::Block(bs) => {
                let mut block_res = vec![];
                for b in bs {
                    block_res.push(b.run_block(args).await)
                }
                Res::Block(block_res)
            }
        }
    }
    fn skip(&self, args: &Args) -> Res {
        match self {
            Self::Test(t) => Res::Test(t.skip(args.clone())),
            Self::Block(bs) => Res::Block(bs.iter().map(|b| b.skip_block(args)).collect()),
        }
    }
}

impl<'a, Args, T, Ret> Block<Args, T, Ret>
where
    Args: Send + Sync + Clone + 'static,
    T: Test<Args> + Send + Sync + 'static,
    Ret: Send + Sync,
{
    fn new(before: Box<T>, test: Action<Args, T, Ret>, after: Box<T>) -> Self {
        Self {
            before,
            test,
            after,
            _phantom: PhantomData,
        }
    }
    #[async_recursion]
    async fn run_block(&self, args: &Args) -> BlockRes {
        let before_res = self.before.run(args.clone()).await;
        let (test_res, after_res) = match before_res.output.status {
            Status::Fail => {
                let test_res = self.test.skip(args);
                let after_res = self.after.skip(args.clone());
                (test_res, after_res)
            }
            _ => {
                let test_res = self.test.run(args).await;
                let after_res = self.after.run(args.clone()).await;
                (test_res, after_res)
            }
        };
        BlockRes {
            before: before_res,
            test: test_res,
            after: after_res,
        }
    }
    fn skip_block(&self, args: &Args) -> BlockRes {
        BlockRes {
            before: self.before.skip(args.clone()),
            test: self.test.skip(args),
            after: self.after.skip(args.clone()),
        }
    }
}

impl<H: Hooks + 'static> HookRunner<H> {
    fn to_block<Args, T, Ret>(&mut self, tests: &'static [T]) -> Block<Args, &dyn Test<Args>, Ret>
    where
        Args: Send + Sync + Clone + 'static,
        T: Test<Args> + Send + Sync + 'static,
        Ret: Send + Sync,
    {
        let before = self.hooks.clone().before();
        let after = self.hooks.clone().after();
        let before_each = self.hooks.clone().before_each();
        let after_each = self.hooks.clone().after_each();
        let inner: Vec<Block<Args, &dyn Test<Args>, Ret>> = tests
            .iter()
            .map(|t|
                Block::new(Box::new(before_each), Action::Test(t), Box::new(after_each))
            )
            .collect();
        Block::new(Box::new(before), Action::Block(inner), Box::new(after))
    }
}

// Want to know:
// - num skipped
// - which tests were skipped
// - which tests/hooks failed
// for each block:
// - skipped
// - failed
// - passed

#[async_trait]
impl<H: Hooks + 'static> Runner for HookRunner<H> {
    type Out = BlockRes;
    type Base = H::Base;
    fn new(base: Self::Base) -> Self {
        let hooks = H::new(base);
        Self { hooks }
    }
    async fn run<Args, T>(&mut self, args: &Args, tests: &'static [T]) -> Self::Out
    where
        Args: Send + Sync + Clone + 'static,
        T: Test<Args> + Send + Sync + 'static,
    {
        let mut r = self.clone();
        let block: Block<_, _, Result<()>> = r.to_block(tests);
        block.run_block(args).await
    }
}
