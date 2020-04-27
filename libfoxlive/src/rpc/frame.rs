use std::hash::Hash;
use std::marker::Unpin;


/// Data payload of a frame
#[derive(Clone)]
pub enum FramePayload<D> {
    /// Message data
    Data(D),
    /// Closes a request
    Close,
}

/// A frame used by multiplex for requests and responses.
pub trait Frame : Clone+Unpin {
    type Id: Copy+Eq+Hash+Unpin;
    type Data: Clone+Unpin+Send;

    /// Create a new frame
    fn create(id: Self::Id, payload: FramePayload<Self::Data>) -> Self;

    /// Create a new frame with provided data
    fn with_data(id: Self::Id, data: Self::Data) -> Self {
        Self::create(id, FramePayload::Data(data))
    }

    /// Return request id
    fn request_id(&self) -> Self::Id;

    /// Return some cancelled request if frame 
    fn payload(&self) -> FramePayload<&Self::Data>;

    /// Update frame payload
    fn set_payload(&mut self, payload: FramePayload<Self::Data>);

    /// Return payload data if any
    fn data(&self) -> Option<&Self::Data> {
        match self.payload() {
            FramePayload::Data(data) => Some(data),
            _ => None
        }
    }

    fn set_data(&mut self, data: Self::Data) {
        self.set_payload(FramePayload::Data(data))
    }

}


/// Simple frame that can be used for multiplex requests and responses.
#[derive(Clone)]
pub struct Message<D: Clone+Unpin+Send> {
    pub req: u32,
    pub payload: FramePayload<D>,
}


impl<D> FramePayload<D> {
    /// Return reference to inner FramePayload value
    pub fn as_ref(&self) -> FramePayload<&D> {
        match self {
            FramePayload::Data(ref v) => FramePayload::Data(v),
            FramePayload::Close => FramePayload::Close,
        }
    }

    /// Return mutable reference to inner FramePayload value
    pub fn as_mut(&mut self) -> FramePayload<&mut D> {
        match self {
            FramePayload::Data(ref mut v) => FramePayload::Data(v),
            FramePayload::Close => FramePayload::Close,
        }
    }
}

impl<D: Clone+Unpin+Send> Frame for Message<D> {
    type Id = u32;
    type Data = D;

    fn create(id: Self::Id, payload: FramePayload<Self::Data>) -> Self {
        Self { req: id, payload: payload }
    }

    fn request_id(&self) -> Self::Id {
        self.req
    }

    fn payload(&self) -> FramePayload<&Self::Data> {
        self.payload.as_ref()
    }

    fn set_payload(&mut self, payload: FramePayload<Self::Data>) {
        self.payload = payload;
    }
}

