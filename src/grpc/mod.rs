use std::pin::Pin;
use std::task::{Context, Poll};

use tycho_types::cell::HashBytes;

use crate::state::{AppState, StateError};

pub mod proto {
    tonic::include_proto!("indexer");
}

// === Service ===

pub struct GrpcServer {
    state: AppState,
}

impl GrpcServer {
    pub fn new(state: AppState) -> Self {
        Self { state }
    }
}

#[tonic::async_trait]
impl proto::tycho_indexer_server::TychoIndexer for GrpcServer {
    type WatchBlockIdsStream = WatchBlockIdsStream;
    type GetBlockStream = GetBlockStream;

    async fn get_status(
        &self,
        _request: tonic::Request<proto::GetStatusRequest>,
    ) -> tonic::Result<tonic::Response<proto::GetStatusResponse>> {
        todo!()
    }

    async fn watch_block_ids(
        &self,
        request: tonic::Request<proto::WatchBlockIdsRequest>,
    ) -> tonic::Result<tonic::Response<Self::WatchBlockIdsStream>> {
        todo!()
    }

    async fn get_block(
        &self,
        request: tonic::Request<proto::GetBlockRequest>,
    ) -> tonic::Result<tonic::Response<Self::GetBlockStream>> {
        todo!()
    }

    async fn get_shard_account(
        &self,
        request: tonic::Request<proto::GetShardAccountRequest>,
    ) -> tonic::Result<tonic::Response<proto::GetShardAccountResponse>> {
        todo!()
    }

    async fn get_library_cell(
        &self,
        request: tonic::Request<proto::GetLibraryCellRequest>,
    ) -> tonic::Result<tonic::Response<proto::GetLibraryCellResponse>> {
        use proto::get_library_cell_response::Library;

        let lib_hash = parse_hash_ref(&request.get_ref().hash)?;
        let res = self.state.get_library_cell(lib_hash)?;
        Ok(tonic::Response::new(proto::GetLibraryCellResponse {
            library: Some(match res.data {
                Some(cell) => Library::Found(proto::LibraryCellFound {
                    cell,
                    mc_state_info: Some(res.mc_state_info.into()),
                }),
                None => Library::NotFound(proto::LibraryCellNotFound {
                    mc_state_info: Some(res.mc_state_info.into()),
                }),
            }),
        }))
    }
}

// === Streams ===

pub struct WatchBlockIdsStream {}

impl tokio_stream::Stream for WatchBlockIdsStream {
    type Item = tonic::Result<proto::WatchBlockIdsEvent>;

    fn poll_next(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        todo!()
    }
}

pub struct GetBlockStream {}

impl tokio_stream::Stream for GetBlockStream {
    type Item = tonic::Result<proto::GetBlockResponse>;

    fn poll_next(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        todo!()
    }
}

// === Protobuf helpers ===

trait RequireField {
    type Field;

    fn require(&self) -> tonic::Result<&Self::Field>;
}

impl<T> RequireField for Option<T> {
    type Field = T;

    #[inline]
    fn require(&self) -> tonic::Result<&Self::Field> {
        match self {
            Some(field) => Ok(field),
            None => Err(tonic::Status::invalid_argument(format!(
                "required field is missing"
            ))),
        }
    }
}

fn parse_hash_ref(value: &[u8]) -> tonic::Result<&HashBytes> {
    match value.try_into() {
        Ok::<&[u8; 32], _>(value) => Ok(HashBytes::wrap(value)),
        Err(_) => Err(tonic::Status::invalid_argument(format!(
            "invalid hash length"
        ))),
    }
}

fn parse_hash(value: Vec<u8>) -> tonic::Result<HashBytes> {
    parse_hash_ref(&value).copied()
}

impl From<StateError> for tonic::Status {
    fn from(value: StateError) -> Self {
        match value {
            StateError::NotReady => tonic::Status::unavailable("service is not ready"),
            StateError::Internal(error) => tonic::Status::internal(error.to_string()),
        }
    }
}

impl From<crate::state::McStateInfo> for proto::McStateInfo {
    #[inline]
    fn from(value: crate::state::McStateInfo) -> Self {
        Self {
            mc_seqno: value.mc_seqno,
            lt: value.lt,
            utime: value.utime,
        }
    }
}
