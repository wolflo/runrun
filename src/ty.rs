use anyhow::Result;
use async_trait::async_trait;
use futures::future::Future;
use std::marker::PhantomData;
use std::sync::Arc;

// Type-level cons list
pub struct TNil;
pub struct TCons<H, T> {
    head: H,
    tail: T,
}

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

// fns from types to values is just parametric polymorphism (generic fns). But we need
// higher-rank types to be able to pass a polymorphic fn without monomorphizing it.

// Need to make something of kind * -> *, but abstract over arity
// * -> * -> *

// have a list of types
// need a fn (trait) that takes a list of types and a (list of?) function on those types and accumulated the results
// Runner should be built from initial state that user designates with #[run_state(init)]
//  - should take only a context and a list of tests -- see proptest lib
#[async_trait]
pub trait Runner {
    type Out;
    async fn run<T>(&self, ctx: &T, tests: &'static [fn(T)]) -> Self::Out;
}
#[async_trait]
pub trait Ctx {
    type Base;
    async fn build(base: Self::Base) -> Self;
}
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
    fn take(self) -> Self::Out;
}
impl<T> TakeFn<Zero> for T {
    type Out = TNil;
    type Rest = T;
    fn take(self) -> Self::Out {
        TNil
    }
}
impl<H, T, N> TakeFn<N> for TCons<H, T>
where
    N: PredFn, // N > 0
    T: TakeFn<Pred<N>>,
{
    type Out = TCons<H, <T as TakeFn<Pred<N>>>::Out>;
    type Rest = <T as TakeFn<Pred<N>>>::Rest;
    fn take(self) -> Self::Out {
        TCons {
            head: self.head,
            tail: self.tail.take(),
        }
    }
}

// Get first element of a TList
type Head<T> = <T as HeadFn>::Out;
pub trait HeadFn {
    type Out;
    fn head(self) -> Self::Out;
}
impl<H, T> HeadFn for TCons<H, T> {
    type Out = H;
    fn head(self) -> Self::Out {
        self.head
    }
}

pub trait ChildTypesFn {
    type Out;
}
type ChildTypes<T> = <T as ChildTypesFn>::Out;

// Func is essentially FnOnce as an associated function (without a self param)
// and an added generic arg that is not a fn parameter. apply() stands in for call_once().
// If specialization were more advanced (specifically, if default associated
// types were not treated as opaque types), we could use GATs to allow different
// return types for the same Func applied to different input types
type Apply<F, Args, T> = <F as Func<T, Args>>::Out;
#[async_trait]
pub trait Func<T, Args> {
    type Out;
    async fn call(&mut self, args: Args) -> Self::Out;
}

type Map<F, Args, Lst> = <Lst as MapFn<F, Args>>::Out;
async fn tmap<F, Args, Lst>(f: &mut F, args: Args) -> Map<F, Args, Lst>
where
    Lst: MapFn<F, Args>,
{
    <Lst as MapFn<F, Args>>::map(f, args).await
}
#[async_trait]
pub trait MapFn<F, Args> {
    type Out; // An HList of the types of the output
    async fn map(f: &mut F, args: Args) -> Self::Out; // A populated hlist of values
}
#[async_trait]
impl<F, Args, H, T> MapFn<F, Args> for TCons<H, T>
where
    T: MapFn<F, Args> + Elem + HeadFn,
    F: Func<H, Args> + Func<Head<T>, Args> + Send + Sync,
    Args: Send + Sync + Clone + 'static,
    Apply<F, Args, H>: Send,
{
    type Out = TCons<Apply<F, Args, H>, Map<F, Args, T>>;
    async fn map(f: &mut F, args: Args) -> Self::Out {
        TCons {
            head: <F as Func<H, Args>>::call(f, args.clone()).await,
            tail: <T as MapFn<F, Args>>::map(f, args).await,
        }
    }
}
#[async_trait]
impl<F, Args, H> MapFn<F, Args> for TCons<H, TNil>
where
    F: Func<H, Args> + Send,
    Args: Send + 'static,
    Apply<F, Args, H>: Send,
{
    type Out = TCons<Apply<F, Args, H>, TNil>;
    async fn map(f: &mut F, args: Args) -> Self::Out {
        TCons {
            head: <F as Func<H, Args>>::call(f, args).await,
            tail: TNil,
        }
    }
}
#[async_trait]
impl<F, Args> MapFn<F, Args> for TNil where F: Send, Args: Send + 'static, {
    type Out = TNil;
    async fn map(_f: &mut F, _args: Args) -> Self::Out {
        TNil
    }
}
pub trait Elem {}
impl<H, T: Elem> Elem for TCons<H, T> {}
impl<H> Elem for TCons<H, TNil> {}

// Turn a monomorphic TList into a vector
pub trait ToVec<T> {
    fn to_vec(&mut self) -> Vec<T>;
}
impl<T, TS> ToVec<T> for TCons<T, TS>
where
    TS: ToVec<T>,
    T: Clone,
{
    fn to_vec(&mut self) -> Vec<T> {
        let mut v = vec![self.head.clone()];
        v.extend(self.tail.to_vec());
        v
    }
}
impl<T> ToVec<T> for TNil {
    fn to_vec(&mut self) -> Vec<T> {
        vec![]
    }
}

