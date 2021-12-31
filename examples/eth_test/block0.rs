use runrun::core::TestSet;

use crate::init::Ctx0;

// -- Macro generated
use linkme::distributed_slice;
#[distributed_slice]
pub static TESTS_ON_CTX0: [fn(Ctx0)] = [..];
#[distributed_slice(TESTS_ON_CTX0)]
pub static __TC01: fn(Ctx0) = |_| println!("test 1");
impl TestSet for Ctx0 {
    fn tests() -> &'static [fn(Self)] {
        &TESTS_ON_CTX0
    }
}
