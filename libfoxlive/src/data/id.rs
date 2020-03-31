use std::sync::atomic::{AtomicUsize, Ordering};


pub type Id = usize;

/// Provides a multithread generator for Id
pub struct IdGenerator {
    last: AtomicUsize,
}

impl IdGenerator {
    pub fn new() -> IdGenerator {
        IdGenerator {
            last: AtomicUsize::new(0),
        }
    }

    pub fn acquire(&mut self) -> Id {
        let last = self.last.load(Ordering::Acquire);
        self.last.store(last+1, Ordering::Release);
        last
    }
}



