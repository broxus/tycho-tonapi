use bytes::Bytes;
use tl_proto::{TlError, TlPacket, TlRead, TlResult, TlWrite};
pub use tycho_block_util::tl::{block_id, hash_bytes, shard_ident};
use tycho_core::global_config::ZerostateId;
use tycho_types::cell::HashBytes;
use tycho_types::models::{BlockId, ShardIdent, StdAddr};

pub mod rpc {
    use super::*;

    // === liteServer.getMasterchainInfo ===

    #[derive(Debug, TlRead, TlWrite)]
    #[tl(
        boxed,
        id = "liteServer.getMasterchainInfo",
        scheme = "../proto/lite_api.tl"
    )]
    pub struct GetMasterchainInfoRequest;

    #[derive(Debug, TlRead, TlWrite)]
    #[tl(
        boxed,
        id = "liteServer.masterchainInfo",
        scheme = "../proto/lite_api.tl"
    )]
    pub struct GetMasterchainInfoResponse {
        #[tl(with = "block_id")]
        pub last: BlockId,
        #[tl(with = "hash_bytes")]
        pub state_root_hash: HashBytes,
        pub init_workchain: i32,
        #[tl(with = "zerostate_id")]
        pub init_zerostate_id: ZerostateId,
    }

    // === liteServer.getMasterchainInfoExt ===

    #[derive(Debug, TlRead, TlWrite)]
    #[tl(
        boxed,
        id = "liteServer.getMasterchainInfoExt",
        scheme = "../proto/lite_api.tl"
    )]
    pub struct GetMasterchainInfoExtRequest {
        pub mode: i32,
    }

    #[derive(Debug, TlRead, TlWrite)]
    #[tl(
        boxed,
        id = "liteServer.masterchainInfoExt",
        scheme = "../proto/lite_api.tl"
    )]
    pub struct GetMasterchainInfoExtResponse {
        pub mode: i32,
        pub version: u32,
        pub capabilities: u64,
        #[tl(with = "block_id")]
        pub last: BlockId,
        pub last_utime: u32,
        pub now: u32,
        #[tl(with = "hash_bytes")]
        pub state_root_hash: HashBytes,
        pub init_workchain: i32,
        #[tl(with = "zerostate_id")]
        pub init_zerostate_id: ZerostateId,
    }

    // === liteServer.getTime ===

    #[derive(Debug, TlRead, TlWrite)]
    #[tl(boxed, id = "liteServer.getTime", scheme = "../proto/lite_api.tl")]
    pub struct GetTimeRequest;

    #[derive(Debug, TlRead, TlWrite)]
    #[tl(boxed, id = "liteServer.currentTime", scheme = "../proto/lite_api.tl")]
    pub struct GetTimeResponse {
        pub now: u32,
    }

    // === liteServer.getVersion ===

    #[derive(Debug, TlRead, TlWrite)]
    #[tl(boxed, id = "liteServer.getVersion", scheme = "../proto/lite_api.tl")]
    pub struct GetVersionRequest;

    #[derive(Debug, TlRead, TlWrite)]
    #[tl(boxed, id = "liteServer.version", scheme = "../proto/lite_api.tl")]
    pub struct GetVersionResponse {
        pub mode: u32,
        pub version: u32,
        pub capabilities: u64,
        pub now: u32,
    }

    // === liteServer.getBlock ===

    #[derive(Debug, TlRead, TlWrite)]
    #[tl(boxed, id = "liteServer.getBlock", scheme = "../proto/lite_api.tl")]
    pub struct GetBlockRequest {
        #[tl(with = "block_id")]
        pub id: BlockId,
    }

    #[derive(Debug, TlRead, TlWrite)]
    #[tl(boxed, id = "liteServer.blockData", scheme = "../proto/lite_api.tl")]
    pub struct GetBlockResponse {
        #[tl(with = "block_id")]
        pub id: BlockId,
        pub data: Bytes,
    }

    // === liteServer.getAccountState ===

    #[derive(Debug, TlRead, TlWrite)]
    #[tl(
        boxed,
        id = "liteServer.getAccountState",
        scheme = "../proto/lite_api.tl"
    )]
    pub struct GetAccountStateRequest {
        #[tl(with = "block_id")]
        pub id: BlockId,
        #[tl(with = "account_id")]
        pub account: StdAddr,
    }

    #[derive(Debug, TlRead, TlWrite)]
    #[tl(boxed, id = "liteServer.accountState", scheme = "../proto/lite_api.tl")]
    pub struct GetAccountStateResponse {
        #[tl(with = "block_id")]
        pub id: BlockId,
        #[tl(with = "block_id")]
        pub shard_block_id: BlockId,
        pub shard_proof: Bytes,
        pub proof: Bytes,
        pub state: Bytes,
    }

    // === liteServer.getShardInfo ===

    #[derive(Debug, TlRead, TlWrite)]
    #[tl(boxed, id = "liteServer.getShardInfo", scheme = "../proto/lite_api.tl")]
    pub struct GetShardInfoRequest {
        #[tl(with = "block_id")]
        pub id: BlockId,
        #[tl(with = "shard_ident")]
        pub shard: ShardIdent,
        pub exact: bool,
    }

    #[derive(Debug, TlRead, TlWrite)]
    #[tl(boxed, id = "liteServer.shardInfo", scheme = "../proto/lite_api.tl")]
    pub struct GetShardInfoResponse {
        #[tl(with = "block_id")]
        pub id: BlockId,
        #[tl(with = "block_id")]
        pub shard_block_id: BlockId,
        pub shard_proof: Bytes,
        pub shard_descr: Bytes,
    }

    // === liteServer.getAllShardsInfo ===

    #[derive(Debug, TlRead, TlWrite)]
    #[tl(
        boxed,
        id = "liteServer.getAllShardsInfo",
        scheme = "../proto/lite_api.tl"
    )]
    pub struct GetAllShardsInfoRequest {
        #[tl(with = "block_id")]
        pub id: BlockId,
    }

    #[derive(Debug, TlRead, TlWrite)]
    #[tl(
        boxed,
        id = "liteServer.allShardsInfo",
        scheme = "../proto/lite_api.tl"
    )]
    pub struct GetAllShardsInfoResponse {
        #[tl(with = "block_id")]
        pub id: BlockId,
        pub proof: Bytes,
        pub data: Bytes,
    }

    // === liteServer.lookupBlock ===

    #[derive(Debug, TlRead, TlWrite)]
    #[tl(boxed, id = "liteServer.lookupBlock", scheme = "../proto/lite_api.tl")]
    pub struct LookupBlockRequest {
        #[tl(flags)]
        pub mode: (),
        #[tl(with = "block_id")]
        pub id: BlockId,
        #[tl(flags_bit = 1)]
        pub lt: Option<u64>,
        #[tl(flags_bit = 2)]
        pub utime: Option<u32>,
    }

    #[derive(Debug, TlRead, TlWrite)]
    #[tl(boxed, id = "liteServer.blockHeader", scheme = "../proto/lite_api.tl")]
    pub struct LookupBlockResponse {
        #[tl(with = "block_id")]
        pub id: BlockId,
        pub mode: u32,
        pub header_proof: Bytes,
    }

    // === liteServer.getLibraries ===

    #[derive(Debug, TlRead, TlWrite)]
    #[tl(boxed, id = "liteServer.getLibraries", scheme = "../proto/lite_api.tl")]
    pub struct GetLibrariesRequest {
        #[tl(with = "HashBytesVec::<16>")]
        pub library_list: Vec<HashBytes>,
    }

    #[derive(Debug, TlRead, TlWrite)]
    #[tl(
        boxed,
        id = "liteServer.libraryResult",
        scheme = "../proto/lite_api.tl"
    )]
    pub struct GetLibrariesResponse {
        pub items: Vec<LibraryEntry>,
    }
}

