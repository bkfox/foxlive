use std::ops::{Index,IndexMut};


struct Frame<'a, S: 'a> {
    samples: &'a [S],
}


impl<'a,S:'a> Index<NChannel> for Frame<'a,S> {
    type Output = S;

    fn index(&self, index: NChannel) -> &Self::Output {
        return self.samples[index as usize];
    }
}


