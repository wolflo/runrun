use async_trait::async_trait;
use futures::{
    ready,
    stream::{Stream, StreamExt},
    Future, FutureExt,
};
use std::{
    marker::PhantomData,
    pin::Pin,
    task::{Context, Poll},
};

use crate::core_stream::MapBounds;

#[derive(Debug, Clone, Copy)]
pub struct TNil;
#[derive(Debug, Clone, Copy)]
pub struct TCons<H: ?Sized, T: ?Sized>(PhantomData<H>, PhantomData<T>);

pub type AsyncRes<'fut, Y> = Pin<Box<dyn Future<Output = Y> + Send + 'fut>>;
pub type AsyncFn<'fut, X, Y> = dyn Fn(X) -> AsyncRes<'fut, Y> + Send + Sync;

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
pub type FnFut<'fut, F, Args> = AsyncRes<'fut, FnOut<F, Args>>;
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
    pub f: F,
    pub args: Args,
    // pub next: fn(&'a mut Self) -> Option<fn(&'a F, Args) -> FnFut<'a, F, Args>>,
    pub next: fn(&mut Self) -> Option<fn(&F, Args) -> FnFut<'a, F, Args>>,
    // pub next: Option<fn(&mut Self)>,
    // fut: Option<Pin<Box<dyn futures::Future<Output = <F as FnT<Args>>::Output> + Send>>>,
    fut: Option<FnFut<'a, F, Args>>,
}

impl<'a, F, Args, Lst> Stream for MapT<'a, F, Args, Lst>
where
    // Self: 'a,
    F: FnT<Args> + Unpin,
    // Pin<&'a mut Self>: 'static,
    // F: FnT<Args> + Unpin,
    Args: Unpin + Clone,
    Lst: ?Sized,
{
    type Item = F::Output;
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        // let _self = self.get_mut();
        if let Some(fut) = &self.fut {

        } else {
            // not waiting on a previous future to resolve, get the next one
            let me = self.get_mut();
            if let Some(call) = (me.next)(me) {
                let mut called = call(&me.f, me.args.clone());
                if let Poll::Ready(res) = called.poll_unpin(cx) {
                    // current future resolved
                    return Poll::Ready(Some(res))
                } else {
                    me.fut = Some(called);
                }
                // _self.fut = Some(call(&_self.f, _self.args));
            }
        }
        // if let Some(fut) = &mut _self.fut {
        //     if let Poll::Ready(res) = fut.poll_unpin(cx) {
        //         _self.fut = None;
        //         return Poll::Ready(Some(res))
        //     } else {
        //         return Poll::Pending
        //     }
        // } else {
        //     let fut = (_self.next)(_self);
        //     // _self.fut = fut;
        //     Poll::Pending
        // }
        Poll::Pending
        // if let Some(fut) = self.fut {
        //     if let Poll::Ready(res) = fut.poll_unpin(cx) {
        //         self.fut = None;
        //         Poll::Ready(Some(res))
        //     } else {
        //         Poll::Pending
        //     }
        // } else {
        //     if let Some(step) = self.next {
        //         (step)(self.get_mut());
        //     }
        //     // (self.next)(self.get_mut());
        //     Poll::Pending
        // }
        // let maybe_fut = (self.next)(self.get_mut());
        // match maybe_fut {
        //     Some(mut fut) => {
        //         println!("some");
        //         // let res = ready!(fut.poll_unpin(cx));
        //         let res = match fut.poll_unpin(cx) {
        //             Poll::Ready(t) => {
        //                 println!("Future Ready!");
        //                 t
        //             },
        //             Poll::Pending =>  {
        //                 println!("Future pending");
        //                 return Poll::Pending
        //             },
        //         };
        //         println!("poll next resolving");
        //         Poll::Ready(Some(res))
        //     }
        //     None => Poll::Ready(None),
        // }
    }
}

impl<'a, F, Args, Lst> MapT<'a, F, Args, Lst>
where
    F: FnT<Args>,
    Lst: ?Sized + MapStep<F, Args>,
{
    pub fn new(f: F, args: Args) -> Self {
        Self {
            f,
            args,
            next: |map| <Lst as MapStep<F, Args>>::step(map),
            // next: <Lst as MapStep<F, Args>>::step,
            fut: None,
        }
    }
}

