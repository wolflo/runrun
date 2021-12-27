use anyhow::Result;
use async_trait::async_trait;
use futures::future::Future;
use std::marker::PhantomData;

pub struct TNil;
pub struct TCons<H, T>(PhantomData<(H, T)>);

// have a list of types
// need a fn (trait) that takes a list of types and a (list of?) function on those types and accumulated the results
// Runner should be built from initial state that user designates with #[run_state(init)]
//  - should take only a context and a list of tests -- see proptest lib
pub trait Runner {
    type Ctx;
    fn run(ctx: &Self::Ctx, acts: &'static [fn(Self::Ctx)]) -> Result<()>;
    // build from previous runner?
}

// Need action on binder to:
// Create state from previous state
// Build runner
// Get and run tests
// Get and bind children

pub trait ActSet {
    type Ctx;
    type TPtrs;
    fn tests() -> &'static [fn(Self::Ctx)];
}

struct NullCtx;
type CtxInit = NullCtx;

// take N elements of a TList
pub trait Take<const N: usize> {
    type Ret;
}
impl Take<0> for TNil {
    type Ret = TNil;
}
impl<H, T, const N: usize> Take<N> for TCons<H, T>
where
    T: Take<{ N - 1 }>,
{
    type Ret = TCons<H, <T as Take<{ N - 1 }>>::Ret>;
}

pub fn main() {
    // CtxInit::bind();
}
