//! Provide bi-directionnal MPMC broadcast
use std::sync::mpsc::*;
use std::pin::Pin;

use futures::{Sink,Stream};
use futures::task::{Poll,Context};
pub use futures::channel::mpsc;
pub use futures::channel::oneshot;

use bus;


/// Bi-directionnal message channel, that can be used as future transport.
pub struct Channel<S,R>
    where R: Unpin+ChannelReceiver, S: Unpin+ChannelSender
{
    pub receiver: R,
    pub sender: S,
}

impl<S,R> Channel<S,R>
    where S: Unpin+ChannelSender, R: Unpin+ChannelReceiver
{
    /// Create a bidirectionnal channel.
    pub fn new(cap: u64) -> (Self,Channel<R::Sender, S::Receiver>) {
        let (rs, rr) = R::channel(cap);
        let (ss, sr) = S::channel(cap);
        (Self { receiver: rr, sender: ss, },
         Channel { receiver: sr, sender: rs })
    }
}


impl<S,R> Stream for Channel<S,R>
    where S: Unpin+ChannelSender, R: Unpin+ChannelReceiver
{
    type Item = R::Item;

    fn poll_next(self: Pin<&mut Self>, _cx: &mut Context) -> Poll<Option<Self::Item>> {
        match self.get_mut().receiver.try_recv() {
            Ok(None) => Poll::Pending,
            Ok(Some(r)) => Poll::Ready(Some(r)),
            Err(_) => Poll::Ready(None),
        }
    }
}


impl<S,R> Sink<S::Item> for Channel<S,R>
    where S: Unpin+ChannelSender, R: Unpin+ChannelReceiver
{
    type Error = ();

    fn poll_ready(self: Pin<&mut Self>, _cx: &mut Context) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn start_send(self: Pin<&mut Self> , item: S::Item)
        -> Result<(), Self::Error>
    {
        self.get_mut().sender.try_send(item).or(Err(()))
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context)
        -> Poll<Result<(), Self::Error>>
    {
        Poll::Ready(Ok(()))
    }

    fn poll_close(self: Pin<&mut Self>, _cx: &mut Context) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }
}


/// Channel whose sender and receiver are mpsc queues.
pub type MPSCChannel<S,R> = Channel<mpsc::Sender<S>, mpsc::Receiver<R>>;

/// Channel for broadcasting message, while receiving message from multiple producers.
pub type BroadcastChannel<S,R> = Channel<bus::Bus<S>, mpsc::Receiver<R>>;
/// Other side of a broadcast channel
pub type BroadcastChannelRev<S,R> = Channel<mpsc::Sender<R>, bus::BusReader<S>>;


/// Generic sender
pub trait ChannelSender : Sized {
    type Item;
    type Receiver: ChannelReceiver<Item=Self::Item>+Unpin;
    type Error;

    fn channel(cap: u64) -> (Self, Self::Receiver);
    fn try_send(&mut self, item: Self::Item) -> Result<(), Self::Error>;
}

/// Generic Receiver
pub trait ChannelReceiver : Sized {
    type Item;
    type Sender: ChannelSender<Item=Self::Item>+Unpin;
    type Error;

    fn channel(cap: u64) -> (Self::Sender, Self);
    fn try_recv(&mut self) -> Result<Option<Self::Item>, Self::Error>;
    // fn close(&mut self);
}


impl<T> ChannelSender for mpsc::Sender<T> {
    type Item = T;
    type Receiver = mpsc::Receiver<T>;
    type Error = mpsc::TrySendError<Self::Item>;

    fn channel(cap: u64) -> (Self, Self::Receiver) {
        mpsc::channel(cap as usize)
    }

    fn try_send(&mut self, item: Self::Item) -> Result<(), Self::Error> {
        <mpsc::Sender<T>>::try_send(self, item)
    }
}

impl<T> ChannelReceiver for mpsc::Receiver<T> {
    type Item = T;
    type Sender = mpsc::Sender<T>;
    type Error = mpsc::TryRecvError;

    fn channel(cap: u64) -> (Self::Sender, Self) {
        mpsc::channel(cap as usize)
    }

    fn try_recv(&mut self) -> Result<Option<Self::Item>, Self::Error> {
        self.try_next()
    }
}

impl<T> ChannelSender for oneshot::Sender<T> {
    type Item = T;
    type Receiver = oneshot::Receiver<Self::Item>;
    type Error = T;

    fn channel(_cap: u64) -> (Self, Self::Receiver) {
        oneshot::channel()
    }

    fn try_send(&mut self, item: Self::Item) -> Result<(), Self::Error> {
        // self.send(item) => consuming self
        Err(item)
    }
}

impl<T> ChannelReceiver for oneshot::Receiver<T> {
    type Item = T;
    type Sender = oneshot::Sender<T>;
    type Error = oneshot::Canceled;

    fn channel(_cap: u64) -> (Self::Sender, Self) {
        oneshot::channel()
    }

    fn try_recv(&mut self) -> Result<Option<Self::Item>, Self::Error> {
        self.try_recv()
    }
}


impl<T: Clone+Sync> ChannelSender for bus::Bus<T> {
    type Item = T;
    type Receiver = bus::BusReader<Self::Item>;
    type Error = T;

    fn channel(cap: u64) -> (Self, Self::Receiver) {
        let mut bus = bus::Bus::new(cap as usize);
        let reader = bus.add_rx();
        (bus, reader)
    }

    fn try_send(&mut self, item: Self::Item) -> Result<(), Self::Error> {
        self.try_broadcast(item)
    }
}

impl<T: Clone+Sync> ChannelReceiver for bus::BusReader<T> {
    type Item = T;
    type Sender = bus::Bus<T>;
    type Error = RecvError;

    fn channel(cap: u64) -> (Self::Sender, Self) {
        let mut bus = bus::Bus::new(cap as usize);
        let reader = bus.add_rx();
        (bus, reader)
    }

    fn try_recv(&mut self) -> Result<Option<Self::Item>, Self::Error> {
        self.recv().map(|o| Some(o))
    }
}


