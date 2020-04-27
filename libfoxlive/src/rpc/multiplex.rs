/// Implement a simple multiplexing transport.
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::{Arc,RwLock};
use std::time::{Duration,Instant};

use futures::prelude::*;
use futures::channel::{mpsc,oneshot};
use futures::task::{Context,Poll};

use super::channel::{self,MPSCChannel,ChannelSender};
use super::frame::*;


/// Message multiplexer that can be used both client and server's side.
///
/// Its `Stream` implementation returns messages not handled by existing channel.
pub struct Multiplex<S,R>
    where S: Frame, R: Frame<Id=S::Id>,
{
    /// Wrapped transport
    transport: MPSCChannel<S,R>,
    /// In-flight requests
    channels: Channels<R>,
    /// Max in flight requests
    pub max_flying: usize,
    /// Request timeout
    pub timeout: Option<Duration>,
}


pub struct Channel<S,R>
    where S: Frame, R: Frame<Id=S::Id>,
{
    id: Option<S::Id>,
    multiplex: Arc<RwLock<Multiplex<S,R>>>,
    queue: channel::Channel<mpsc::Sender<S>,MxReceiver<R>>,
    timeout: Option<(Instant,Duration)>,
}


/// Frame receiver part of a channel.
pub enum MxReceiver<T> {
    None,
    Oneshot(oneshot::Receiver<T>),
    MPSC(mpsc::Receiver<T>),
}

/// Frame sender to channels' receivers.
pub enum MxSender<T> {
    None,
    Oneshot(Option<oneshot::Sender<T>>),
    MPSC(mpsc::Sender<T>),
}

/// channel::Channels by id.
type Channels<T> = HashMap<Option<<T as Frame>::Id>, MxSender<T>>;



pub fn multiplex<S,R>(max_flying: usize, timeout: Option<Duration>)
    -> (Channel<S,R>, MPSCChannel<R,S>)
    where S: Frame, R: Frame<Id=S::Id>
{
    let (transport, out) = MPSCChannel::channel(max_flying);
    (multiplex_from(transport, max_flying, timeout), out)
}

pub fn multiplex_from<S,R>(transport: MPSCChannel<S,R>, max_flying: usize, timeout: Option<Duration>)
    -> Channel<S,R>
    where S: Frame, R: Frame<Id=S::Id>
{
    let sender = transport.sender.clone();
    let mut multiplex = Multiplex::new(transport, max_flying, timeout);
    let receiver = multiplex.subscribe(None, || MxSender::channel(max_flying)).unwrap();
    Channel::new(None, Arc::new(RwLock::new(multiplex)), channel::Channel::new(sender, receiver),
                   timeout)
}


// -- Multiplex
impl<S,R> Multiplex<S,R>
    where S: Frame, R: Frame<Id=S::Id>,
{
    /// Create a new instance of Multiplex
    /// `cap` is the channel receiver capacity:
    /// - 0: Expects no message from Muliplex (`Channel`s polled from Multiplex stream);
    /// - 1: Expect a single message (e.g request's response);
    /// - > 1: Expect a stream of messages (e.g. request's responses);
    fn new(transport: MPSCChannel<S,R>, max_flying: usize, timeout: Option<Duration>) -> Self {
        Self {
            transport, max_flying, timeout,
            channels: Channels::with_capacity(max_flying),
        }
    }

    /// Close all multiplex's requests
    pub fn close(&mut self) {
        self.channels.clear();
    }

    /// Add a new channel
    fn subscribe(&mut self, id: Option<S::Id>, make: impl Fn() -> (MxSender<R>, MxReceiver<R>))
        -> Option<MxReceiver<R>>
    {
        let channel_exists = self.channels.get(&id).map(|c| !c.is_closed()).unwrap_or(false);
        if channel_exists || self.channels.len() >= self.max_flying {
            return None
        }

        let (sender, receiver) = make();
        self.channels.insert(id, sender);
        Some(receiver)
    }

    /// Remove a channel
    fn unsubscribe(&mut self, id: Option<S::Id>) {
        self.channels.remove(&id);
    }

    /// Handle an incoming message.
    fn handle_frame(&mut self, frame: R, id: Option<S::Id>) -> Option<R>
    {
        let rid = Some(frame.request_id());
        match (self.channels.get_mut(&rid), frame.payload()) {
            (None, FramePayload::Close) => {},
            // 1: Frame handled by a channel
            (Some(_), FramePayload::Close) => {
                self.unsubscribe(rid);
            }
            (Some(_), FramePayload::Data(_)) if rid == id => return Some(frame),
            (Some(channel), FramePayload::Data(_)) if !channel.is_closed() => {
                if channel.try_send(frame).is_err() {
                    self.unsubscribe(rid);
                }
            }
            // 2: Frame handled by default channel
            _ if id.is_none() => return Some(frame),
            _ => if let Some(channel) = self.channels.get_mut(&None) {
                channel.try_send(frame);
            }
        };
        None
    }

    /// Poll a frame from transport, return frame if any for the provided channel id.
    /// Unhandled frames are dispatched to default channel (returned if `id=None`).
    fn poll_frame(&mut self, cx: &mut Context, id: Option<S::Id>) -> Poll<Option<R>>
    {
        self.channels.retain(|_, ref mut channel| !channel.is_closed());

        if self.channels.len() >= self.max_flying {
            Poll::Pending
        }
        else {
            match Pin::new(&mut self.transport.receiver).poll_next(cx) {
                Poll::Ready(None) => Poll::Ready(None),
                Poll::Ready(Some(frame)) => match self.handle_frame(frame, id) {
                    None => Poll::Pending,
                    Some(r) => Poll::Ready(Some(r)),
                }
                _ => Poll::Pending,
            }
        }
    }
}


