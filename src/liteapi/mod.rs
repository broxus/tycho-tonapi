pub mod codec;
pub mod connection;

#[derive(thiserror::Error, Debug)]
pub enum AdnlError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("too big packet: {0} bytes")]
    TooBigPacket(usize),
    #[error("too small packet: {0} bytes")]
    TooSmallPacket(usize),
    #[error("invalid checksum")]
    InvalidChecksum,
    #[error("invalid public key")]
    InvalidPubkey,
    #[error("unknown public key")]
    UnknownPubkey,
}
