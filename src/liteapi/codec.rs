// Based on https://github.com/tonstack/adnl-rs

use aes::Aes256;
use aes::cipher::{KeyIvInit, StreamCipher};
use bytes::{Buf, Bytes, BytesMut};
use ctr::Ctr128BE;
use rand::Rng;
use sha2::{Digest, Sha256};
use tokio_util::codec::{Decoder, Encoder};

use super::AdnlError;

pub type TcpAdnlCipher = Ctr128BE<Aes256>;

#[repr(transparent)]
pub struct AesParams([u8; Self::LEN]);

impl AesParams {
    pub const LEN: usize = 160;

    pub fn wrap(bytes: &[u8; Self::LEN]) -> &Self {
        // SAFETY: `HandshakeData` has the same layout as `[u8; Self::LEN]`.
        unsafe { &*(bytes as *const [u8; Self::LEN]).cast() }
    }

    pub fn rx_key(&self) -> &[u8; 32] {
        (&self.0[0..32]).try_into().unwrap()
    }

    pub fn tx_key(&self) -> &[u8; 32] {
        (&self.0[32..64]).try_into().unwrap()
    }

    pub fn rx_nonce(&self) -> &[u8; 16] {
        (&self.0[64..80]).try_into().unwrap()
    }

    pub fn tx_nonce(&self) -> &[u8; 16] {
        (&self.0[80..96]).try_into().unwrap()
    }

    pub fn as_bytes(&self) -> &[u8; Self::LEN] {
        &self.0
    }
}

impl From<&[u8; Self::LEN]> for AesParams {
    #[inline]
    fn from(value: &[u8; Self::LEN]) -> Self {
        Self(*value)
    }
}

impl rand::distr::Distribution<AesParams> for rand::distr::StandardUniform {
    #[inline]
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> AesParams {
        AesParams(rng.random())
    }
}

pub struct TcpAdnlCodec {
    rx_cipher: TcpAdnlCipher,
    tx_cipher: TcpAdnlCipher,
    packet_len: Option<usize>,
}

impl TcpAdnlCodec {
    pub fn client(params: &AesParams) -> Self {
        Self {
            rx_cipher: TcpAdnlCipher::new(params.rx_key().into(), params.rx_nonce().into()),
            tx_cipher: TcpAdnlCipher::new(params.tx_key().into(), params.tx_nonce().into()),
            packet_len: None,
        }
    }

    pub fn server(aes_params: &AesParams) -> Self {
        Self {
            rx_cipher: TcpAdnlCipher::new(aes_params.tx_key().into(), aes_params.tx_nonce().into()),
            tx_cipher: TcpAdnlCipher::new(aes_params.rx_key().into(), aes_params.rx_nonce().into()),
            packet_len: None,
        }
    }
}

impl Decoder for TcpAdnlCodec {
    type Item = Bytes;
    type Error = AdnlError;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        let packet_len = if let Some(len) = self.packet_len {
            len
        } else {
            if src.len() < 4 {
                return Ok(None);
            }
            self.rx_cipher.apply_keystream(&mut src[0..4]);
            let len = u32::from_le_bytes(src[0..4].try_into().unwrap()) as usize;
            if len < 64 {
                return Err(AdnlError::TooSmallPacket(len));
            } else if len > MAX_PACKET_LEN {
                return Err(AdnlError::TooBigPacket(len));
            }

            src.advance(4);
            self.packet_len = Some(len);
            len
        };

        if src.len() < packet_len {
            if src.capacity() < packet_len {
                src.reserve(packet_len - src.capacity());
            }
            return Ok(None);
        }

        self.packet_len = None;
        self.rx_cipher.apply_keystream(&mut src[..packet_len]);

        let hash_offset = packet_len - 32;
        if &*Sha256::digest(&src[..hash_offset]) != &src[hash_offset..packet_len] {
            return Err(AdnlError::InvalidChecksum);
        }

        let result = Bytes::copy_from_slice(&src[32..hash_offset]);
        src.advance(packet_len);
        Ok(Some(result))
    }
}

impl Encoder<Bytes> for TcpAdnlCodec {
    type Error = AdnlError;

    fn encode(&mut self, packet: Bytes, dst: &mut BytesMut) -> Result<(), Self::Error> {
        if packet.len() > MAX_PACKET_LEN {
            return Err(AdnlError::TooBigPacket(packet.len()));
        }

        let length = (packet.len() + 64) as u32;
        let nonce = rand::random::<[u8; 32]>();

        dst.reserve(packet.len() + 4 + 32 + 32);

        let encrypt_from = dst.len();
        dst.extend_from_slice(&length.to_le_bytes());
        let hash_from = dst.len();
        dst.extend_from_slice(&nonce);
        dst.extend_from_slice(&packet);

        let hash = Sha256::digest(&dst[hash_from..]);
        dst.extend_from_slice(&hash);

        self.tx_cipher.apply_keystream(&mut dst[encrypt_from..]);
        Ok(())
    }
}

const MAX_PACKET_LEN: usize = 1 << 24; // 16 MiB