// -- Channel
impl<S,R> Channel<S,R>
    where S: Frame, R: Frame<Id=S::Id>,
{
    fn new(id: Option<S::Id>, multiplex: Arc<RwLock<Multiplex<S,R>>>, queue: channel::Channel<mpsc::Sender<S>, MxReceiver<R>>, timeout: Option<Duration>) -> Self {
        Self {
            id, multiplex, queue,
            timeout: timeout.map(|t| (Instant::now() + t, t)),
        }
    }

    /// Return channel id
    pub fn id(&self) -> Option<S::Id> {
        self.id
    }

    /// Return inner multiplex
    pub fn multiplex(&self) -> &Arc<RwLock<Multiplex<S,R>>> {
        &self.multiplex
    }

    /// Change channel's timeout, delaying expiration time.
    pub fn set_timeout(&mut self, timeout: Option<Duration>) {
        self.timeout = timeout.map(|t| (Instant::now() + t, t));
    }

    /// Close all multiplex's channel
    pub fn close_multiplex(&self) {
        if let Ok(mut mx) = self.multiplex.write() {
            mx.close()
        }
    }

    /// Create a new channel
    pub fn add_channel(&mut self, id: S::Id, make: impl Fn() -> (MxSender<R>, MxReceiver<R>))
        -> Option<Self>
    {
        if let Ok(mut mx) = self.multiplex.write() {
            mx.subscribe(Some(id), make).map(|receiver| 
                Self::new(Some(id), self.multiplex.clone(),
                          channel::Channel::new(mx.transport.sender.clone(), receiver),
                          mx.timeout)
            )
        } else { None }
    }

    /// Send a request and return channel awaiting a single response.
    pub async fn request(&mut self, frame: S) -> Option<Channel<S,R>> {
        match self.add_channel(frame.request_id(), || MxSender::oneshot_channel()) {
            None => None,
            Some(chan) => self.send(frame).await.ok().map(|_| chan)
        }
    }

    /// Send a request and return channel awaiting multiple responses.
    pub async fn request_stream(&mut self, frame: S) -> Option<Channel<S,R>> {
        let cap = if let Ok(cap) = self.multiplex.read().map(|mx| mx.max_flying) { cap }
                  else { return None };

        match self.add_channel(frame.request_id(), || MxSender::channel(cap)) {
            None => None,
            Some(chan) => self.send(frame).await.ok().map(|_| chan)
        }
    }

    /// Update expiration time, delaying from now to timeout.
    fn delay_timeout(&mut self) {
        if let Some((ref mut time, timeout)) = self.timeout {
            *time = Instant::now() + timeout;
        }
    }

    /// Return True if timeout expired
    fn is_timed_out(&self) -> bool {
        match self.timeout {
            Some((time, _)) if time < Instant::now() => true,
            _ => false,
        }
    }

    /// Poll multiplex for a frame
    fn poll_multiplex(&self, cx: &mut Context) -> Poll<Option<R>> {
        if let Ok(mut mx) = self.multiplex.write() {
            mx.poll_frame(cx, self.id)
        } else { Poll::Ready(None) }
    }
}

impl<S,R> Drop for Channel<S,R>
    where S: Frame, R: Frame<Id=S::Id>,
{
    fn drop(&mut self) {
        if let Ok(mut mx) = self.multiplex.write() {
            mx.unsubscribe(self.id);
        }
    }
}

