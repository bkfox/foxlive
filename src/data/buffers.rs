use std::sync::{Arc,RwLock};

use smallvec::SmallVec;

use super::channels::NChannels;


/// Continguous samples buffer
pub type Buffer<S> = Vec<S>;

/// Container of multiple audio buffers
pub type Buffers<S> = SmallVec<[Buffer<S>; 2]>;

/// Container of multiple buffer slices
pub type Slices<'a,S> = SmallVec<[&'a mut [S]; 2]>;

/// Multi-owner shared buffers
pub type SharedBuffers<S> = Arc<RwLock<Buffers<S>>>;


/// Used to write data into a cache before flushing it into a shared Buffers.
/// This avoid to much lock on the shared buffer.
///
/// Flushing is done only when the cache reaches its reserved capacity
/// (or is forced to flush)
pub struct PreBuffer<S>
    where S: Copy+Clone
{
    pub caches: Buffers<S>,
    pub buffers: SharedBuffers<S>,
}

impl<S> PreBuffer<S>
    where S: Copy+Default
{
    // TODO: channels: usize -> NChannels
    pub fn new(channels: NChannels, buffers: Arc<RwLock<Buffers<S>>>) -> PreBuffer<S>
    {
        let mut caches = Buffers::with_capacity(channels as usize);

        // set up caches & buffers -- try to avoid reallocation
        // whenever possible
        {
            let ref mut buffers = *buffers.write().unwrap();

            // avoid reallocate contained vecs
            if buffers.len() > channels as usize {
                buffers.truncate(channels as usize);
            }
            else {
                buffers.reserve(channels as usize);
            }

            for i in 0..channels {
                caches.push(Buffer::new());
                if buffers.len() <= i as usize {
                    buffers.push(Buffer::new());
                }
            }
        }

        PreBuffer {
            caches: caches,
            buffers: buffers,
        }
    }

    pub fn channels(&self) -> NChannels {
        self.caches.len() as NChannels
    }

    /// Reserve this amount of data for buffers
    pub fn reserve_buffers(&mut self, size: usize) {
        let mut buffers = self.buffers.write().unwrap();
        for mut buffer in buffers.iter_mut() {
            buffer.reserve(size);
        }
    }

    /// Reserve this amount of sample for caches
    pub fn reserve_caches(&mut self, size: usize) {
        for mut cache in self.caches.iter_mut() {
            cache.reserve(size);
        }
    }

    /// Reserve this amount of data for caches
    pub fn resize_caches(&mut self, size: usize) {
        for mut cache in self.caches.iter_mut() {
            cache.resize(size, S::default());
        }
    }

    /// Flush caches into buffers
    pub fn flush(&mut self, force: bool) -> bool {
        if !force && self.caches[0].len() == self.caches[0].capacity()
        {
            return false
        }

        let mut buffers = self.buffers.write().unwrap();
        for (i, buffer) in buffers.iter_mut().enumerate() {
            buffer.extend(self.caches[i].iter());
            self.caches[i].clear();
        }
        true
    }
}




