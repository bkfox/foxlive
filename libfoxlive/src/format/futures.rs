use futures;
use std::marker::Unpin;

use super::error::Error;


pub type PollValue = Result<(), Error>;
pub type Poll = futures::task::Poll<PollValue>;
pub type Future = dyn futures::Future<Output=PollValue>+Unpin;


/// Transform Poll::Ready(Ok) to Poll::Pending
pub fn pending_or_err(poll: Poll) -> Poll {
    match poll {
        Poll::Ready(Ok(_)) => Poll::Pending,
        _ => poll
    }
}

/// Return a Poll<Result<(), Error>> from provided ffmpeg function result
macro_rules! ToPoll {
    ($err:ident, $r: ident) => {{
        use libc::{EDOM,EAGAIN};

        // EOF
        if $r == -541478725 {
            return Poll::Ready(Ok(()));
        }

        // cf. AVERROR macros definitions
        let err = if EDOM as i32 > 0 { -$r } else { $r };
        match err {
            EAGAIN => Poll::Pending,
            _ => Poll::Ready(Err(AVError!($err, $r))),
        }
    }}
}