struct Driver<R> {
    runner: R,
}
#[async_trait]
impl<T, R, C> Func<T, C> for Driver<R>
where
    T: Ctx<Base = C> + TestSet + ChildTypesFn + Send + Sync + 'static,
    R: Runner + Send + Sync,
    C: Ctx + Send + Clone + 'static,
    ChildTypes<T>: MapFn<Self, T>,
{
    type Out = Arc<Result<()>>;
    async fn call(&mut self, base: C) -> Self::Out {
        let ctx = T::build(base).await;
        let tests = T::tests();
        self.runner.run(&ctx, tests).await;
        ChildTypes::<T>::map(self, ctx).await;
        Arc::new(Ok(()))
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
    impl BuildMe for Ctx1 {
        fn build() -> Self {
            Ctx1
        }
    }
    impl BuildMe for Ctx2 {
        fn build() -> Self {
            Ctx2
        }
    }
    impl BuildMe for Ctx3 {
        fn build() -> Self {
            Ctx3
        }
    }

    impl ChildTypesFn for NullCtx {
        type Out = TCons<Ctx1, TCons<Ctx2, TNil>>;
    }

    type Lst = TCons<Ctx1, TCons<Ctx2, TNil>>;
    #[async_trait]
    impl Ctx for Ctx1 {
        type Base = NullCtx;
        async fn build(base: Self::Base) -> Self {
            Self
        }
    }
    #[async_trait]
    impl Ctx for Ctx2 {
        type Base = NullCtx;
        async fn build(base: Self::Base) -> Self {
            Self
        }
    }
    #[async_trait]
    impl Ctx for Ctx3 {
        type Base = Ctx1;
        async fn build(base: Self::Base) -> Self {
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
        type L = TCons<u8, TCons<u16, TNil>>;
        // let run_mapped = tmap::<Run, Lst>().to_vec();
        // dbg!(run_mapped);
        // let other_mapped = <L as MapFn<NullFn, ()>>::map(()).to_vec();
        // dbg!(other_mapped);
    }
}

/////

// We don't actually need fns that return different types
// depending on the input type.
type TIsZero<N> = <N as TIsZeroFn>::Out;
pub trait TIsZeroFn {
    type Out;
    fn call() -> Self::Out;
}
impl TIsZeroFn for Zero {
    type Out = True;
    fn call() -> Self::Out {
        True
    }
}
impl<N> TIsZeroFn for Succ<N> {
    type Out = False;
    fn call() -> Self::Out {
        False
    }
}
// struct NullFn;
// impl Func for NullFn {
//     type Out = ();
//     fn apply<H>() -> Self::Out {
//         println!("foo")
//     }
// }
pub struct True;
pub struct False;

// Implemented for any list containing only elements of the same type
pub trait Mono {}
impl Mono for TNil {}
impl<H> Mono for TCons<H, TNil> {}
impl<H, T> Mono for TCons<H, T>
where
    (H, Head<T>): TEq,
    T: HeadFn + Mono,
{
}
pub trait TEq {}
impl<T> TEq for (T, T) {}

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
    fn run_fn<F>(f: F)
    where
        F: FnOnce(),
    {
        // Self::f();
    }
}
pub trait BuildMe {
    fn build() -> Self;
}
pub trait RunRunImplementer {
    fn run();
    fn run_fn<F>(f: F)
    where
        F: FnOnce();
}
// impl<B, H, T: TList<B>> TList<B> for TCons<H, T>
// where
//     H: Ctx<Base = B> + TestSet + ChildTypesFn + Clone + 'static,
//     ChildTypes<H>: TList<H>,
//     B: Clone,
// {
//     fn map(base: B) {
//         let ctx = H::build(base.clone());
//         let tests = H::tests();
//         for t in tests {
//             t(ctx.clone());
//         }
//         ChildTypes::<H>::map(ctx);
//         // Func is a Fn (trait) that takes a type (H) and returns an FnOnce
//         // Func<H>();
//         // H::run(base);
//         T::map(base);
//     }
// }

// pub struct GNil<T>(PhantomData<T>);
// impl<F: Func<T>, T> MapFn<F> for GNil<T> {
//     type Out = TNil;
//     fn map() -> Self::Out {
//         TNil
//     }
// }

// let b = Build;
// let f: fn(u8) = b.build();
// let g: fn(usize) = b.build();
pub trait Builder<T> {
    fn build(&self) -> fn(T);
}
pub trait Trait {}
impl Trait for usize {}
impl Trait for u8 {}
impl Trait for u16 {}
struct Build;
impl<T: Trait> Builder<T> for Build {
    fn build(&self) -> fn(T) {
        fn inner<U>(x: U) {
            ()
        }
        inner
    }
}

// struct NullFn;
// impl<T: Trait> Func<T, ()> for NullFn {
//     type Out = ();
//     fn call(_: ()) -> Self::Out {
//         ()
//     }
// }

// impl<H, T> Iterator for TCons<H, T> where H: Clone, T: HeadFn<Out = H> + DropFn<One> + Clone {
//     type Item = H;
//     fn next(&mut self) -> Option<Self::Item> {
//         self.head = self.tail.clone().head();
//         self.tail = self.tail.drop(1);
//         Some(self.head.clone())
//     }
// }

// Need to map a TypeToValFn over a TList, generating a Vec<Output>
// This trait represents a fn from a type (Self) and a set of values (Args)
// to a value of type Output
// To map a different TypeToValFn, make a new trait with the same fields
// pub trait TypeToValFn<Args> {
//     type Out;
//     fn call(self, args: Args) -> Self::Out;
// }
// pub trait Run<Args> {
//     type Out;
//     fn call(args: Args) -> Self::Out;
// }
// impl<T> Run<()> for T
// where
//     T: BuildMe + TestSet + ChildTypesFn + Clone + 'static,
//     ChildTypes<T>: RunRunImplementer,
// {
//     type Out = ();
//     fn call(args: ()) -> Self::Out {
//         let ctx = Self::build();
//         let tests = Self::tests();
//         for t in tests {
//             t(ctx.clone());
//         }
//         ChildTypes::<Self>::run();
//     }
// }
