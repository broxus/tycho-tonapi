use std::sync::Arc;

use anyhow::{Result, anyhow};
use bytes::{Buf, Bytes};
use serde::{Deserialize, Serialize};
use tl_proto::{TlError, TlWrite};
use tycho_crypto::ed25519;
use tycho_util::time::now_sec;

use self::server::{QueryContext, QueryHandler, TcpAdnlServer, TcpAdnlServerConfig};
use crate::state::{AppState, AtBlock, QueryBlock, StateError};

pub mod codec;
pub mod connection;
pub mod proto;
pub mod server;

const LS_VERSION: u32 = 0x101;
const LS_CAPABILITIES: u64 = 1;
const INIT_WORKCHAIN: i32 = -1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiteServerConfig {
    #[serde(flatten)]
    pub adnl: TcpAdnlServerConfig,

    /// Whether to answer to failed queries.
    ///
    /// Default: `true`
    pub answer_errors: bool,
}

pub struct LiteServer {
    state: AppState,
    answer_errors: bool,
}

impl LiteServer {
    pub async fn bind(
        state: AppState,
        keypair: Arc<ed25519::KeyPair>,
        config: LiteServerConfig,
    ) -> Result<TcpAdnlServer<Self>> {
        let query_handler = Self {
            state,
            answer_errors: config.answer_errors,
        };
        TcpAdnlServer::bind(keypair, config.adnl, query_handler).await
    }

    async fn handle_query_impl(&self, query: Bytes) -> Result<Bytes, LiteServerError> {
        let query = trim_query(query)?;

        match query.as_ref().get_u32_le() {
            proto::rpc::GetMasterchainInfoRequest::TL_ID => {
                let Some(info) = self.state.get_latest_mc_info() else {
                    return Err(LiteServerError::NotReady);
                };
                Ok(to_response(proto::rpc::GetMasterchainInfoResponse {
                    last: info.last_block_id,
                    state_root_hash: info.state_root_hash,
                    init_workchain: INIT_WORKCHAIN,
                    init_zerostate_id: info.zerostate_id,
                }))
            }
            proto::rpc::GetMasterchainInfoExtRequest::TL_ID => {
                let Some(info) = self.state.get_latest_mc_info() else {
                    return Err(LiteServerError::NotReady);
                };
                Ok(to_response(proto::rpc::GetMasterchainInfoExtResponse {
                    mode: 0,
                    version: LS_VERSION,
                    capabilities: LS_CAPABILITIES,
                    last: info.last_block_id,
                    last_utime: info.last_block_utime,
                    now: now_sec(),
                    state_root_hash: info.state_root_hash,
                    init_workchain: INIT_WORKCHAIN,
                    init_zerostate_id: info.zerostate_id,
                }))
            }
            proto::rpc::GetTimeRequest::TL_ID => {
                Ok(to_response(proto::rpc::GetTimeResponse { now: now_sec() }))
            }
            proto::rpc::GetVersionRequest::TL_ID => {
                Ok(to_response(proto::rpc::GetVersionResponse {
                    mode: 0,
                    version: LS_VERSION,
                    capabilities: LS_CAPABILITIES,
                    now: now_sec(),
                }))
            }
            proto::rpc::GetBlockRequest::TL_ID => {
                let proto::rpc::GetBlockRequest { id } = proto::deserialize_exact(&query)?;

                let res = self.state.get_block(&QueryBlock::ById(id)).await?;
                let Some(stream) = res.data else {
                    return Err(LiteServerError::Internal(anyhow!("block not found")));
                };

                // FIXME: Rework responses to allow using streams and don't allocate 2x buffer for response.
                let data = stream.into_inner();
                Ok(to_response(proto::rpc::GetBlockResponse { id, data }))
            }
            proto::rpc::GetAccountStateRequest::TL_ID => {
                let proto::rpc::GetAccountStateRequest { id, account } =
                    proto::deserialize_exact(&query)?;

                let res = self
                    .state
                    .get_account_state(&account, true, &AtBlock::ById(id))
                    .await?;
                let Some(data) = res.data else {
                    return Err(LiteServerError::Internal(anyhow!("block not found")));
                };

                Ok(to_response(proto::rpc::GetAccountStateResponse {
                    id,
                    shard_block_id: data.block_id,
                    shard_proof: Default::default(), // todo
                    proof: data.proof.unwrap_or_default(),
                    state: data.account_state.unwrap_or_default(),
                }))
            }
            proto::rpc::GetLibrariesRequest::TL_ID => {
                let proto::rpc::GetLibrariesRequest { mut library_list } =
                    proto::deserialize_exact(&query)?;

                library_list.sort_unstable();
                library_list.dedup();

                let mut items = Vec::with_capacity(library_list.len());
                for hash in library_list {
                    if let Some(data) = self.state.get_library_cell(&hash)?.data {
                        items.push(proto::LibraryEntry { hash, data });
                    }
                }

                Ok(to_response(proto::rpc::GetLibrariesResponse { items }))
            }
            _ => Err(LiteServerError::InvalidQuery(TlError::UnknownConstructor)),
        }
    }
}

impl QueryHandler for LiteServer {
    async fn handle_query(&self, _ctx: QueryContext<'_>, query: Bytes) -> Option<Bytes> {
        match self.handle_query_impl(query).await {
            Ok(answer) => Some(answer),
            Err(e) if self.answer_errors => {
                let res = tl_proto::serialize(proto::LiteServerError {
                    code: e.code(),
                    message: e.to_string().into(),
                });
                Some(res.into())
            }
            Err(_) => None,
        }
    }
}

fn trim_query(mut query: Bytes) -> Result<Bytes, TlError> {
    match proto::deserialize_exact(&query) {
        Ok(proto::LiteServerQuery { data }) => {
            if data.len() < 4 {
                return Err(TlError::UnexpectedEof);
            }
            Ok(query.slice_ref(data))
        }
        Err(_) if query.len() >= 8 => {
            if query.get_u32_le() != proto::LiteServerQueryPrefix::TL_ID {
                return Err(TlError::UnknownConstructor);
            }
            return Ok(query);
        }
        _ => Err(TlError::UnexpectedEof),
    }
}

fn to_response<T: TlWrite<Repr = tl_proto::Boxed>>(data: T) -> Bytes {
    tl_proto::serialize(data).into()
}

#[derive(thiserror::Error, Debug)]
enum LiteServerError {
    #[error("task cancelled")]
    Cancelled,
    #[error("server is not ready yet")]
    NotReady,
    #[error("invalid query: {0}")]
    InvalidQuery(#[from] tl_proto::TlError),
    #[error("internal error: {0}")]
    Internal(anyhow::Error),
}

impl From<StateError> for LiteServerError {
    fn from(value: StateError) -> Self {
        match value {
            StateError::NotReady => Self::NotReady,
            StateError::Internal(error) => Self::Internal(error),
            StateError::Cancelled => Self::Cancelled,
        }
    }
}

impl LiteServerError {
    // https://github.com/ton-blockchain/ton/blob/4ebd7412c52248360464c2df5f434c8aaa3edfe1/common/errorcode.h#L23-L31
    fn code(&self) -> i32 {
        match self {
            Self::Cancelled => 653,
            Self::NotReady => 651,
            Self::InvalidQuery(_) => 621,
            Self::Internal(_) => 602,
        }
    }
}

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
    #[error("timeout")]
    Timeout,
    #[error("invalid data: {0}")]
    InvalidData(#[from] tl_proto::TlError),
    #[error("operation cancelled")]
    Cancelled,
}