#[derive(Debug, TlRead, TlWrite)]
#[tl(boxed, id = "tcp.ping", scheme = "../proto/lite_api.tl")]
pub struct TcpPing {
    pub random_id: u64,
}

#[derive(Debug, TlRead, TlWrite)]
#[tl(boxed, id = "tcp.pong", scheme = "../proto/lite_api.tl")]
pub struct TcpPong {
    pub random_id: u64,
}

#[derive(Debug, TlRead, TlWrite)]
#[tl(boxed, id = "adnl.message.query", scheme = "../proto/lite_api.tl")]
pub struct AdnlQuery<'tl> {
    #[tl(with = "hash_bytes")]
    pub query_id: HashBytes,
    pub query: &'tl [u8],
}

#[derive(Debug, TlRead, TlWrite)]
#[tl(boxed, id = "adnl.message.answer", scheme = "../proto/lite_api.tl")]
pub struct AdnlAnswer {
    #[tl(with = "hash_bytes")]
    pub query_id: HashBytes,
    pub answer: Bytes,
}

#[derive(Debug, TlRead, TlWrite)]
#[tl(boxed, id = "liteServer.error", scheme = "../proto/lite_api.tl")]
pub struct LiteServerError {
    pub code: i32,
    pub message: Bytes,
}

#[derive(TlRead, TlWrite)]
#[tl(boxed, id = "liteServer.query", scheme = "../proto/lite_api.tl")]
pub struct LiteServerQuery<'tl> {
    pub data: &'tl [u8],
}

