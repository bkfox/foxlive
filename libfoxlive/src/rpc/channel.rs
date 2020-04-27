//! Provide bi-directionnal MPMC broadcast
use std::sync::mpsc::*;

use futures::prelude::*;
pub use futures::channel::mpsc;
pub use futures::channel::oneshot;

use bus;


/// Bi-directionnal message channel, that can be used as future transport.
pub struct Channel<S,R> {
    /// Messages receiver
    pub receiver: R,
    /// Message sender
    pub sender: S,
}

/// Channel of mpsc sender and receiver.
pub type MPSCChannel<S,R> = Channel<mpsc::Sender<S>, mpsc::Receiver<R>>;
/// Channel of oneshot sender and receiver.
pub type OneshotChannel<S,R> = Channel<Option<oneshot::Sender<S>>, oneshot::Receiver<R>>;
/// Channel for broadcasting message, while receiving message from multiple producers.
pub type BroadcastChannel<S,R> = Channel<bus::Bus<S>, mpsc::Receiver<R>>;
/// Other side of a broadcast channel
pub type BroadcastChannelRev<S,R> = Channel<mpsc::Sender<R>, bus::BusReader<S>>;


/// Generic sender
pub trait ChannelSender : Sized+Unpin {
    type Item;
    type Receiver: ChannelReceiver<Item=Self::Item,Sender=Self>+Unpin;
    type Error;

    fn channel(cap: usize) -> (Self, Self::Receiver);
    fn try_send(&mut self, item: Self::Item) -> Result<(), Self::Error>;
    fn is_closed(&self) -> bool;
}

/// Generic Receiver
pub trait ChannelReceiver : Sized+Unpin {
    type Item;
    type Sender: ChannelSender<Item=Self::Item,Receiver=Self>+Unpin;
    type Error;

    /// Create a mono-directional channel
    fn channel(cap: usize) -> (Self::Sender, Self) {
        Self::Sender::channel(cap)
    }

    /// Try receive an item
    fn try_recv(&mut self) -> Result<Option<Self::Item>, Self::Error>;
}

/// Marker for MPSC senders and receivers
pub trait MPSC : Clone {}


impl<S,R> Channel<S,R>
    where S: Unpin+ChannelSender, R: Unpin+ChannelReceiver
{
    /// Create a channel.
    pub fn new(sender: S, receiver: R) -> Self {
        Self { receiver, sender }
    }

    /// Create a bidirectionnal channel.
    pub fn channel(cap: usize) -> (Self,Channel<R::Sender, S::Receiver>) {
        let (rs, rr) = R::channel(cap);
        let (ss, sr) = S::channel(cap);
        (Self::new(ss, rr), Channel::new(rs, sr))
    }
}


impl<T> ChannelSender for mpsc::Sender<T> {
    type Item = T;
    type Receiver = mpsc::Receiver<T>;
    type Error = mpsc::TrySendError<Self::Item>;

    fn channel(cap: usize) -> (Self, Self::Receiver) {
        mpsc::channel(cap as usize)
    }

    fn try_send(&mut self, item: Self::Item) -> Result<(), Self::Error> {
        <mpsc::Sender<T>>::try_send(self, item)
    }

    fn is_closed(&self) -> bool {
        <mpsc::Sender<T>>::is_closed(self)
    }
}

impl<T> MPSC for mpsc::Sender<T> {}



impl<T> ChannelReceiver for mpsc::Receiver<T> {
    type Item = T;
    type Sender = mpsc::Sender<T>;
    type Error = mpsc::TryRecvError;

    fn try_recv(&mut self) -> Result<Option<Self::Item>, Self::Error> {
        self.try_next()
    }
}


impl<T> ChannelSender for Option<oneshot::Sender<T>> {
    type Item = T;
    type Receiver = oneshot::Receiver<Self::Item>;
    type Error = T;

    fn channel(_cap: usize) -> (Self, Self::Receiver) {
        let (s,r) = oneshot::channel();
        (Some(s), r)
    }

    fn try_send(&mut self, item: Self::Item) -> Result<(), Self::Error> {
        match self.take() {
            Some(s) => s.send(item),
            None => Err(item),
        }
    }

    fn is_closed(&self) -> bool {
        match self {
            None => true,
            Some(s) => s.is_canceled(),
        }
    }
}

impl<T> ChannelReceiver for oneshot::Receiver<T> {
    type Item = T;
    type Sender = Option<oneshot::Sender<T>>;
    type Error = oneshot::Canceled;

    fn try_recv(&mut self) -> Result<Option<Self::Item>, Self::Error> {
        self.try_recv()
    }
}


impl<T: Clone+Sync> ChannelSender for bus::Bus<T> {
    type Item = T;
    type Receiver = bus::BusReader<Self::Item>;
    type Error = T;

    fn channel(cap: usize) -> (Self, Self::Receiver) {
        let mut bus = bus::Bus::new(cap as usize);
        let reader = bus.add_rx();
        (bus, reader)
    }

    fn try_send(&mut self, item: Self::Item) -> Result<(), Self::Error> {
        self.try_broadcast(item)
    }

    fn is_closed(&self) -> bool {
        false
    }
}

impl<T: Clone+Sync> ChannelReceiver for bus::BusReader<T> {
    type Item = T;
    type Sender = bus::Bus<T>;
    type Error = TryRecvError;

    fn try_recv(&mut self) -> Result<Option<Self::Item>, Self::Error> {
        self.try_recv().map(|o| Some(o))
    }
}


