use async_trait::async_trait;
use std::marker::PhantomData;

// Type-level represenation of natural numbers (see Peano numbers)
// generic_const_exprs is unstable or we could just use a const generic
struct Zero;
struct Succ<N>(PhantomData<N>);
type One = Succ<Zero>;

// Type-level cons list
pub struct TNil;
pub struct TCons<H, T> {
    head: H,
    tail: T,
}

// A mapping from type -> TList of descendant types. Specifically, this is
// used to define the child Ctxs that can be built from each Ctx.
pub type ChildTypes<T> = <T as ChildTypesFn>::Out;
pub trait ChildTypesFn {
    type Out;
}

// Any element of a TList (not TNil). For distinguishing base case trait impls.
pub trait Elem {}
impl<H, T: Elem> Elem for TCons<H, T> {}
impl<H> Elem for TCons<H, TNil> {}

// Get the predecessor (N - 1) of any nonzero Nat
type Pred<N> = <N as PredFn>::Out;
trait PredFn {
    type Out;
}
impl<N> PredFn for Succ<N> {
    type Out = N;
}

// Get first element of a TList. Undefined for an empty TList (TNil)
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

// Take N elements of a TList
type Take<N, T> = <T as TakeFn<N>>::Out;
type Drop<N, T> = <T as TakeFn<N>>::Rest;
type Drop1<T> = Drop<One, T>;
type Take1<T> = Take<One, T>;
type TakeDrop<N, T> = (Take<N, T>, Drop<N, T>);
trait TakeFn<N> {
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

// Func is essentially FnMut with an additional generic parameter that is not included
// in the type signature of call().
type Apply<F, Args, T> = <F as Func<T, Args>>::Out;
#[async_trait]
pub trait Func<T, Args> {
    type Out;
    async fn call(&mut self, args: Args) -> Self::Out;
}

// Map maps a polymorphic value-level function over a TList. The return type
// of the function must be the same for all types.
// If specialization were more advanced (specifically, if default associated
// types were not treated as opaque types), we could use GATs to allow different
// return types for the same Func applied to different input types.
type Map<F, Args, Lst> = <Lst as MapFn<F, Args>>::Out;
pub async fn tmap<F, Args, Lst>(f: &mut F, args: Args) -> Map<F, Args, Lst>
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
impl<F, Args> MapFn<F, Args> for TNil
where
    F: Send,
    Args: Send + 'static,
{
    type Out = TNil;
    async fn map(_f: &mut F, _args: Args) -> Self::Out {
        TNil
    }
}

// Turn a monomorphic TList into a vector
trait ToVec<T> {
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
