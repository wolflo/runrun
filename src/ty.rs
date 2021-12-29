use anyhow::Result;
use async_trait::async_trait;
use futures::future::Future;
use std::marker::PhantomData;

// Need to bind a single fn across a TList, but a simple ptr to a generic fn will be
// monomorphized for the first type so won't work.
// - impl FnOnce for a type?
// - function builder
// Python fixtures are really just functions that return a value. Could that be a more general model
// than states?
// What if instead of a TList of Ctxs built from current Ctx, children are an HList of fns that take
// the current Ctx as an arg and return their ctx / result?
// Could be useful for generation of testcases and expected result lists.
// What I want is pretty close to https://github.com/la10736/rstest, except I want to be able to
// run a particular fixture only once. Could we define fixture fns that do something different (e.g.
// reset state) when they are called the second time?

// Need to make something of kind * -> *, but abstract over arity
// * -> * -> *

// have a list of types
// need a fn (trait) that takes a list of types and a (list of?) function on those types and accumulated the results
// Runner should be built from initial state that user designates with #[run_state(init)]
//  - should take only a context and a list of tests -- see proptest lib
pub trait Runner<T> {
    fn run(&self, ctx: &T, tests: &'static [fn(T)]) -> Result<()>;

    // Do we want to be able to share state across runners? i.e. build from previous runner
    // Do we want to be able to change the Runner type throughout a test suite?
    // fn build<B, R: Self<B>>(base: R) -> Self;
    // fn build<C, R: Runner<C>>(&self, ctx: C) -> R;
    fn new(ctx: &T) -> Self;
}
pub trait Ctx {
    type Base;
    fn build(base: Self::Base) -> Self;
}

// Need action on binder to:
// Create state from previous state
// Build runner
// Get and run tests
// Get and bind children

pub trait TestSet {
    type Children;
    // type TPtrs;
    fn tests() -> &'static [fn(Self)];
}

// Type-level represenation of natural numbers (see Peano numbers)
// generic_const_exprs is unstable or we could just use usize as a const generic
struct Zero;
struct Succ<N>(PhantomData<N>);
type One = Succ<Zero>;

// Get the predecessor (N - 1) of any nonzero Nat
type Pred<N> = <N as PredFn>::Out;
pub trait PredFn {
    type Out;
}
impl<N> PredFn for Succ<N> {
    type Out = N;
}

// Take N elements of a TList
type Take<N, T> = <T as TakeFn<N>>::Out;
type Drop<N, T> = <T as TakeFn<N>>::Rest;
type Drop1<T> = Drop<One, T>;
type Take1<T> = Take<One, T>;
type TakeDrop<N, T> = (Take<N, T>, Drop<N, T>);
pub trait TakeFn<N> {
    type Out;
    type Rest;
}
impl<T> TakeFn<Zero> for T {
    type Out = TNil;
    type Rest = T;
}
impl<H, T, N> TakeFn<N> for TCons<H, T>
where
    N: PredFn, // N > 0
    T: TakeFn<Pred<N>>,
{
    type Out = TCons<H, <T as TakeFn<Pred<N>>>::Out>;
    type Rest = <T as TakeFn<Pred<N>>>::Rest;
}

pub trait ChildTypesFn {
    type Out;
}
type ChildTypes<T> = <T as ChildTypesFn>::Out;

// Type-level cons list
pub struct TNil;
pub struct TCons<H, T>(PhantomData<(H, T)>);
pub trait TList<B> {
    fn map(base: B);
}
impl<B> TList<B> for TNil {
    fn map(_: B) {
        println!("TNil");
    }
}
impl<B, H, T: TList<B>> TList<B> for TCons<H, T>
where
    H: Ctx<Base = B> + TestSet + Clone + 'static,
    <H as TestSet>::Children: TList<H>,
    B: Clone,
{
    fn map(base: B) {
        let ctx = H::build(base.clone());
        let tests = H::tests();
        for t in tests {
            t(ctx.clone());
        }
        H::Children::map(ctx);
        T::map(base);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Clone)]
    struct NullCtx;
    type CtxInit = NullCtx;
    #[derive(Clone)]
    struct Ctx1;
    #[derive(Clone)]
    struct Ctx2;
    #[derive(Clone)]
    struct Ctx3;
    impl ChildTypesFn for NullCtx {
        type Out = TCons<Ctx1, TCons<Ctx2, TNil>>;
    }

    type Lst = TCons<Ctx1, TCons<Ctx2, TNil>>;
    impl Ctx for Ctx1 {
        type Base = NullCtx;
        fn build(base: Self::Base) -> Self {
            Self
        }
    }
    impl Ctx for Ctx2 {
        type Base = NullCtx;
        fn build(base: Self::Base) -> Self {
            Self
        }
    }
    impl Ctx for Ctx3 {
        type Base = Ctx1;
        fn build(base: Self::Base) -> Self {
            Self
        }
    }
    impl TestSet for Ctx1 {
        type Children = TCons<Ctx3, TNil>;
        fn tests() -> &'static [fn(Self)] {
            &[|_| println!("Ctx1 test1"), |_| println!("Ctx1 test2")]
        }
    }
    impl TestSet for Ctx2 {
        type Children = TNil;
        fn tests() -> &'static [fn(Self)] {
            &[|_| println!("Ctx2 test1"), |_| println!("Ctx2 test2")]
        }
    }
    impl TestSet for Ctx3 {
        type Children = TNil;
        fn tests() -> &'static [fn(Self)] {
            &[|_| println!("Ctx3 test1"), |_| println!("Ctx3 test2")]
        }
    }

    #[test]
    fn test() {
        Lst::map(NullCtx);
    }
}
