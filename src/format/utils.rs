use futures;
use nix::errno::Errno;

use super::error::Error;


pub type Poll = futures::task::Poll<Result<(), Error>>;


/// Return a Poll<Result<(), Error>> from provided ffmpeg function result
macro_rules! ToPoll {
    ($err:ident, $r: ident) => {{
        // EOF
        if $r == -541478725 {
            return Poll::Ready(Ok(()));
        }

        // cf. AVERROR macros definitions
        let err = Errno::from_i32(if Errno::EDOM as i32 > 0 { -$r }
                                  else { $r });
        match err {
            Errno::EAGAIN => Poll::Pending,
            _ => Poll::Ready(Err(AVError!($err, $r))),
        }
    }}
}




