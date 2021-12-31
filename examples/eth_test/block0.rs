use anyhow::Result;
use runrun::core::{TestCase, TestSet, Testable};

use crate::init::Ctx0;

async fn test_not_paused(ctx: Ctx0) -> Result<()> {
    println!("test_not_paused");
    let paused = ctx.token.paused().call().await?;
    assert!(!paused);
    Ok(())
}
async fn test_supply(ctx: Ctx0) -> Result<()> {
    println!("test_supply");
    let supply = ctx.token.total_supply().call().await?;
    assert_eq!(supply, 0.into());
    Ok(())
}
// -- Macro generated
use linkme::distributed_slice;
#[distributed_slice]
pub static TESTS_ON_CTX0: [Testable<Ctx0>] = [..];
#[distributed_slice(TESTS_ON_CTX0)]
pub static __TC01: Testable<Ctx0> = &TestCase {
    name: "test_not_paused",
    test: &|x| Box::pin(test_not_paused(x)),
};
#[distributed_slice(TESTS_ON_CTX0)]
pub static __TC02: Testable<Ctx0> = &TestCase {
    name: "test_supply",
    test: &|x| Box::pin(test_supply(x)),
};

impl TestSet for Ctx0 {
    fn tests() -> &'static [Testable<Self>] {
        &TESTS_ON_CTX0
    }
}