pub trait MapStep<F, Args>
where
    F: FnT<Args>,
{
    // Lst only serves to determine the type of the original TList to be mapped over.
    // In practice, we care about the current position in the TList being mapped over,
    // represented by the implementer of this trait.
    // fn step<Lst>(map: &mut MapT<F, Args, Lst>) -> Option<FnFut<'_, F, Args>>
    // fn step<Lst>(map: &mut MapT<F, Args, Lst>) -> Option<fn(&F, Args) -> FnFut<'_, F, Args>>
    // fn step<Lst>(map: &mut MapT<F, Args, Lst>) -> Option<fn(&F, Args) -> FnFut2<F, Args>>
    // fn step<Lst>(map: &mut MapT<F, Args, Lst>) -> Option<fn(&F, Args) -> FnFut<'_, F, Args>>
    // fn step<Lst>(map: &mut MapT<F, Args, Lst>) -> Option<for<'a> fn(&'a F, Args) -> FnFut<'_, F, Args>>
    // fn step<'a, Lst>(map: &'a mut MapT<F, Args, Lst>) -> Option<fn(&'a F, Args) -> FnFut<'a, F, Args>>
    fn step<Lst>(map: &mut MapT<F, Args, Lst>) -> Option<fn(&F, Args) -> FnFut<'_, F, Args>>
    where
        Lst: ?Sized;
}
impl<F, Args> MapStep<F, Args> for TNil
where
    F: FnT<Args>,
{
    // fn step<Lst>(_map: &mut MapT<F, Args, Lst>) -> Option<FnFut<'_, F, Args>>
    // fn step<Lst>(map: &mut MapT<F, Args, Lst>) -> Option<fn(&F, Args) -> FnFut<'_, F, Args>>
    // fn step<Lst>(map: &mut MapT<F, Args, Lst>) -> Option<fn(&F, Args) -> FnFut2<F, Args>>
    // fn step<Lst>(map: &mut MapT<F, Args, Lst>) -> Option<fn(&F, Args) -> FnFut<'_, F, Args>>
    // fn step<Lst>(map: &mut MapT<F, Args, Lst>) -> Option<for<'a> fn(&'a F, Args) -> FnFut<'_, F, Args>>
    // fn step<'a, Lst>(map: &'a mut MapT<F, Args, Lst>) -> Option<fn(&'a F, Args) -> FnFut<'a, F, Args>>
    fn step<Lst>(map: &mut MapT<F, Args, Lst>) -> Option<fn(&F, Args) -> FnFut<'_, F, Args>>
    where
        Lst: ?Sized,
    {
        None
    }
}
impl<F, Args, H, T> MapStep<F, Args> for TCons<H, T>
where
    F: FnT<Args> + FnT<H>,
    Args: Clone,
    T: MapStep<F, Args>,
    H: MapBounds<Args>,
    ChildTypes<H>: MapStep<F, H>,
{
    // fn step<Lst>(map: &mut MapT<F, Args, Lst>) -> Option<fn(&F, Args) -> FnFut2<F, Args>>
    // fn step<Lst>(map: &mut MapT<F, Args, Lst>)
    // fn step<Lst>(map: &mut MapT<F, Args, Lst>) -> fn(&mut MapT<F, Args, Lst>)
    fn step<Lst>(map: &mut MapT<F, Args, Lst>) -> Option<fn(&F, Args) -> FnFut<'_, F, Args>>
    // fn step<Lst>(map: &mut MapT<F, Args, Lst>) -> Option<for<'a> fn(&'a F, Args) -> FnFut<'_, F, Args>>
    // fn step<'a, Lst>(map: &'a mut MapT<F, Args, Lst>) -> Option<fn(&'a F, Args) -> FnFut<'a, F, Args>>
    where
        Lst: ?Sized,
    {
        // map.next = <T as MapStep<F, Args>>::step;
        // map.fut = Some(())
        // map.fut = Some(F::call::<H>(&map.f, map.args.clone()));
        // Some(F::call::<H>(&map.f, map.args.clone()))
        Some(|f, args| F::call::<H>(f, args))
    }
}
