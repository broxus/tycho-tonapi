use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use aes::cipher::{KeyIvInit, StreamCipher};
use bytes::Bytes;
use futures_util::{Sink, SinkExt, Stream};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite};
use tokio_util::codec::{Decoder, Framed};
use tycho_crypto::ed25519;
use tycho_types::cell::HashBytes;

use super::AdnlError;
use super::codec::{AesParams, TcpAdnlCipher, TcpAdnlCodec};

pin_project_lite::pin_project! {
    pub struct AdnlConnection<T> {
        peer_id: HashBytes,
        #[pin]
        stream: Framed<T, TcpAdnlCodec>,
    }
}

impl<T> AdnlConnection<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    pub async fn accept<F>(mut transport: T, resolve_receiver: F) -> Result<Self, AdnlError>
    where
        F: Fn(&[u8; 32]) -> Option<Arc<ed25519::KeyPair>>,
    {
        let Handshake { peer_id, codec } =
            Handshake::receive(&mut transport, resolve_receiver).await?;

        let mut server = Self {
            peer_id,
            stream: codec.framed(transport),
        };

        server.send(Bytes::new()).await?;

        Ok(server)
    }

    pub fn peer_id(&self) -> &HashBytes {
        &self.peer_id
    }
}

impl<T> Stream for AdnlConnection<T>
where
    T: AsyncRead + AsyncWrite,
{
    type Item = Result<Bytes, AdnlError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.project().stream.poll_next(cx)
    }
}

impl<T> Sink<Bytes> for AdnlConnection<T>
where
    T: AsyncWrite + AsyncRead,
{
    type Error = AdnlError;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project().stream.poll_ready(cx)
    }

    fn start_send(self: Pin<&mut Self>, item: Bytes) -> Result<(), Self::Error> {
        self.project().stream.start_send(item)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project().stream.poll_flush(cx)
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project().stream.poll_close(cx)
    }
}

struct Handshake {
    peer_id: HashBytes,
    codec: TcpAdnlCodec,
}

impl Handshake {
    async fn receive<T, F>(transport: &mut T, resolve_receiver: F) -> Result<Self, AdnlError>
    where
        T: AsyncReadExt + Unpin,
        F: Fn(&[u8; 32]) -> Option<Arc<ed25519::KeyPair>>,
    {
        let mut handshake = [0u8; 256];
        transport.read_exact(&mut handshake).await?;

        let receiver: &[u8; 32] = handshake[0..32].try_into().unwrap();
        let Some(sender_pubkey) =
            ed25519::PublicKey::from_bytes(handshake[32..64].try_into().unwrap())
        else {
            return Err(AdnlError::InvalidPubkey);
        };

        let Some(keypair) = resolve_receiver(receiver) else {
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

        Ok(Self {
            peer_id: HashBytes(sender_pubkey.to_bytes()),
            codec: TcpAdnlCodec::server(AesParams::wrap(&*aes_params)),
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
        let keypair = Arc::new(ed25519::KeyPair::from(&secret));

        let listener = TcpListener::bind("0.0.0.0:12000").await?;
        loop {
            let (socket, addr) = listener.accept().await?;

            let keypair = keypair.clone();
            tokio::task::spawn(async move {
                println!("accepted connection from {addr}");

                let task = async {
                    let mut connection =
                        AdnlConnection::accept(socket, |_| Some(keypair.clone())).await?;
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
