#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]

use runrun_derive::{run_ctx, run_test, collect};
use runrun::{tlist, TList};

pub struct Ctx0;
pub struct Ctx1;

#[async_trait]
pub trait Ctx {
    type Base;
    async fn build(base: Self::Base) -> Self;
}

#[test]
pub fn tests() {
    // #[run_test]
    // pub async fn foo(ctx: Ctx0) {}
    // #[run_ctx]
    // impl Ctx for Ctx0 {
    //     type Base = String;
    //     async fn build(args: Self::Base) -> Self {
    //         Self
    //     }
    // }
    // collect!();
    // tlist![1,2,3];
    // type T = TList!(usize, u8);
}
