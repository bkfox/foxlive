//! Provide multi-thread utilities for communicating between entities.
use crossbeam_channel::{bounded,Receiver,Sender};
pub use crossbeam_channel::{RecvError,TryRecvError,TryIter,SendError,TrySendError};

/// Bidirectional channel
pub struct BiChannel<R,S> {
    pub receiver: Receiver<R>,
    pub sender: Sender<S>,
}

impl<R,S> BiChannel<R,S> {
    pub fn bounded<R_,S_>(cap: usize) -> (BiChannel<R_,S_>, BiChannel<S_,R_>) {
        let (s1, r1) = bounded(cap);
        let (s2, r2) = bounded(cap);
        (BiChannel { receiver: r1, sender: s2 }, BiChannel { receiver: r2, sender: s1 })
    }

    pub fn try_recv(&self) -> Result<R, TryRecvError> {
        self.receiver.try_recv()
    }

    pub fn recv(&self) -> Result<R, RecvError> {
        self.receiver.recv()
    }

    pub fn recv_try_iter(&self) -> TryIter<R> {
        self.receiver.try_iter()
    }

    pub fn try_send(&self, msg: S) -> Result<(), TrySendError<S>> {
        self.sender.try_send(msg)
    }

    pub fn send(&self, msg: S) -> Result<(), SendError<S>> {
        self.sender.send(msg)
    }
}

