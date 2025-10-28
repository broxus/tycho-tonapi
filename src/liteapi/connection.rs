use std::pin::Pin;
use std::task::{Context, Poll};

use aes::cipher::{KeyIvInit, StreamCipher};
use bytes::Bytes;
use futures_util::{Sink, SinkExt, Stream};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite};
use tokio_util::codec::{FramedRead, FramedWrite};
use tycho_crypto::ed25519;
use tycho_types::cell::HashBytes;

use super::AdnlError;
use super::codec::{AesParams, TcpAdnlCipher, TcpAdnlDecoder, TcpAdnlEncoder};

pin_project_lite::pin_project! {
    pub struct AdnlConnection<I, O> {
        pub peer_id: HashBytes,
        #[pin]
        pub rx: FramedRead<I, TcpAdnlDecoder>,
        #[pin]
        pub tx: FramedWrite<O, TcpAdnlEncoder>,
    }
}

pub trait ResolveReceiver {
    fn resolve_receiver<'s, 'i>(&'s self, id: &'i [u8; 32]) -> Option<&'s ed25519::KeyPair>;
}

impl<T: ResolveReceiver> ResolveReceiver for &'_ T {
    #[inline]
    fn resolve_receiver<'s, 'i>(&'s self, id: &'i [u8; 32]) -> Option<&'s ed25519::KeyPair> {
        T::resolve_receiver(*self, id)
    }
}

impl ResolveReceiver for ed25519::KeyPair {
    #[inline]
    fn resolve_receiver<'s, 'i>(&'s self, _: &'i [u8; 32]) -> Option<&'s ed25519::KeyPair> {
        Some(self)
    }
}

impl<I, O> AdnlConnection<I, O>
where
    I: AsyncRead + Unpin,
    O: AsyncWrite + Unpin,
{
    pub async fn accept<R: ResolveReceiver>(
        mut rx: I,
        tx: O,
        resolver: R,
    ) -> Result<Self, AdnlError> {
        let Handshake {
            peer_id,
            decoder,
            encoder,
        } = Handshake::receive(&mut rx, resolver).await?;

        let mut server = Self {
            peer_id,
            rx: FramedRead::new(rx, decoder),
            tx: FramedWrite::new(tx, encoder),
        };

        server.tx.send(Bytes::new()).await?;

        Ok(server)
    }

    pub fn peer_id(&self) -> &HashBytes {
        &self.peer_id
    }
}

impl<I, O> Stream for AdnlConnection<I, O>
where
    I: AsyncRead,
{
    type Item = Result<Bytes, AdnlError>;

    #[inline]
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.project().rx.poll_next(cx)
    }
}

impl<I, O> Sink<Bytes> for AdnlConnection<I, O>
where
    O: AsyncWrite,
{
    type Error = AdnlError;

    #[inline]
    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Sink::<Bytes>::poll_ready(self.project().tx, cx)
    }

    #[inline]
    fn start_send(self: Pin<&mut Self>, item: Bytes) -> Result<(), Self::Error> {
        Sink::<Bytes>::start_send(self.project().tx, item)
    }

    #[inline]
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Sink::<Bytes>::poll_flush(self.project().tx, cx)
    }

    #[inline]
    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Sink::<Bytes>::poll_close(self.project().tx, cx)
    }
}

struct Handshake {
    peer_id: HashBytes,
    decoder: TcpAdnlDecoder,
    encoder: TcpAdnlEncoder,
}

impl Handshake {
    async fn receive<T, R>(rx: &mut T, resolver: R) -> Result<Self, AdnlError>
    where
        T: AsyncReadExt + Unpin,
        R: ResolveReceiver,
    {
        let mut handshake = [0u8; 256];
        rx.read_exact(&mut handshake).await?;

        let receiver: &[u8; 32] = handshake[0..32].try_into().unwrap();
        let Some(sender_pubkey) =
            ed25519::PublicKey::from_bytes(handshake[32..64].try_into().unwrap())
        else {
            return Err(AdnlError::InvalidPubkey);
        };

        let Some(keypair) = resolver.resolve_receiver(receiver) else {
            return Err(AdnlError::UnknownPubkey);
        };

        let hash: [u8; 32] = handshake[64..96].try_into().unwrap();

        let secret = keypair.compute_shared_secret(&sender_pubkey);
        let mut cipher = make_handshake_cipher(&secret, &hash);

        let aes_params: &mut [u8; 160] = (&mut handshake[96..256]).try_into().unwrap();
        cipher.apply_keystream(aes_params);

        if hash != &*Sha256::digest(&*aes_params) {
            return Err(AdnlError::InvalidChecksum);
        }

        let aes_params = AesParams::wrap(&*aes_params);

        Ok(Self {
            peer_id: HashBytes(sender_pubkey.to_bytes()),
            decoder: TcpAdnlDecoder::server(aes_params),
            encoder: TcpAdnlEncoder::server(aes_params),
        })
    }
}

fn make_handshake_cipher(secret: &[u8; 32], hash: &[u8]) -> TcpAdnlCipher {
    let mut key = [0u8; 32];
    key[..16].copy_from_slice(&secret[..16]);
    key[16..32].copy_from_slice(&hash[16..32]);

    let mut nonce = [0u8; 16];
    nonce[..4].copy_from_slice(&hash[..4]);
    nonce[4..16].copy_from_slice(&secret[20..32]);

    TcpAdnlCipher::new(key.as_slice().into(), nonce.as_slice().into())
}

#[cfg(test)]
mod test {
    use anyhow::Result;
    use futures_util::StreamExt;
    use tokio::net::TcpListener;

    use super::*;

    #[ignore]
    #[tokio::test]
    async fn echo_server() -> Result<()> {
        // pubkey: 2c674535d314bea7fd4020e7515d9b3c7525f895cdbc3ba523b7cddd2ac33c23
        let secret = "e10cea80da5cb2458799f524370d19c2060f79b8fd0cf285eddac11894a20769"
            .parse()
            .map(|HashBytes(bytes)| ed25519::SecretKey::from_bytes(bytes))
            .unwrap();
        let keypair = ed25519::KeyPair::from(&secret);

        let listener = TcpListener::bind("0.0.0.0:12000").await?;
        loop {
            let (socket, addr) = listener.accept().await?;

            let keypair = keypair.clone();
            tokio::task::spawn(async move {
                println!("accepted connection from {addr}");

                let task = async {
                    let (rx, tx) = socket.into_split();
                    let mut connection = AdnlConnection::accept(rx, tx, &keypair).await?;
                    println!(
                        "handshake complete for {addr}, peer_id={}",
                        connection.peer_id()
                    );

                    while let Some(packet) = connection.next().await {
                        let packet = packet?;
                        println!(
                            "got {} bytes from  {addr}, peer_id={}",
                            packet.len(),
                            connection.peer_id()
                        );
                        connection.send(packet).await?;
                    }

                    Ok::<_, anyhow::Error>(())
                };

                if let Err(e) = task.await {
                    println!("connection failed: {e}");
                }
            });
        }
    }
}
