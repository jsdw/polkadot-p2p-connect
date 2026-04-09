mod mock_stream;

pub use mock_stream::{MockStream, MockStreamHandle};

/// Poll some async task, returning Some(result) if Poll::Ready
/// and None if Poll::Pending.
pub fn block_on<F: core::future::Future>(f: F) -> Option<F::Output> {
    use alloc::sync::Arc;
    use alloc::task::Wake;
    use core::pin::pin;
    use core::task::{Context, Poll, Waker};

    struct NoopWaker;
    impl Wake for NoopWaker {
        fn wake(self: Arc<Self>) {}
    }

    let waker = Waker::from(Arc::new(NoopWaker));
    let mut cx = Context::from_waker(&waker);
    let mut f = pin!(f);
    match f.as_mut().poll(&mut cx) {
        Poll::Ready(v) => Some(v),
        Poll::Pending => None,
    }
}
