use async_trait::async_trait;
use futures::{
    ready,
    stream::{Stream, StreamExt},
    Future, FutureExt,
    future::BoxFuture,
};
use std::{
    marker::PhantomData,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

use crate::core_stream::MapBounds;

#[derive(Debug, Clone, Copy)]
pub struct TNil;
#[derive(Debug, Clone, Copy)]
pub struct TCons<H: ?Sized, T: ?Sized>(PhantomData<H>, PhantomData<T>);

// pub type BoxFuture<'fut, Y> = Pin<Box<dyn Future<Output = Y> + Send + 'fut>>;
pub type AsyncFn<'fut, X, Y> = dyn Fn(X) -> BoxFuture<'fut, Y> + Send + Sync;

// A mapping from type -> TList of descendant types. Specifically, this is
// used to define the child Ctxs that can be built from each Ctx.
pub type ChildTypes<T> = <T as ChildTypesFn>::Out;
pub trait ChildTypesFn {
    type Out;
}

// FnOut is the output type of an FnT
pub type FnOut<F, Args> = <F as FnT<Args>>::Output;
// FnFut is the output type of an FnT, wrapped in a pinned future
pub type FnFut<'fut, F, Args> = BoxFuture<'fut, FnOut<F, Args>>;
// FnT is basically an Fn trait, but async and with a type parameter to call().
// Sadly, we can't map an arbitrary function over a TList without GATs and
// specialization, so we need to include all of the bounds for the runner
// directly on the FnT trait.
// https://willcrichton.net/notes/gats-are-hofs/
#[async_trait]
pub trait FnT<Args> {
    type Output;
    async fn call<T>(&self, args: Args) -> FnOut<Self, Args>
    where
        Self: FnT<T>,
        T: MapBounds<Args>,
        ChildTypes<T>: MapStep<Self, T>;
}

// Given a TList and a function that maps each type in the TList to values of
// the same type, we can map the function over the TList to generate an iterator.
// Akin to std::iter::Map, except the function to be mapped is an FnT, meaning
// it takes the next type in the TList as a type parameter to call<T>()
pub struct MapT<'a, F, Args, Lst>
where
    F: FnT<Args> + ?Sized,
    Lst: ?Sized,
{
    pub f: &'a F,
    pub args: Args,
    next: fn(&'_ mut Self) -> Option<FnFut<'a, F, Args>>,
}
impl<'a, F, Args, Lst> Iterator for MapT<'a, F, Args, Lst>
where
    F: FnT<Args>,
    Lst: ?Sized,
{
    type Item = FnFut<'a, F, Args>;
    fn next(&mut self) -> Option<Self::Item> {
        (self.next)(self)
    }
}
impl<'a, F, Args, Lst> MapT<'a, F, Args, Lst>
where
    F: FnT<Args>,
    Lst: ?Sized + MapStep<F, Args>,
{
    pub fn new(f: &'a F, args: Args) -> Self {
        Self {
            f,
            args,
            next: |map| <Lst as MapStep<F, Args>>::step(map),
        }
    }
}

pub trait MapStep<F, Args>
where
    F: FnT<Args> + ?Sized,
{
    // Lst type param only serves to determine the type of the original TList
    // to be mapped over. In practice, we care about the current position in
    // the TList being mapped over, represented by the implementer of this trait.
    fn step<'a, Lst>(map: &mut MapT<'a, F, Args, Lst>) -> Option<FnFut<'a, F, Args>>
    where
        Lst: ?Sized;
}

impl<F, Args, H, T> MapStep<F, Args> for TCons<H, T>
where
    F: FnT<Args> + FnT<H>,
    Args: Clone,
    T: MapStep<F, Args>,
    H: MapBounds<Args>,
    ChildTypes<H>: MapStep<F, H>,
{
    fn step<'a, Lst>(map: &mut MapT<'a, F, Args, Lst>) -> Option<FnFut<'a, F, Args>>
    where
        Lst: ?Sized,
    {
        map.next = |map| <T as MapStep<F, Args>>::step(map);
        Some(map.f.call::<H>(map.args.clone()))
    }
}

impl<F, Args> MapStep<F, Args> for TNil
where
    F: FnT<Args>,
{
    fn step<'a, Lst>(_map: &mut MapT<'a, F, Args, Lst>) -> Option<FnFut<'a, F, Args>>
    where
        Lst: ?Sized,
    {
        None
    }
}
