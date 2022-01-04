use crate::{core::AsyncOutput, stream::*, ty::*, TList};
use async_trait::async_trait;
use futures::{
    ready, stream,
    stream::{Stream, StreamExt},
    Future, FutureExt,
};
use std::{
    fmt::Debug,
    iter,
    iter::{Chain, Once},
    marker::PhantomData,
    pin::Pin,
    task::{Context, Poll},
};

pub trait Trait1<T> {
    fn inner() -> T;
}
pub trait Trait2 {
    fn act();
    fn new() -> usize;
}
#[async_trait]
pub trait FnT2<T> {
    type Out;
    async fn call(&mut self) -> Self::Out;
}
// Can we hide the "real" iterator (non mutable) in a &dyn Obj field
// that we mutate?
pub struct ConsStream<F, H, T> {
    lst: TCons<H, T>,
    f: F,
    // x: Drop1<TCons<H, T>>,
}
// What about a function that calls itself in a chain with generic args descending the cons list
// fn build_and_ret<T>() -> (Box<dyn Stream<Item = T>>, fn()) {
// fn build_and_ret<T>() -> fn() {
//     build_and_ret::<usize>()
// }
pub trait Print {
    fn name();
}
pub trait NextFn {
    type Out;
}
type Next<T> = <T as NextFn>::Out;
impl Print for usize {
    fn name() {
        println!("usize");
    }
}
impl Print for u8 {
    fn name() {
        println!("u8");
    }
}
impl NextFn for usize {
    type Out = u8;
}
impl NextFn for u8 {
    type Out = TNil;
}

pub trait IterIter {
    fn ret() -> Option<()>;
}
impl<H, T> IterIter for TCons<H, T>
where
    T: IterIter,
{
    fn ret() -> Option<()> {
        println!("foo");
        T::ret()
    }
}
impl IterIter for TNil {
    fn ret() -> Option<()> {
        None
    }
}
pub fn map_fn<T>() -> Option<()>
where
    T: TakeFn<One> + IterIter,
    Take1<T>: TakeFn<Zero>,
{
    T::ret()
}
// struct Builder;
// impl Builder {
//     fn build<T>(&self) -> (fn(), fn() -> fn())
//         where
//             T: Print + NextFn,
//             Next<T>: Print + NextFn,
//     {
//         (T::name, || self.build::<Next<T>>())
//     }
// }
// I want a sort of pausable recursive fn. Something that I can partially apply,
// getting one result and the remainder of the fn chain
// fn builder?
// fn print(x: usize) -> fn(usize) -> Builder {
//     println!("x: {}", x);
//     if x > 0 {
//         // print(x - 1);
//         return Builder.build(x - 1)
//     }
//     fn exit(_: usize) -> Builder {
//         panic!("")
//     }
//     exit
// }

fn foo<T>()
where
    T: Trait2 + TakeFn<One>,
{
    let _ = T::new();
    // foo::<Take1<T>>();
}

pub trait RealStream {}

impl<F, H, T> Stream for ConsStream<F, H, T>
where
    F: FnT2<T> + Send,
    H: Trait1<usize> + Default + Unpin,
    T: Default + Unpin + Trait2,
{
    type Item = (usize, T);
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        // self.f = T::act;
        // self.f = <F as FnT2<T>>::call;
        // Poll::Ready(Some(H::inner()))
        Poll::Pending
    }
}

pub struct ConsStream2<'a, H, T> {
    lst: TCons<H, T>,
    stream: &'a dyn Something<'a>,
}
// Need a fn on self that returns the next item in the stream, and the new &dyn streamer
// to store
impl<'a, H, T> Stream for ConsStream2<'a, H, T>
where
    Self: Unpin,
{
    // type Item = &'static dyn Stream<Item = TestRes<'static>>;
    type Item = &'a dyn Something<'a>;
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let res = self.stream.foo();
        if let Some(new_stream) = res {
            self.stream = new_stream.tail;
            // Really we would be operating on new_stream.head, getting its output (a stream
            // of TestRes), and returning that
            return Poll::Ready(Some(new_stream.head));
        }
        Poll::Ready(None)
    }
}
// impl<'a, H, T> Stream for ConsStream2<'a, H, T>
// where
//     T: Stream<Item = &'static dyn Stream<Item = TestRes<'static>>>,
// {
//     type Item = &'static dyn Stream<Item = TestRes<'static>>;
//     fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
//         Poll::Ready(None)
//     }
// }

