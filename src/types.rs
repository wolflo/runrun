use async_trait::async_trait;
use futures::{
    ready,
    stream::{Stream, StreamExt},
    Future, FutureExt,
};
use std::{
    sync::Arc,
    marker::PhantomData,
    pin::Pin,
    task::{Context, Poll},
};

use crate::core_stream::MapBounds;

#[derive(Debug, Clone, Copy)]
pub struct TNil;
#[derive(Debug, Clone, Copy)]
pub struct TCons<H: ?Sized, T: ?Sized>(PhantomData<H>, PhantomData<T>);

use futures::future::BoxFuture;
// pub type AsyncRes<'fut, Y> = Pin<Box<dyn Future<Output = Y> + Send + 'fut>>;
pub type AsyncFn<'fut, X, Y> = dyn Fn(X) -> BoxFuture<'fut, Y> + Send + Sync;

// TODO: Remove?
// Get first element of a TList. Undefined for an empty TList (TNil)
pub type Head<T> = <T as HeadFn>::Out;
pub trait HeadFn {
    type Out;
}
impl<H, T> HeadFn for TCons<H, T> {
    type Out = H;
}

// A mapping from type -> TList of descendant types. Specifically, this is
// used to define the child Ctxs that can be built from each Ctx.
pub type ChildTypes<T> = <T as ChildTypesFn>::Out;
pub trait ChildTypesFn {
    type Out;
}

// FnT is basically an Fn trait, but async and with a type parameter to call().
// Sadly, we can't map an arbitrary function over a TList without GATs and
// specialization, so we need to include all of the bounds for the runner
// directly on the FnT trait.
// https://willcrichton.net/notes/gats-are-hofs/
// FnOut is the output type of an FnT
pub type FnOut<F, Args> = <F as FnT<Args>>::Output;
// FnFut is the output type of an FnT, wrapped in a future
pub type FnFut<'fut, F, Args> = BoxFuture<'fut, FnOut<F, Args>>;
pub type FnFut2<F, Args> = Pin<Box<dyn Future<Output = FnOut<F, Args>> + Send>>;
// TODO: Sized bound should be removable
#[async_trait]
pub trait FnT<Args>: Sized {
    type Output;
    async fn call<T>(&self, args: Args) -> FnOut<Self, Args>
    where
        Self: FnT<T>,
        T: MapBounds<Args>,
        ChildTypes<T>: MapStep<Self, T>;
}

// Given a TList and a function that maps each type in the TList to values
// of the same type, we can map the function over the TList to generate an
// iterator/stream. Akin to std::iter::Map.
pub struct MapT<'a, F, Args, Lst>
where
    F: FnT<Args>,
    Lst: ?Sized,
{
    pub f: &'a F,
    pub args: Args,
    pub next: fn(&'_ mut Self) -> Option<FnFut<'a, F, Args>>,
    fut: Option<FnFut<'a, F, Args>>,
}

impl<'a, F, Args, Lst> Stream for MapT<'a, F, Args, Lst>
where
    F: FnT<Args>,
    Args: Unpin + Clone,
    Lst: ?Sized,
{
    type Item = <F as FnT<Args>>::Output;
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let me = self.get_mut();
        // If we're waiting on a future to resolve, Poll it
        if let Some(ref mut fut) = me.fut {
            if let Poll::Ready(res) = fut.poll_unpin(cx) {
                me.fut = None;
                Poll::Ready(Some(res))
            } else {
                Poll::Pending
            }
        // No pending future, so start the next one
        } else {
            if let Some(mut fut) = (me.next)(me) {
                // If the future resolves immediately, return the result
                if let Poll::Ready(res) = fut.poll_unpin(cx) {
                    Poll::Ready(Some(res))
                } else {
                    // Future is pending, so store it for next time we're polled
                    me.fut = Some(fut);
                    Poll::Pending
                }
            } else {
                // no more items to map over
                Poll::Ready(None)
            }
        }
    }
}

impl<'a, F, Args, Lst> MapT<'a, F, Args, Lst>
where
    F: FnT<Args>,
    Lst: ?Sized + MapStep<F, Args>,
{
    pub fn new(f: &'a F, args: Args) -> Self {
        Self {
            f: f,
            args,
            next: |map| <Lst as MapStep<F, Args>>::step(map),
            fut: None,
        }
    }
}

pub trait MapStep<F, Args>
where
    F: FnT<Args>,
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
        Lst: ?Sized
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
        Lst: ?Sized
    {
        None
    }
}
