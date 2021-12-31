use runrun::core::{Test, TestSet};

use crate::init::Ctx0;

async fn test1(ctx: Ctx0) {
    println!("test 1");
}
// -- Macro generated
use linkme::distributed_slice;
#[distributed_slice]
pub static TESTS_ON_CTX0: [Test<Ctx0>] = [..];
#[distributed_slice(TESTS_ON_CTX0)]
pub static __TC01: Test<Ctx0> = &|x| Box::pin(test1(x));
impl TestSet for Ctx0 {
    fn tests() -> &'static [Test<Self>] {
        &TESTS_ON_CTX0
    }
}