// need to store an &dyn Something, which impls a fn to give us another the next &dyn Something.
// But this is inherently non-object-safe.
// Can we instead have every &dyn Something return a TCons<>, which can then be turned into an &dyn Something?
pub trait Something<'a> {
    // Gives back either a cons list or None
    fn foo(&'a self) -> Option<TCons<&'a dyn Something<'a>, &'a dyn Something<'a>>>;
}
pub struct IterList<'a> {
    x: &'a dyn Something<'a>,
}
impl<'a, H, X, XS> Something<'a> for TCons<H, TCons<X, XS>>
where
    // T: DropFn<One> + TakeFn<One> + Clone,
    X: Something<'a> + 'a,
    XS: Something<'a> + 'a,
{
    fn foo(&'a self) -> Option<TCons<&'a dyn Something<'a>, &'a dyn Something<'a>>> {
        // let tail: Take1<T> = self.take();
        // let tail = <T as DropFn<One>>::drop(self.tail);
        // let t = TCons {
        //     head: &<T as TakeFn<One>>::take(self.tail.clone()) as &dyn Something,
        //     tail: &<T as DropFn<One>>::drop(self.tail) as &dyn Something,
        // };
        // t
        // self.tail
        Some(TCons {
            head: &self.tail.head,
            tail: &self.tail.tail,
        })
    }
}
impl<'a, H> Something<'a> for TCons<H, TNil> {
    fn foo(&'a self) -> Option<TCons<&'a dyn Something<'a>, &'a dyn Something<'a>>> {
        None
    }
}
impl<'a> Something<'a> for TNil {
    fn foo(&'a self) -> Option<TCons<&'a dyn Something<'a>, &'a dyn Something<'a>>> {
        None
    }
}

// We're actually going to have a stream of futures that all resolve to a stream
// I dont really want to return an iterator over a vec of futures resolving to a stream,
// because I want to generate the streams in series. So I need an iterator over
// fns that return futures that resolve to streams. Which means I can skip futures
// altogether and just return an iterator over the items returns by F (which will be fns)
pub trait FnTImm<T, Args> {
    type Out;
    fn call(&self, args: Args) -> Self::Out;
}
// pub trait FnTSync<T, Args> {
//     type Out;
//     fn call(&mut self, args: Args) -> Self::Out;
// }
// trait MapToStream<F, I> {
//     fn to_stream(f: &mut F) -> Box<dyn Iterator<Item = I>>;
// }
// impl<F, I, H, T> MapToStream<F, I> for TCons<H, T>
// where
//     F: FnTImm<H, (), Out = I> + FnTImm<Head<T>, ()> + Send + Sync + 'static,
//     T: MapToStream<F, I> + HeadFn + Send + Sync,
//     I: Send + Sync + 'static,
// {
//     fn to_stream(f: &mut F) -> Box<dyn Iterator<Item = I>> {
//         let node = <F as FnTImm<H, ()>>::call(f, ());
//         let v = iter::once(node);
//         Box::new(v.chain(T::to_stream(f)))
//     }
// }
// struct TMap<F, Lst> {
//     lst: Lst,
//     f: F,
// }
// impl<F, Lst> IntoIterator for TMap<F, Lst>
// where
//     F: FnTSync<Head<Lst>, ()>,
//     Lst: HeadFn + TailFn,
//     Tail<Lst>: TailFn,
//     TMap<F, Tail<Lst>>: IntoIterator<Item = F::Out>,
// {
//     type Item = F::Out;
//     type IntoIter = Chain<Once<Self::Item>,
//             <TMap<F, Tail<Lst>> as IntoIterator>::IntoIter
//         >;
//     fn into_iter(self) -> Self::IntoIter {
//         let node = <F as FnTSync<Head<Lst>, ()>>::call(&self.f, ());
//         let mapped = TMap {
//             lst: self.lst.tail(),
//             f: self.f,
//         };
//         iter::once(node).chain(mapped.into_iter())
//     }
// }
// impl<F, H, T> Iterator for Mapped<F, H, T>
// where
//     F: FnTSync<T, ()>,
// {
//     type Item = F::Out;
//     fn next(&mut self) -> Option<Self::Item> {
//         todo!()
//     }
// }

// F is something that can turn a type into a T
#[async_trait]
trait MapToVec<F, T> {
    // type Out = Vec<T>;
    async fn to_vec(f: &mut F) -> Vec<T>;
}
#[async_trait]
impl<F, I, X, XS> MapToVec<F, I> for TCons<X, XS>
where
    F: FnTMut<X, (), Out = I> + FnTMut<Head<XS>, ()> + Send + Sync,
    XS: MapToVec<F, I> + HeadFn + Send + Sync,
    I: Send + Sync,
{
    async fn to_vec(f: &mut F) -> Vec<I> {
        let mut v = vec![<F as FnTMut<X, ()>>::call(f, ()).await];
        v.extend(XS::to_vec(f).await);
        v
    }
}

use anyhow::Result;
use crate::core::{Runner, Driver};
struct DBuilder<'a, R>{ runner: R, _ghost: PhantomData<&'a R>, }
impl<'a, R, T, Args> FnTSync<T, Args> for DBuilder<'a, R>
where
    R: Runner<Out = Result<()>> + Send + Sync + Clone,
    T: 'a,
    Args: 'a,
{
    // type Out = &'a dyn FnT<T, Args, Out = ()>;
    type Out = Driver<R>;
    fn call(&self, args: Args) -> Self::Out {
        Driver::new(self.runner.clone())
    }
}

// If we lift our TList to the trait level, can we store an &dyn TList? No, would
// need to specify the associated type
#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn test_exp() {
        // in between every print statement, want to print something else
        type T = TList!(usize, u8, usize);
        type Next1 = Take1<T>;
        type Next2 = Take1<Next1>;
        type Next3 = Take1<Next2>;
    }
}
