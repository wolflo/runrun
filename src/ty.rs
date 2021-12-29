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
// pub struct TCons<H, T>(PhantomData<(H, T)>);
pub struct TCons<H, T>(H, T);
pub trait TList2 {
    fn map_fn<F>(f: F);
}
pub trait TList<B> {
    fn map(base: B);
}
impl<B> TList<B> for TNil {
    fn map(_: B) {
        println!("TNil");
    }
}

impl<T> RunRunImplementer for T
where
    T: BuildMe + TestSet + ChildTypesFn + Clone + 'static,
    ChildTypes<T>: RunRunImplementer,
{
    fn run() {
        let ctx = Self::build();
        let tests = Self::tests();
        for t in tests {
            t(ctx.clone());
        }
        ChildTypes::<Self>::run();
    }
    fn run_fn<F>(f: F) where F: FnOnce() {
        // Self::f();
    }
}
pub trait BuildMe {
    fn build() -> Self;
}
pub trait RunRunImplementer {
    fn run();
    fn run_fn<F>(f: F) where F: FnOnce();
}
impl<B, H, T: TList<B>> TList<B> for TCons<H, T>
where
    H: Ctx<Base = B> + TestSet + ChildTypesFn + Clone + 'static,
    ChildTypes<H>: TList<H>,
    B: Clone,
{
    fn map(base: B) {
        let ctx = H::build(base.clone());
        let tests = H::tests();
        for t in tests {
            t(ctx.clone());
        }
        ChildTypes::<H>::map(ctx);

        // Func is a Fn (trait) that takes a type (H) and returns an FnOnce
        // Func<H>();
        // H::run(base);
        T::map(base);
    }
}

pub trait TypeToTypeFn {
    type Out;
}

// F takes a type as an arg
pub trait MapFn<F> {
    type Out; // An HList of the types of the output
    fn map() -> Self::Out; // A populated hlist of values
}
// let x: Vec<Map<List, F>> = <List as MapFn<F>>::map();
// If specialization were more advanced (specifically, if default associated
// types were not treated as opaque types), we could use GATs to allow different
// return types for the same Func applied to different input types
pub trait Func {
    type Out;
    fn apply<H>() -> Self::Out;
}

// We don't actually need fns that return different types
// depending on the input type.
pub struct True; pub struct False;
type TIsZero<N> = <N as TIsZeroFn>::Out;
pub trait TIsZeroFn {
    type Out;
    fn call() -> Self::Out;
}
impl TIsZeroFn for Zero {
    type Out = True;
    fn call() -> Self::Out { True }
}
impl<N> TIsZeroFn for Succ<N> {
    type Out = False;
    fn call() -> Self::Out { False }
}
// struct TIsZeroProxy;
// impl Func<()> for TIsZeroProxy {
//     // type Apply = <Self as TIsZeroFn>::Out;
//     fn apply() -> () { T::call() }
// }
struct NullFn;
impl Func for NullFn {
    type Out = ();
    fn apply<H>() -> Self::Out { println!("foo") }
}

// pub trait TIsNonZeroFn {
//     type Out;
//     fn call() -> Self::Out;
// }
// impl<T> Func for T where T: TIsNonZeroFn {
//     type Apply = <T as TIsNonZeroFn>::Out;
//     fn apply() -> Self::Apply { T::call() }
// }

// This won't work bc specialization of associated types are treated as opaque types
// struct Error;
// impl<T> TIsZeroFn for T {
//     default type Out = Error;
//     default fn call() -> Self::Out { Error }
// }

// Instead of Run<()>, we need F: Func, and F::Apply<>
impl<F, H, T> MapFn<F> for TCons<H, T>
where
    T: MapFn<F>,
    // F: Run<()>
    F: Func,
{
    // type Out = TCons<<F as Func>::Out, <T as MapFn<F>>::Out>;
    type Out = TCons<Apply<F>, Map<F, T>>;
    // type Out = TCons<<F as Func>::Apply, <T as MapFn<F>>::Out>;
    // fn map() -> Self::Out { TCons(<H as Run<()>>::call(()), <T as MapFn<F>>::map()) }
    // fn map() -> Self::Out { TCons(<H as TIsZeroFn>::call(), <T as MapFn<F>>::map()) }
    fn map() -> Self::Out { TCons(<F as Func>::apply::<H>(), <T as MapFn<F>>::map()) }
}
impl<F: Func> MapFn<F> for TNil {
    type Out = TNil;
    fn map() -> Self::Out { TNil }
}
type Map<F, Lst> = <Lst as MapFn<F>>::Out;
type Apply<F> = <F as Func>::Out;


// H::map(f);

// Need to map a TypeToValFn over a TList, generating a Vec<Output>
// This trait represents a fn from a type (Self) and a set of values (Args)
// to a value of type Output
// To map a different TypeToValFn, make a new trait with the same fields
pub trait TypeToValFn<Args> {
    type Out;
    fn call(self, args: Args) -> Self::Out;
}
pub trait Run<Args> {
    type Out;
    fn call(args: Args) -> Self::Out;
}
impl<T> Run<()> for T
where
    T: BuildMe + TestSet + ChildTypesFn + Clone + 'static,
    ChildTypes<T>: RunRunImplementer,
{
    type Out = ();
    fn call(args: ()) -> Self::Out {
        let ctx = Self::build();
        let tests = Self::tests();
        for t in tests {
            t(ctx.clone());
        }
        ChildTypes::<Self>::run();
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
        fn tests() -> &'static [fn(Self)] {
            &[|_| println!("Ctx1 test1"), |_| println!("Ctx1 test2")]
        }
    }
    impl TestSet for Ctx2 {
        fn tests() -> &'static [fn(Self)] {
            &[|_| println!("Ctx2 test1"), |_| println!("Ctx2 test2")]
        }
    }
    impl TestSet for Ctx3 {
        fn tests() -> &'static [fn(Self)] {
            &[|_| println!("Ctx3 test1"), |_| println!("Ctx3 test2")]
        }
    }
    impl ChildTypesFn for Ctx1 {
        type Out = TCons<Ctx3, TNil>;
    }
    impl ChildTypesFn for Ctx2 {
        type Out = TNil;
    }
    impl ChildTypesFn for Ctx3 {
        type Out = TNil;
    }

    #[test]
    fn test() {
        // Lst::map(NullCtx);
        <Lst as MapFn<NullFn>>::map();
    }
}
