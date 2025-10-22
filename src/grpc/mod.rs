use std::pin::Pin;
use std::task::{Context, Poll};

use tokio_stream::Stream;
use tycho_types::cell::HashBytes;
use tycho_types::models::{BlockIdShort, ShardIdent};

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
        let query = request.get_ref().query.require()?.try_into()?;
        let res = self.state.get_block(&query).await?;

        Ok(tonic::Response::new(match res.data {
            Some(stream) => GetBlockStream::Found {
                mc_state_info: res.mc_state_info,
                offset: 0,
                first: true,
                stream: Some(stream),
            },
            None => GetBlockStream::NotFound {
                mc_state_info: res.mc_state_info,
                finished: false,
            },
        }))
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

pub enum GetBlockStream {
    NotFound {
        mc_state_info: crate::state::McStateInfo,
        finished: bool,
    },
    Found {
        mc_state_info: crate::state::McStateInfo,
        offset: u64,
        first: bool,
        stream: Option<Box<crate::state::BlockDataStream>>,
    },
}

impl Stream for GetBlockStream {
    type Item = tonic::Result<proto::GetBlockResponse>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        use proto::get_block_response::Msg;

        match &mut *self {
            // Send "NotFound" once.
            GetBlockStream::NotFound {
                mc_state_info,
                finished,
            } => Poll::Ready(if *finished {
                None
            } else {
                *finished = true;
                Some(Ok(proto::GetBlockResponse {
                    msg: Some(Msg::NotFound(proto::BlockNotFound {
                        mc_state_info: Some((*mc_state_info).into()),
                    })),
                }))
            }),
            // Send "Found" once and all remaining "Chunk"'s afterwards.
            GetBlockStream::Found {
                mc_state_info,
                offset,
                first,
                stream,
            } => {
                let field = stream;
                let Some(stream) = field.as_mut() else {
                    return Poll::Ready(None);
                };

                match Pin::new(stream.as_mut()).poll_next(cx) {
                    Poll::Ready(Some(Ok(data))) => {
                        let next_offset = offset.saturating_add(data.len() as u64);
                        let chunk = proto::BlockChunk {
                            offset: *offset,
                            data,
                        };
                        *offset = next_offset;

                        let total_size = stream.total_size();
                        let max_chunk_size = stream.chunk_size();
                        if next_offset >= total_size {
                            *field = None;
                        }

                        let msg = if std::mem::take(first) {
                            Msg::Found(proto::BlockFound {
                                mc_state_info: Some((*mc_state_info).into()),
                                total_size,
                                max_chunk_size,
                                first_chunk: Some(chunk),
                            })
                        } else {
                            Msg::Chunk(chunk)
                        };

                        Poll::Ready(Some(Ok(proto::GetBlockResponse { msg: Some(msg) })))
                    }
                    Poll::Ready(Some(Err(e))) => {
                        *field = None;
                        Poll::Ready(Some(Err(tonic::Status::internal(format!(
                            "data stream failed: {e}"
                        )))))
                    }
                    Poll::Ready(None) => {
                        *field = None;
                        Poll::Ready(if *first {
                            Some(Err(tonic::Status::internal(
                                "unexpected end of data stream",
                            )))
                        } else {
                            None
                        })
                    }
                    Poll::Pending => Poll::Pending,
                }
            }
        }
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

fn parse_shard_id(workchain: i32, shard: u64) -> tonic::Result<ShardIdent> {
    ShardIdent::new(workchain, shard)
        .ok_or_else(|| tonic::Status::invalid_argument("invalid shard id"))
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

impl TryFrom<&proto::get_block_request::Query> for crate::state::QueryBlock {
    type Error = tonic::Status;

    fn try_from(value: &proto::get_block_request::Query) -> Result<Self, Self::Error> {
        match value {
            proto::get_block_request::Query::BySeqno(q) => Ok(Self::BySeqno(BlockIdShort {
                shard: parse_shard_id(q.workchain, q.shard)?,
                seqno: q.seqno,
            })),
            proto::get_block_request::Query::ById(q) => q.try_into().map(Self::ById),
        }
    }
}

impl TryFrom<&proto::BlockId> for tycho_types::models::BlockId {
    type Error = tonic::Status;

    fn try_from(value: &proto::BlockId) -> Result<Self, Self::Error> {
        let shard = parse_shard_id(value.workchain, value.shard)?;
        Ok(Self {
            shard,
            seqno: value.seqno,
            root_hash: *parse_hash_ref(&value.root_hash)?,
            file_hash: *parse_hash_ref(&value.file_hash)?,
        })
    }
}
