mod utils;

use core::marker::PhantomData;
use core::future::Future;
use utils::{
    async_stream,
    multistream,
    peer_id,
    noise,
    yamux,
    yamux_multistream,
};

#[derive(Debug, Clone)]
pub struct Configuration<Platform> {
    identity_secret: Option<[u8; 32]>,
    marker: PhantomData<(Platform,)>
}

impl <Platform: PlatformT> Configuration<Platform> {
    /// Create some new connection configuration.
    pub fn new() -> Self {
        Self {
            identity_secret: None,
            marker: PhantomData,
        }
    }

    pub fn with_fixed_identity(mut self) -> Self {
        let mut random_bytes = [0u8; 32]; 
        Platform::fill_with_random_values(&mut random_bytes);
        self.identity_secret = Some(random_bytes);
        self
    }

    pub fn with_identity(mut self, secret_bytes: [u8; 32]) -> Self {
        self.identity_secret = Some(secret_bytes);
        self
    }

    pub async fn connect<Stream: async_stream::AsyncStream>(&self, stream: Stream) -> Result<Connection<Stream, Platform>, Error> {
        Connection::from_stream(stream, self.identity_secret, None).await
    }

    pub async fn connect_to_peer<Stream: async_stream::AsyncStream>(&self, stream: Stream, peer_id: peer_id::PeerId) -> Result<Connection<Stream, Platform>, Error> {
        Connection::from_stream(stream, self.identity_secret, Some(peer_id)).await
    }
}

pub struct RequestResponseId(yamux_multistream::YamuxStreamId);

pub struct Connection<Stream, Platform> {
    yamux: yamux_multistream::YamuxMultistream<noise::NoiseStream<Stream>>,
    remote_id: peer_id::PeerId,
    marker: PhantomData<(Platform,)>
}

impl <Stream: async_stream::AsyncStream, Platform: PlatformT> Connection<Stream, Platform> {
    async fn from_stream(
        mut stream: Stream, 
        identity_secret: Option<[u8; 32]>,
        remote_peer_id: Option<peer_id::PeerId>
    ) -> Result<Self, Error> {
        const NEGOTIATE_TIMEOUT_MS: usize = 10_000;
        const NOISE_HANDSHAKE_TIMEOUT_MS: usize = 30_000;

        // Agree to use the noise protocol.
        Platform::timeout(NEGOTIATE_TIMEOUT_MS, multistream::negotiate_dialer(&mut stream, "/noise"))
            .await
            .map_err(|()| Error::NoiseNegotiationTimeout)?
            .map_err(Error::Multistream)?;

        // Generate/use an identity for ourselves.
        let identity = match identity_secret {
            Some(key) => peer_id::Identity::from_random_bytes(key),
            None => {
                let mut random_bytes = [0u8; 32]; 
                Platform::fill_with_random_values(&mut random_bytes);
                peer_id::Identity::from_random_bytes(random_bytes)
            }
        };

        // Establish our encypted noise session and find the remote Peer ID
        let (mut noise_stream, remote_id) = Platform::timeout(NOISE_HANDSHAKE_TIMEOUT_MS, noise::handshake_dialer(stream, &identity, remote_peer_id.as_ref()))
            .await
            .map_err(|()| Error::NoiseHandshakeTimeout)?
            .map_err(Error::Noise)?;

        // Agree to use the yamux protocol in this noise stream.
        Platform::timeout(NEGOTIATE_TIMEOUT_MS, multistream::negotiate_dialer(&mut noise_stream, "/yamux/1.0.0"))
            .await
            .map_err(|()| Error::YamuxNegotiationTimeout)?
            .map_err(Error::Multistream)?;
            
        // Wrap our noise stream in a yamux session (we'll be using yamux substreams), and wrap
        // that in a YamuxMultistream adaptor to handle multistream negotiation on top of these
        // substreams.
        let yamux_session = yamux::YamuxSession::new(noise_stream);
        let yamux_multistream = yamux_multistream::YamuxMultistream::new(yamux_session);

        Ok(Connection {
            yamux: yamux_multistream,
            remote_id,
            marker: PhantomData 
        })
    }

    pub fn request_response<P: Into<String>>(&mut self, protocol: &str, request: &[u8]) -> Result<RequestResponseId, Error> {
        let stream_id = self.yamux.open_stream(Some(protocol))?;

        // TODO: Save the request bytes somewhere and internally handle the message where the stream is accepted or rejected,
        // returning a suitable output.

        Ok(RequestResponseId(stream_id))
    }

    // pub fn id(&mut self) -> Id {
    //     todo!("Return an available opaque ID to use for the next stream")
    // }

    // pub fn request<F: FnOnce(Vec<u8>) -> Event>(&mut self, id: Id, stream: impl AsRef<str>, transform: F) {
    //     todo!("Send a request on the given request-response stream and transform the response to our event type.")
    // }

    // pub fn subscribe<V: FnOnce(Vec<u8>) -> bool, F: FnOnce(Vec<u8>) -> Event>(&mut self, id: Id, stream: impl AsRef<str>, handshake: Vec<u8>, validate: V, transform: F) {
    //     todo!("Subscribe to a notification stream, validating their handshake and transforming responses to our event type.")
    // }

    // pub async fn next(&mut self) -> Option<Event> {
    //     todo!("Return the next event, or None if no connection has been closed.")
    // }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("timeout negotiating noise stream")]
    NoiseNegotiationTimeout,
    #[error("timeout negotiating yamux stream")]
    YamuxNegotiationTimeout,
    #[error("timeout exchanging noise handshakes")]
    NoiseHandshakeTimeout,
    #[error("error negotiating multistream: {0}")]
    Multistream(#[from] multistream::Error),
    #[error("error establish noise encrypted stream: {0}")]
    Noise(#[from] noise::Error),
    #[error("yamux multistream error: {0}")]
    YamuxMultistream(#[from] yamux_multistream::Error)
}

pub trait PlatformT {
    /// Fill the given bytes with random values.
    fn fill_with_random_values(bytes: &mut [u8]);
    /// Returns Err(()) if the given future times out, else returns the output from the future.
    fn timeout<F: core::future::Future<Output = R>, R>(ms: usize, fut: F) -> impl Future<Output = Result<R, ()>>;
}
