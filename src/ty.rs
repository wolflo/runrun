use async_trait::async_trait;
use std::iter;
use std::marker::PhantomData;

// Type-level represenation of natural numbers (see Peano numbers)
// generic_const_exprs is unstable or we could just use a const generic
pub struct Zero;
pub struct Succ<N>(PhantomData<N>);
pub type One = Succ<Zero>;

// Type-level cons list
pub struct TNil;
pub struct TCons<H, T> {
    pub head: H,
    pub tail: T,
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
pub type Pred<N> = <N as PredFn>::Out;
pub trait PredFn {
    type Out;
}
impl<N> PredFn for Succ<N> {
    type Out = N;
}

// Get first element of a TList. Undefined for an empty TList (TNil)
pub type Head<T> = <T as HeadFn>::Out;
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
pub type Tail<T> = <T as TailFn>::Out;
pub trait TailFn {
    type Out;
    fn tail(self) -> Self::Out;
}
impl<H, T> TailFn for TCons<H, T> {
    type Out = T;
    fn tail(self) -> Self::Out {
        self.tail
    }
}

// Take N elements of a TList
pub type Take<N, T> = <T as TakeFn<N>>::Out;
// pub type Drop<N, T> = <T as TakeFn<N>>::Rest;
// pub type Drop1<T> = Drop<One, T>;
// type TakeDrop<N, T> = (Take<N, T>, Drop<N, T>);
pub type Drop<N, T> = <T as DropFn<N>>::Out;
pub type Drop1<T> = Drop<One, T>;
pub type Take1<T> = Take<One, T>;
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
impl<N, H, T> TakeFn<N> for TCons<H, T>
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
pub trait DropFn<N> {
    type Out;
    fn drop(self) -> Self::Out;
}
impl<T> DropFn<Zero> for T {
    type Out = T;
    fn drop(self) -> Self::Out {
        self
    }
}
impl<N, H, T> DropFn<N> for TCons<H, T>
where
    N: PredFn,
    T: DropFn<Pred<N>>,
{
    type Out = <T as DropFn<Pred<N>>>::Out;
    fn drop(self) -> Self::Out {
        self.tail.drop()
    }
}

// FnT is essentially FnMut with an additional generic parameter that is not included
// in the type signature of call().
type Apply<F, Args, T> = <F as FnT<T, Args>>::Out;
#[async_trait]
pub trait FnT<T, Args> {
    type Out;
    async fn call(&mut self, args: Args) -> Self::Out;
}
pub trait FnTSync<T, Args> {
    type Out;
    fn call(&self, args: Args) -> Self::Out;
}

struct TMap<F, Args, Lst> {
    lst: Lst,
    f: F,
    args: Args,
}
impl<F, Args, H, T> IntoIterator for TMap<F, Args, TCons<H, T>>
where
    F: FnTSync<H, Args>,
    T: TailFn,
    TMap<F, Args, T>: IntoIterator<Item = F::Out>,
    Args: Clone,
{
    type Item = F::Out;
    type IntoIter =
        iter::Chain<iter::Once<Self::Item>, <TMap<F, Args, T> as IntoIterator>::IntoIter>;
    fn into_iter(self) -> Self::IntoIter {
        let node = <F as FnTSync<H, Args>>::call(&self.f, self.args.clone());
        let rest = TMap {
            lst: self.lst.tail(),
            f: self.f,
            args: self.args,
        };
        iter::once(node).chain(rest.into_iter())
    }
}
impl<F, Args, H> IntoIterator for TMap<F, Args, TCons<H, TNil>>
where
    F: FnTSync<H, Args>,
    Args: Clone,
{
    type Item = F::Out;
    type IntoIter = iter::Once<Self::Item>;
    fn into_iter(self) -> Self::IntoIter {
        let node = <F as FnTSync<H, Args>>::call(&self.f, self.args.clone());
        iter::once(node)
    }
}
impl<F, Args> IntoIterator for TMap<F, Args, TNil>
where
    F: FnTSync<TNil, Args>,
    Args: Clone,
{
    type Item = F::Out;
    type IntoIter = iter::Empty<Self::Item>;
    fn into_iter(self) -> Self::IntoIter {
        iter::empty()
    }
}

// Map maps a polymorphic value-level function over a TList. The return type
// of the function must be the same for all types.
// If specialization were more advanced (specifically, if default associated
// types were not treated as opaque types), we could use GATs to allow different
// return types for the same FnT applied to different input types.
type Map<F, Args, Lst> = <Lst as MapFn<F, Args>>::Out;
pub async fn tmap<F, Args, Lst>(f: &mut F, args: Args) -> Map<F, Args, Lst>
where
    Lst: MapFn<F, Args>,
{
    <Lst as MapFn<F, Args>>::map(f, args).await
}
#[async_trait]
pub trait MapFn<F, Args> {
    type Out; // A TList of the types of the output
    async fn map(f: &mut F, args: Args) -> Self::Out; // A populated TList of values
}
#[async_trait]
impl<F, Args, H, T> MapFn<F, Args> for TCons<H, T>
where
    T: MapFn<F, Args> + Elem + HeadFn,
    F: FnT<H, Args> + FnT<Head<T>, Args> + Send + Sync,
    Args: Send + Sync + Clone + 'static,
    Apply<F, Args, H>: Send,
{
    type Out = TCons<Apply<F, Args, H>, Map<F, Args, T>>;
    async fn map(f: &mut F, args: Args) -> Self::Out {
        TCons {
            head: <F as FnT<H, Args>>::call(f, args.clone()).await,
            tail: <T as MapFn<F, Args>>::map(f, args).await,
        }
    }
}
#[async_trait]
impl<F, Args, H> MapFn<F, Args> for TCons<H, TNil>
where
    F: FnT<H, Args> + Send,
    Args: Send + 'static,
    Apply<F, Args, H>: Send,
{
    type Out = TCons<Apply<F, Args, H>, TNil>;
    async fn map(f: &mut F, args: Args) -> Self::Out {
        TCons {
            head: <F as FnT<H, Args>>::call(f, args).await,
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
