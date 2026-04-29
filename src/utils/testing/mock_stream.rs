use crate::utils::async_stream::{AsyncRead, AsyncReadError, AsyncWrite, AsyncWriteError};
use alloc::collections::VecDeque;
use alloc::rc::Rc;
use alloc::vec::Vec;
use core::cell::RefCell;

/// A Mock stream implementation which implements [`AsyncStream`] and can
/// be used in single threaded contexts for exampels and testing.
#[derive(Debug, Default, Clone)]
pub struct MockStream {
    inner: Rc<RefCell<MockStreamInner>>,
}

/// The handle to a [`MockStream`]. This can write data to the buffer that is
/// read when [`MockStream::read_exact`] is called, and can read data from the
/// buffer that is written too when [`MockStream::write_all`] is called.
#[derive(Debug, Clone)]
pub struct MockStreamHandle {
    inner: Rc<RefCell<MockStreamInner>>,
}

#[derive(Debug, Clone, Default)]
struct MockStreamInner {
    read_buf: VecDeque<u8>,
    write_buf: VecDeque<u8>,
}

impl MockStream {
    /// Create a new, empty, [`MockStream`]. Use [`MockStream::handle`]
    /// to read and write bytes to this stream for consumption.
    pub fn new() -> Self {
        Self {
            inner: Rc::new(RefCell::new(MockStreamInner {
                read_buf: Default::default(),
                write_buf: Default::default(),
            })),
        }
    }

    /// Create a handle to this [`MockStream`] which allows bytes to be read
    /// and written through it.
    pub fn handle(&self) -> MockStreamHandle {
        MockStreamHandle {
            inner: self.inner.clone(),
        }
    }
}

/// Future returned by [`MockStream::read_exact`].
///
/// Re-checks the buffer on every poll so that data added between polls is
/// found, rather than getting permanently stuck inside `pending().await`.
struct MockReadExact<'a> {
    inner: Rc<RefCell<MockStreamInner>>,
    buf: &'a mut [u8],
}

impl<'a> core::future::Future for MockReadExact<'a> {
    type Output = Result<(), AsyncReadError>;

    fn poll(mut self: core::pin::Pin<&mut Self>, _cx: &mut core::task::Context<'_>) -> core::task::Poll<Self::Output> {
        let needed = self.buf.len();
        // Drain into a temporary vec first so that we can drop the `RefMut`
        // borrow before writing into `self.buf` (accessing two fields through
        // `Pin<&mut Self>` at the same time confuses the borrow checker).
        let data: Option<Vec<u8>> = {
            let mut borrow = self.inner.borrow_mut();
            if borrow.read_buf.len() < needed {
                None
            } else {
                Some(borrow.read_buf.drain(..needed).collect())
            }
        };
        match data {
            None => core::task::Poll::Pending,
            Some(data) => {
                self.buf.copy_from_slice(&data);
                core::task::Poll::Ready(Ok(()))
            }
        }
    }
}

impl AsyncRead for MockStream {
    fn read_exact(&mut self, buf: &mut [u8]) -> impl core::future::Future<Output = Result<(), AsyncReadError>> {
        MockReadExact { inner: self.inner.clone(), buf }
    }
}

impl AsyncWrite for MockStream {
    async fn write_all(&mut self, data: &[u8]) -> Result<(), AsyncWriteError> {
        self.inner.borrow_mut().write_buf.extend(data);
        Ok(())
    }
}

impl MockStreamHandle {
    /// Push a byte to the buffer that will be read when [`MockStream::read_exact`] is used.
    pub fn push(&self, byte: u8) {
        self.inner.borrow_mut().read_buf.push_back(byte);
    }

    /// Drain `n` bytes from the buffer that is written too when [`MockStream::write_all`] is
    /// used, from the oldest to newest byte written.
    pub fn drain(&self, n: usize) -> Vec<u8> {
        let mut inner = self.inner.borrow_mut();
        inner.write_buf.drain(..n).collect()
    }

    /// Drain all bytes from the write buffer.
    pub fn drain_all(&self) -> Vec<u8> {
        let mut inner = self.inner.borrow_mut();
        inner.write_buf.drain(..).collect()
    }
}

impl Extend<u8> for MockStreamHandle {
    fn extend<T: IntoIterator<Item = u8>>(&mut self, iter: T) {
        for b in iter.into_iter() {
            self.push(b);
        }
    }
}
