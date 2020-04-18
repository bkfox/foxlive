/// Implement a Multiplex transport with the following features:
/// - regular futures' transport pipeline integration;
/// - send request outside the pipeline, e.g. for broadcasting;
/// - custom request/response format and id;
/// - expose infos of sent requests & various utility methods
///     such as `drop_timeout()`
///
/// In order to provide multiplexing, a Transport's Item must implemet
/// `RequestFrame`.
/// Multiplex's behaviour stay simple, without checking for lost requests
/// etc. Since it is not shareable by default, calling functions such as
/// request() is not always possible; this however can be done by wrapping
/// `Multiplex` transport into a `SharedTransport` instance.

use std::cmp::Eq;
use std::collections::HashMap;
use std::hash::Hash;
use std::ops::{Deref,DerefMut};
use std::pin::Pin;
use std::marker::{PhantomData,Unpin};

use futures::{Stream,Sink};
use futures::task::{Context,Poll};

use super::channel::*;


/// Provides support for multiplexing for a given frame
pub trait RequestFrame : Clone {
    type Id: Copy + Eq + Hash + Unpin;

    /// Return request id
    fn request_id(&self) -> Self::Id;

    /// Set request id
    fn set_request_id(&mut self, id: Self::Id);
}


/// Sender part used by multiplex to forward messages to requests.
pub enum RequestSender<T> {
    Oneshot(Option<oneshot::Sender<T>>),
    MPSC(mpsc::Sender<T>),
}

impl<T> RequestSender<T> {
    pub fn try_send(&mut self, v: T) -> Result<(), ()> {
        match self {
            Self::Oneshot(ref mut r) => {
                let r = r.take();
                if let Some(r) = r {
                    r.send(v).or(Err(())).or(Err(()))
                } else { Err(()) }
            },
            Self::MPSC(ref mut r) => r.try_send(v).or(Err(())),
        }
    }

    pub fn is_closed(&self) -> bool {
        match self {
            Self::Oneshot(Some(ref r)) => r.is_canceled(),
            Self::MPSC(ref r) => r.is_closed(),
            _ => true,
        }
    }

    pub fn is_oneshot(&self) -> bool {
        match self {
            Self::Oneshot(_) => true,
            _ => false,
        }
    }
}


/// A stream that will either receive/send multiple messages.
pub struct Request<S,R>
    where S: Unpin +RequestFrame, R: Unpin +RequestFrame<Id=S::Id>,
{
    pub id: R::Id,
    pub sender: mpsc::Sender<S>,
    pub receiver: mpsc::Receiver<R>,
}


pub type Requests<R> = HashMap<<R as RequestFrame>::Id, RequestSender<R>>;

// TODO: max requests

/// Multiplex message over multiple requests. It can be used both for message streaming
/// or oneshot request-response cycle, and both on client or server sides.
pub struct Multiplex<S,R,T>
    where S: Unpin +RequestFrame, R: Unpin +RequestFrame<Id=S::Id>,
          T: Sink<S> + Stream<Item=R> + Deref + DerefMut + Unpin
{
    /// Wrapped transport
    transport: T,
    /// In-flight requests
    pub requests: Requests<R>,
    pub channel: (mpsc::Sender<S>, mpsc::Receiver<S>),
    /// Channels' capacity
    channel_cap: usize,
    /// Max in flight requests
    max_flying: u32,
    phantom: PhantomData<S>,
}