#[derive(TlRead, TlWrite)]
#[tl(boxed, id = "liteServer.queryPrefix", scheme = "../proto/lite_api.tl")]
pub struct LiteServerQueryPrefix;

#[derive(Debug, TlRead, TlWrite)]
#[tl(
    boxed,
    id = "liteServer.waitMasterchainSeqno",
    scheme = "../proto/lite_api.tl"
)]
pub struct WaitMasterchainSeqno {
    pub seqno: u32,
    pub timeout_ms: u32,
}

#[derive(Debug, TlRead, TlWrite)]
pub struct LibraryEntry {
    #[tl(with = "hash_bytes")]
    pub hash: HashBytes,
    pub data: Bytes,
}

pub mod account_id {
    use tycho_types::models::StdAddr;

    use super::*;

    pub const SIZE_HINT: usize = 1 + 32;

    pub const fn size_hint(_: &StdAddr) -> usize {
        SIZE_HINT
    }

    pub fn write<P: TlPacket>(addr: &StdAddr, packet: &mut P) {
        packet.write_i32(addr.workchain as i32);
        packet.write_raw_slice(addr.address.as_slice());
    }

    pub fn read(packet: &mut &[u8]) -> TlResult<StdAddr> {
        let Ok::<i8, _>(workchain) = i32::read_from(packet)?.try_into() else {
            return Err(TlError::InvalidData);
        };
        Ok(StdAddr::new(workchain, hash_bytes::read(packet)?))
    }
}

pub mod zerostate_id {
    use super::*;

    pub const SIZE_HINT: usize = 32 + 32;

    pub const fn size_hint(_: &ZerostateId) -> usize {
        SIZE_HINT
    }

    pub fn write<P: TlPacket>(zerostate_id: &ZerostateId, packet: &mut P) {
        packet.write_raw_slice(zerostate_id.root_hash.as_slice());
        packet.write_raw_slice(zerostate_id.file_hash.as_slice());
    }

    pub fn read(packet: &mut &[u8]) -> TlResult<ZerostateId> {
        Ok(ZerostateId {
            root_hash: hash_bytes::read(packet)?,
            file_hash: hash_bytes::read(packet)?,
        })
    }
}

pub mod block_id_short {
    use tycho_types::models::BlockIdShort;

    use super::*;

    pub const SIZE_HINT: usize = 4 + 8 + 4;

    pub const fn size_hint(_: &BlockIdShort) -> usize {
        SIZE_HINT
    }

    pub fn write<P: TlPacket>(block_id: &BlockIdShort, packet: &mut P) {
        shard_ident::write(&block_id.shard, packet);
        block_id.seqno.write_to(packet);
    }

    pub fn read(packet: &mut &[u8]) -> TlResult<BlockIdShort> {
        Ok(BlockIdShort {
            shard: shard_ident::read(packet)?,
            seqno: u32::read_from(packet)?,
        })
    }
}

struct HashBytesVec<const MAX_LEN: usize>;

impl<const MAX_LEN: usize> HashBytesVec<MAX_LEN> {
    pub fn size_hint(ids: &[HashBytes]) -> usize {
        4 + ids.len() * hash_bytes::SIZE_HINT
    }

    pub fn write<P: TlPacket>(items: &[HashBytes], packet: &mut P) {
        packet.write_u32(items.len() as u32);
        for item in items {
            hash_bytes::write(item, packet);
        }
    }

    pub fn read(packet: &mut &[u8]) -> TlResult<Vec<HashBytes>> {
        let len = u32::read_from(packet)?;
        if len as usize > MAX_LEN {
            return Err(TlError::InvalidData);
        }
        if packet.len() < len as usize * hash_bytes::SIZE_HINT {
            return Err(TlError::UnexpectedEof);
        }

        let mut items = Vec::with_capacity(len as usize);
        for _ in 0..len {
            items.push(hash_bytes::read(packet)?);
        }
        Ok(items)
    }
}

pub fn deserialize_exact<'a, T: TlRead<'a>>(data: &'a [u8]) -> TlResult<T> {
    let bytes = &mut data.as_ref();
    let parsed = T::read_from(bytes)?;
    if !bytes.is_empty() {
        return Err(TlError::InvalidData);
    }
    Ok(parsed)
}