impl<S,R> Stream for Channel<S,R>
    where S: Frame, R: Frame<Id=S::Id>,
{
    type Item = R;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        match Pin::new(&mut self.queue.receiver).poll_next(cx) {
            Poll::Pending => match self.poll_multiplex(cx) {
                Poll::Pending => if self.is_timed_out() { Poll::Ready(None) }
                                 else { Poll::Pending },
                Poll::Ready(r) => Poll::Ready(r),
            }
            Poll::Ready(Some(r)) => {
                self.delay_timeout();
                Poll::Ready(Some(r))
            },
            Poll::Ready(None) => Poll::Ready(None),
        }
    }
}

impl<S,R> Sink<S> for Channel<S,R>
    where S: Frame, R: Frame<Id=S::Id>,
{
    type Error = ();

    fn poll_ready(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Result<(), Self::Error>> {
        Pin::new(&mut self.queue.sender).poll_ready(cx).map_err(|_| ())
    }

    fn start_send(mut self: Pin<&mut Self> , item: S) -> Result<(), Self::Error>
    {
        Pin::new(&mut self.queue.sender).start_send(item).map_err(|_| ())
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Result<(), Self::Error>> {
        self.delay_timeout();
        Pin::new(&mut self.queue.sender).poll_flush(cx).map_err(|_| ())
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Result<(), Self::Error>> {
        Pin::new(&mut self.queue.sender).poll_close(cx).map_err(|_| ())
    }
}


// -- MxReceiver
impl<T> channel::ChannelReceiver for MxReceiver<T> {
    type Item = T;
    type Sender = MxSender<T>;
    type Error = ();

    fn try_recv(&mut self) -> Result<Option<Self::Item>, Self::Error> {
        match self {
            MxReceiver::Oneshot(ref mut r) => r.try_recv().or(Err(())),
            MxReceiver::MPSC(ref mut r) => r.try_next().or(Err(())),
            _ => Ok(None),
        }
    }
}

impl<T> Stream for MxReceiver<T> {
    type Item = T;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        match self.get_mut() {
            MxReceiver::MPSC(r) => Pin::new(r).poll_next(cx),
            MxReceiver::Oneshot(r) => Pin::new(r).poll(cx).map(|r| r.ok()),
            _ => Poll::Ready(None),
        }
    }
}


// -- MxSender
impl<T> MxSender<T> {
    pub fn no_channel() -> (Self, MxReceiver<T>) {
        (MxSender::None, MxReceiver::None)
    }

    pub fn oneshot_channel() -> (Self, MxReceiver<T>) {
        let (s,r) = oneshot::channel();
        (MxSender::Oneshot(Some(s)), MxReceiver::Oneshot(r))
    }
}

impl<T> channel::ChannelSender for MxSender<T> {
    type Item = T;
    type Receiver = MxReceiver<T>;
    type Error = ();

    /// Creates an MPSC channel. For oneshot, use oneshot_channel.
    fn channel(cap: usize) -> (Self, Self::Receiver) {
        let (s,r) = mpsc::channel(cap);
        (MxSender::MPSC(s), MxReceiver::MPSC(r))
    }

    fn try_send(&mut self, item: Self::Item) -> Result<(), Self::Error> {
        match self {
            MxSender::Oneshot(ref mut r) => r.try_send(item).or(Err(())),
            MxSender::MPSC(ref mut r) => r.try_send(item).or(Err(())),
            _ => Err(()),
        }
    }

    fn is_closed(&self) -> bool {
        match self {
            MxSender::Oneshot(r) => r.is_closed(),
            MxSender::MPSC(r) => r.is_closed(),
            _ => true,
        }
    }
}



#[cfg(test)]
mod test {
    use futures::join;
    use futures::executor::LocalPool;
    use futures_util::task::LocalSpawnExt;
    use super::*;

    pub type TestMessage = Message<u32>;

    #[test]
    fn test_simple_client() {
        let mut pool = LocalPool::new();
        let spawner = pool.spawner();
        let (mut client, mut server) = multiplex::<TestMessage,TestMessage>(10, None);

        // server
        spawner.spawn_local(async move {
            while let Some(frame) = server.receiver.next().await {
                println!("server: frame {}", frame.request_id());
                let resp = *frame.data().unwrap()*2;
                let r = server.sender.send(Message::with_data(frame.request_id(), resp)).await;
                println!("server: response sent... {:?}", r);
            }
            println!("server done");
        });

        // client
        spawner.spawn_local(async move {
            let mut reqs = Vec::new();
            for i in 0..4u32 {
                println!("client: request {}", i);
                let req = client.request(TestMessage::create(i, FramePayload::Data(i))).await;
                reqs.push(async move {
                    println!("client: wait for response {}", i);
                    let resp = req.unwrap().next().await;
                    println!("client: response {}", resp.is_some());
                    assert_eq!(*resp.unwrap().data().unwrap(), i as u32*2);
                });
            }

            future::join_all(reqs).await;
            println!("client done");
        });

        pool.run();
    }
}