impl<S,R,T> Multiplex<S,R,T>
    where S: Unpin +RequestFrame, R: Unpin +RequestFrame<Id=S::Id>,
          T: Sink<S> + Stream<Item=R> + Deref + DerefMut + Unpin
{
    /// Create a new instance of Multiplex
    pub fn new(transport: T, max_flying: u32, channel_cap: usize) -> Self {
        Self {
            transport, max_flying, channel_cap,
            requests: Requests::new(),
            channel: mpsc::channel(channel_cap),
            phantom: PhantomData,
        }
    }

    /// Create a new request stream without sending data
    pub fn add_stream(&mut self, id: R::Id)
        -> Option<Request<S,R>>
    {
        if self.requests.contains_key(&id) || self.requests.len() >= self.channel_cap {
            return None
        }

        let (sender, receiver) = mpsc::channel(self.channel_cap);
        self.requests.insert(id, RequestSender::MPSC(sender));
        Some(Request { id, receiver, sender: self.channel.0.clone() })
    }

    /// Send a request expecting a stream
    pub fn send_stream(&mut self, frame: S, cx: &mut Context)
        -> Option<Request<S,R>>
    {
        self.add_stream(frame.request_id())
            .and_then(|c| match self.send_raw(frame, cx) {
                Poll::Ready(Ok(_)) => Some(c),
                _ => None,
            })
    }

    /// Send a request expecting a single response
    pub fn send_request(&mut self, frame: S, cx: &mut Context)
        -> Option<oneshot::Receiver<R>>
    {
        let id = frame.request_id();
        if self.requests.contains_key(&id) {
            return None
        }

        if let Poll::Ready(Err(_)) = self.send_raw(frame, cx) {
            return None;
        }

        let (sender, receiver) = oneshot::channel();
        self.requests.insert(id, RequestSender::Oneshot(Some(sender)));
        Some(receiver)
    }

    /// Send a frame to wrapped transport and poll directly.
    pub fn send_raw(&mut self, frame: S, cx: &mut Context)
        -> Poll<Result<(), ()>>
    {
        let mut this = Pin::new(self);
        match this.as_mut().start_send(frame) {
            Ok(()) => this.poll_flush(cx).map_err(|_| ()),
            Err(_) => Poll::Ready(Err(())),
        }
    }

    /// Close multiplex's requests
    pub fn close(&mut self) {
        self.requests.clear();
    }

    /// Cancels a request
    pub fn cancel(&mut self, id: S::Id) {
        self.requests.remove(&id);
    }
}


impl<S,R,T> Deref for Multiplex<S,R,T>
    where S: Unpin +RequestFrame, R: Unpin +RequestFrame<Id=S::Id>,
          T: Sink<S> + Stream<Item=R> + Deref + DerefMut + Unpin
{
    type Target = Requests<R>;

    fn deref(&self) -> &Self::Target {
        &self.requests
    }
}


impl<S,R,T> DerefMut for Multiplex<S,R,T>
    where S: Unpin +RequestFrame, R: Unpin +RequestFrame<Id=S::Id>,
          T: Sink<S> + Stream<Item=R> + Deref + DerefMut + Unpin
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.requests
    }
}


impl<S,R,T> Stream for Multiplex<S,R,T>
    where S: Unpin +RequestFrame, R: Unpin +RequestFrame<Id=S::Id>,
          T: Sink<S>+Stream<Item=R> + Deref + DerefMut + Unpin
{
    type Item = R;

    /// Poll from transport, call handlers for responses and return
    /// the first frame that either is a request or a response that
    /// doesn't correspond to an existing request.
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>>
    {
        // send messages
        while let Ok(Some(frame)) = self.as_mut().channel.1.try_next() {
            self.as_mut().send_raw(frame, cx);
        };

        // removed closed channels
        let mx = self.get_mut();
        mx.requests.retain(|_, ref mut request| {
            request.is_closed()
        });

        // poll input
        for _ in 0..mx.max_flying {
            match Pin::new(&mut mx.transport).poll_next(cx) {
                Poll::Pending => break,
                Poll::Ready(None) => return Poll::Ready(None),
                Poll::Ready(Some(frame)) => {
                    let id = frame.request_id();
                    match mx.requests.get_mut(&id) {
                        None => return Poll::Ready(Some(frame)),
                        Some(ref mut request) => {
                            request.try_send(frame);
                            if request.is_oneshot() {
                                mx.requests.remove(&id);
                            }
                        }
                    }
                },
            }
        }

        Poll::Pending
    }
}


impl<S,R,T> Sink<S> for Multiplex<S,R,T>
    where S: Unpin +RequestFrame, R: Unpin +RequestFrame<Id=S::Id>,
          T: Sink<S> + Stream<Item=R> + Deref + DerefMut + Unpin
{
    type Error = T::Error;

    fn start_send(self: Pin<&mut Self>, item: S)
        -> Result<(), Self::Error>
    {
        Pin::new(&mut self.get_mut().transport).as_mut().start_send(item)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context)
        -> Poll<Result<(), Self::Error>>
    {
        Pin::new(&mut self.get_mut().transport).as_mut().poll_flush(cx)
    }

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context)
        -> Poll<Result<(), Self::Error>>
    {
        Pin::new(&mut self.get_mut().transport).poll_ready(cx)
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context)
        -> Poll<Result<(), Self::Error>>
    {
        Pin::new(&mut self.get_mut().transport).poll_close(cx)
    }
}


