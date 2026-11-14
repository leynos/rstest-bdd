//! Bounded draining of a nested command's output pipes.
//!
//! A pipe is drained on a background thread into a channel, so the waiting
//! side can take what has arrived without depending on EOF. That distinction
//! only matters after a timeout: killing a child leaves any grandchildren it
//! spawned holding the same pipe write ends, and joining a `read_to_end`
//! thread would then wait for those descendants rather than for the bound the
//! caller asked for. A nine-minute preparation bound that outlived its child
//! this way was observed to run until the test runner killed the whole test.

use std::{
    io::Read,
    sync::mpsc::{Receiver, RecvTimeoutError, channel},
    thread,
    time::Instant,
};

/// Bytes read from the pipe per iteration.
const CHUNK: usize = 8 * 1024;

/// One child pipe drained on a background thread.
pub(super) struct Drain {
    /// Chunks as they arrive; the sender drops when the pipe reaches EOF.
    chunks: Receiver<Vec<u8>>,
}

impl Drain {
    /// Start draining `pipe` and return without waiting for output.
    ///
    /// The reader thread is detached: it ends when the pipe reaches EOF or
    /// when the receiver is dropped, and nothing here waits for either.
    pub(super) fn spawn(pipe: impl Read + Send + 'static) -> Self {
        let (sender, chunks) = channel();
        let _ = thread::spawn(move || {
            let mut pipe = pipe;
            let mut buffer = [0_u8; CHUNK];
            loop {
                match pipe.read(&mut buffer) {
                    Ok(0) | Err(_) => break,
                    Ok(read) => {
                        let Some(chunk) = buffer.get(..read) else {
                            break;
                        };
                        if sender.send(chunk.to_vec()).is_err() {
                            break;
                        }
                    }
                }
            }
        });
        Self { chunks }
    }

    /// Collect what the pipe has produced, stopping at EOF or at `stop`.
    ///
    /// Returns everything read so far when `stop` arrives first, so a caller
    /// that has already abandoned the child still reports its output instead
    /// of blocking on a write end some descendant still holds.
    pub(super) fn collect_until(&mut self, stop: Instant) -> Vec<u8> {
        let mut collected = Vec::new();
        while let Some(remaining) = stop.checked_duration_since(Instant::now()) {
            match self.chunks.recv_timeout(remaining) {
                Ok(chunk) => collected.extend_from_slice(&chunk),
                Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => break,
            }
        }
        collected
    }
}

#[cfg(test)]
mod tests {
    //! Draining behaviour that does not need a real child process.

    use std::{
        io::Write,
        time::{Duration, Instant},
    };

    use super::Drain;

    #[test]
    fn collect_until_returns_buffered_output_at_eof() {
        let (reader, mut writer) = std::io::pipe().expect("pipe");
        let mut drain = Drain::spawn(reader);
        writer.write_all(b"hello").expect("write");
        drop(writer);
        let collected = drain.collect_until(Instant::now() + Duration::from_secs(10));
        assert_eq!(collected, b"hello");
    }

    #[test]
    fn collect_until_returns_at_the_stop_time_while_the_pipe_stays_open() {
        // The writer stays alive, which is exactly the state of a pipe whose
        // write end an orphaned grandchild still holds. The bound must still
        // be honoured.
        let (reader, mut writer) = std::io::pipe().expect("pipe");
        let mut drain = Drain::spawn(reader);
        writer.write_all(b"partial").expect("write");

        let started = Instant::now();
        let collected = drain.collect_until(started + Duration::from_millis(50));
        assert_eq!(collected, b"partial");
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "the stop time must bound the wait: {:?}",
            started.elapsed()
        );
        drop(writer);
    }
}
