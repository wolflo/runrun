use async_trait::async_trait;
use futures::future::BoxFuture;
use std::marker::PhantomData;

use crate::core::MapBounds;

pub async fn tmap<T, F, Args>(f: &F, args: Args)
where
    T: ChildTypesFn,
    F: FnT<Args>,
    ChildTypes<T>: TList + MapStep<F, Args>,
{
    let map = MapT::new::<ChildTypes<T>>(f, args);
    for fut in map {
        fut.await;
    }
}

pub async fn map<Lst, F, Args>(f: &F, args: Args)
where
    F: FnT<Args>,
    Lst: TList + MapStep<F, Args>,
{
    let map = MapT::new::<Lst>(f, args);
    for fut in map {
        fut.await;
    }
}

/// Indicates the end of a [`trait@TList`].
#[derive(Debug, Clone, Copy)]
pub struct TNil;
/// The continuation of a [`trait@TList`].
#[derive(Debug, Clone, Copy)]
pub struct TCons<H: ?Sized, T: ?Sized>(PhantomData<H>, PhantomData<T>);
pub trait TList {
    const LEN: usize;
}
impl<H, T> TList for TCons<H, T>
where
    H: ?Sized,
    T: TList + ?Sized,
{
    const LEN: usize = 1 + T::LEN;
}
impl TList for TNil {
    const LEN: usize = 0;
}

/// An async function from `X -> Y`.
pub type AsyncFn<'fut, X, Y> = dyn Fn(X) -> BoxFuture<'fut, Y> + Send + Sync;

/// Type alias for [`ChildTypesFn`].
pub type ChildTypes<T> = <T as ChildTypesFn>::Out;

/// Maps a type to a [`trait@TList`] of descendant types.
///
/// Specifically, this is used to define the child Ctxs that can be built from each test [`Ctx`].
///
/// [`Ctx`]: crate::core::Ctx
pub trait ChildTypesFn {
    type Out;
}

/// The output type of an [`FnT`].
pub type FnOut<F, Args> = <F as FnT<Args>>::Output;
/// The output type of an FnT, wrapped in a pinned future.
pub type FnFut<'fut, F, Args> = BoxFuture<'fut, FnOut<F, Args>>;

/// Analogous to [FnMut], but async and with a type parameter to `call<T>()`.
///
/// Maps `Args` to [`Self::Output`], parameterized by the type `T`.
///
/// The bounds on `T` must be the same for all runners, because it is not possible
/// to map an arbitrary function over a [`trait@TList`] without GATs and specialization
/// (see Will Crighton's [exploration] of this approach).
///
/// [FnMut]: https://doc.rust-lang.org/std/ops/trait.FnMut.html
/// [exploration]: https://willcrichton.net/notes/gats-are-hofs/
#[async_trait]
pub trait FnT<Args> {
    /// The return type of the function.
    type Output;
    // Returns Self::Output, but FnOut is needed to disambiguate
    async fn call<T>(&self, args: Args) -> FnOut<Self, Args>
    where
        Self: FnT<T>,
        T: MapBounds<Args>,
        ChildTypes<T>: TList + MapStep<Self, T>;
}

// Given a TList and a function that maps each type in the TList to values of
// a common type, we can map the function over the TList to generate an iterator.
// Akin to std::iter::Map, except the function to be mapped is an FnT, meaning
// it takes the next type in the TList as a type parameter to call<T>()
pub struct MapT<'a, F, Args>
where
    F: FnT<Args> + ?Sized,
{
    pub f: &'a F,
    pub args: Args,
    pub len: usize,
    next: fn(&'_ mut Self) -> Option<FnFut<'a, F, Args>>,
}
impl<'a, F, Args> Iterator for MapT<'a, F, Args>
where
    F: FnT<Args>,
{
    type Item = FnFut<'a, F, Args>;
    fn next(&mut self) -> Option<Self::Item> {
        (self.next)(self)
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len, Some(self.len))
    }
}
impl<'a, F, Args> ExactSizeIterator for MapT<'a, F, Args>
where
    F: FnT<Args>,
{
    fn len(&self) -> usize {
        self.len
    }
}
impl<'a, F, Args> MapT<'a, F, Args>
where
    F: FnT<Args>,
{
    pub fn new<Lst>(f: &'a F, args: Args) -> Self
    where
        Lst: TList + MapStep<F, Args> + ?Sized,
    {
        Self {
            f,
            args,
            len: Lst::LEN,
            next: |map| Lst::step(map),
        }
    }
}

pub trait MapStep<F, Args>
where
    F: FnT<Args> + ?Sized,
{
    fn step<'a>(map: &mut MapT<'a, F, Args>) -> Option<FnFut<'a, F, Args>>;
}

impl<F, Args, H, T> MapStep<F, Args> for TCons<H, T>
where
    F: FnT<Args> + FnT<H>,
    Args: Clone,
    T: TList + MapStep<F, Args>,
    H: MapBounds<Args>,
    ChildTypes<H>: TList + MapStep<F, H>,
{
    fn step<'a>(map: &mut MapT<'a, F, Args>) -> Option<FnFut<'a, F, Args>> {
        map.len = T::LEN;
        map.next = |map| T::step(map);
        Some(map.f.call::<H>(map.args.clone()))
    }
}

impl<F, Args> MapStep<F, Args> for TNil
where
    F: FnT<Args>,
{
    fn step<'a>(_: &mut MapT<'a, F, Args>) -> Option<FnFut<'a, F, Args>> {
        None
    }
}
