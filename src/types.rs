use async_trait::async_trait;
use futures::{
    future::BoxFuture,
    ready,
    stream::{Stream, StreamExt},
    Future, FutureExt,
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
// FnT is similar to the FnMut trait, but async and with a type parameter to call().
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
        ChildTypes<T>: TList + MapStep<Self, T>;
}

// Given a TList and a function that maps each type in the TList to values of
// the same type, we can map the function over the TList to generate an iterator.
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
            next: |map| <Lst as MapStep<F, Args>>::step(map),
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
        map.next = |map| <T as MapStep<F, Args>>::step(map);
        Some(map.f.call::<H>(map.args.clone()))
    }
}

impl<F, Args> MapStep<F, Args> for TNil
where
    F: FnT<Args>,
{
    fn step<'a>(map: &mut MapT<'a, F, Args>) -> Option<FnFut<'a, F, Args>> {
        map.len = Self::LEN;
        None
    }
}

pub trait ExactSizeStream: Stream {
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
