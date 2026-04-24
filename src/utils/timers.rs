use crate::PlatformT;
use alloc::collections::VecDeque;
use alloc::sync::Arc;
use alloc::task::Wake;
use core::pin::Pin;
use core::sync::atomic::{AtomicBool, Ordering};
use core::task::{Context, Poll, Waker};
use core::time::Duration;

/// A set of timers, all with the same duration. New
/// values can be added, and calling [`Timers::try_next`]
/// will return the next elapsed value.
///
/// This is _not_ async because we don't want to wait for
/// the next timer to complete; only check if it is ready
/// periodically without waiting for it.
///
/// This internally reuses [`PlatformT::sleep()`] to do the
/// timing, which avoids needing additional platform methods
/// to fetch the current instant for example.
pub struct Timers<T, P: PlatformT> {
    poll_next: Arc<AtomicBool>,
    duration: Duration,
    timers: VecDeque<(T, P::Sleep)>,
}

impl<T, P: PlatformT> Timers<T, P> {
    /// Instantiate a new timer set. Every timer will have
    /// the given duration.
    pub fn new(duration: Duration) -> Self {
        Self {
            poll_next: Arc::new(AtomicBool::new(false)),
            duration,
            timers: Default::default(),
        }
    }

    /// Add a new timer to the set, which will return the given
    /// item upon completion.
    pub fn add(&mut self, item: T) {
        let fut = P::sleep(self.duration);
        self.timers.push_back((item, fut));
        if self.timers.len() == 1 {
            // If we've added the first item, we need to
            // make a note to try polling it.
            self.poll_next.store(true, Ordering::Relaxed);
        }
    }

    /// Return the next available ready value, if there is one.
    pub fn try_next(&mut self) -> Option<T> {
        // If our poll_next is false then we don't poll and return false.
        // we are waiting for our next fut to ask to be polled still.
        if self.poll_next.load(Ordering::Relaxed) == false {
            return None;
        }

        let (item, mut fut) = self.timers.pop_front()?;

        // A quick waker that, when called, sets our poll_next to true.
        struct PollWaker {
            poll_me: Arc<AtomicBool>,
        }
        impl Wake for PollWaker {
            fn wake(self: Arc<Self>) {
                self.poll_me.store(true, Ordering::Relaxed);
            }
        }
        let waker = Waker::from(Arc::new(PollWaker {
            poll_me: self.poll_next.clone(),
        }));

        // poll our future and either return the value or put it back in
        // the queue if it's not ready yet.
        let mut cx = Context::from_waker(&waker);
        match Pin::new(&mut fut).poll(&mut cx) {
            Poll::Pending => {
                self.poll_next.store(false, Ordering::Relaxed);
                self.timers.push_front((item, fut));
                None
            }
            Poll::Ready(()) => {
                // Leave poll_me set to true so that we
                // try the next item.
                Some(item)
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
}
