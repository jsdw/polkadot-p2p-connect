use alloc::boxed::Box;
use alloc::string::String;

/// This should be implemented by the read half of some stream.
pub trait AsyncRead {
    /// Read enough bytes from the stream to fill the given buffer.
    fn read_exact(
        &mut self,
        buf: &mut [u8],
    ) -> impl core::future::Future<Output = Result<(), AsyncReadError>>;
}

/// An error that can occur when calling [`AsyncRead::read_exact`].
#[derive(Debug, thiserror::Error)]
#[error("cannot read from stream: {0}")]
pub struct AsyncReadError(Box<dyn core::error::Error + Send + Sync + 'static>);

impl AsyncReadError {
    /// Create an error relating to [`AsyncRead::read_exact`].
    pub fn new<E: core::error::Error + Send + Sync + 'static>(e: E) -> AsyncReadError {
        AsyncReadError(Box::new(e))
    }
    /// Create an error relating to [`AsyncRead::read_exact`] from a string.
    pub fn from_string(s: impl Into<String>) -> AsyncReadError {
        AsyncReadError(Box::new(StringError(s.into())))
    }
}

/// This should be implemented by the write half of some stream.
pub trait AsyncWrite {
    /// Write the entirity of the given bytes to the stream.
    fn write_all(
        &mut self,
        data: &[u8],
    ) -> impl core::future::Future<Output = Result<(), AsyncWriteError>>;
}

/// An error that can occur when calling [`AsyncWrite::write_all`].
#[derive(Debug, thiserror::Error)]
#[error("cannot write to stream: {0}")]
pub struct AsyncWriteError(Box<dyn core::error::Error + Send + Sync + 'static>);

impl AsyncWriteError {
    /// Create an error relating to [`AsyncWrite::write_all`].
    pub fn new<E: core::error::Error + Send + Sync + 'static>(e: E) -> AsyncWriteError {
        AsyncWriteError(Box::new(e))
    }
    /// Create an error relating to [`AsyncWrite::write_all`] from a string.
    pub fn from_string(s: impl Into<String>) -> AsyncWriteError {
        AsyncWriteError(Box::new(StringError(s.into())))
    }
}

#[derive(Debug)]
struct StringError(String);

impl core::fmt::Display for StringError {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        f.write_str(&self.0)
    }
}
impl core::error::Error for StringError {}
