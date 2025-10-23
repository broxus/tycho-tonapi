use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::task::{Context, Poll};

use anyhow::{Context as _, Result};
use arc_swap::ArcSwapOption;
use bytes::Bytes;
use bytesize::ByteSize;
use futures_util::future::BoxFuture;
use serde::{Deserialize, Serialize};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tokio_stream::Stream;
use tycho_block_util::state::ShardStateStuff;
use tycho_core::block_strider::{BlockSubscriber, BlockSubscriberContext, StateSubscriber};
use tycho_core::storage::CoreStorage;
use tycho_storage::kv::NamedTables;
use tycho_types::models::{BlockId, BlockIdShort, DepthBalanceInfo, LibDescr, ShardAccount};
use tycho_types::prelude::*;
use tycho_util::FastHasherState;
use weedb::rocksdb;

use crate::db::{TonApiDb, TonApiTables, tables};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppStateConfig {
    pub libs_cache_capacity: u64,
    pub block_data_chunk_size: ByteSize,
    pub max_concurrent_downloads: usize,
}

impl Default for AppStateConfig {
    fn default() -> Self {
        Self {
            libs_cache_capacity: 100,
            block_data_chunk_size: ByteSize::mib(1),
            max_concurrent_downloads: 1000,
        }
    }
}

#[derive(Clone)]
#[repr(transparent)]
pub struct AppState {
    inner: Arc<Inner>,
}

impl AppState {
    pub fn new(storage: CoreStorage, config: AppStateConfig) -> Result<Self> {
        let db = storage.context().open_preconfigured(TonApiTables::NAME)?;

        Ok(Self {
            inner: Arc::new(Inner {
                is_ready: AtomicBool::new(false),
                storage,
                libs_cache: moka::sync::Cache::builder()
                    .max_capacity(config.libs_cache_capacity)
                    .build_with_hasher(Default::default()),
                latest_mc_state: Default::default(),
                block_data_chunk_size: config.block_data_chunk_size.as_u64(),
                download_block_semaphore: Arc::new(Semaphore::new(config.max_concurrent_downloads)),
                db,
            }),
        })
    }

    pub async fn init(&self, _init_block_id: &BlockId) -> Result<()> {
        self.inner.is_ready.store(true, Ordering::Release);
        tracing::info!("app state is ready");
        Ok(())
    }

    pub fn is_ready(&self) -> bool {
        self.inner.is_ready.load(Ordering::Acquire)
    }

    pub async fn get_block(&self, query: &QueryBlock) -> StateResult<Option<Box<BlockDataStream>>> {
        let this = self.inner.as_ref();
        let mc_state_info = if self.is_ready()
            && let Some(mc_state_info) = this
                .latest_mc_state
                .load()
                .as_ref()
                .map(|x| x.mc_state_info)
        {
            mc_state_info
        } else {
            return Err(StateError::NotReady);
        };

        let handles = this.storage.block_handle_storage();
        let blocks = this.storage.block_storage();

        let handle = match query {
            QueryBlock::BySeqno(_) => {
                // TODO: Support
                return Err(StateError::Internal(anyhow::anyhow!("not supported")));
            }
            QueryBlock::ById(block_id) => handles.load_handle(block_id),
        };

        let stream = 'stream: {
            let handle = match handle {
                Some(handle) if handle.has_data() => handle,
                // Early exit of no data found.
                _ => break 'stream None,
            };

            let permit = this
                .download_block_semaphore
                .clone()
                .acquire_owned()
                .await
                .map_err(|_| StateError::Internal(anyhow::anyhow!("blocks semaphore dropped")))?;

            // Check once more if block was removed during waiting for a permit.
            if !handle.has_data() {
                break 'stream None;
            }

            // TODO: Stream block data from disk.
            let data = blocks
                .load_block_data_decompressed(&handle)
                .await
                .map_err(StateError::Internal)?;

            Some(Box::new(BlockDataStream {
                total_size: data.len() as u64,
                chunk_size: this.block_data_chunk_size,
                block_id: *handle.id(),
                data,
                offset: 0,
                _permit: permit,
            }))
        };

        Ok(WithMcStateInfo::new(mc_state_info, stream))
    }

    pub fn get_account_state(&self) {}

    pub fn get_library_cell(&self, hash: &HashBytes) -> StateResult<Option<Bytes>> {
        let Some(state) = self.inner.latest_mc_state.load_full() else {
            return Err(StateError::NotReady);
        };

        let mut lib = self.inner.libs_cache.get(hash);
        if lib.is_none() {
            lib = state
                .libraries
                .get(hash)?
                .map(|x| Bytes::from(Boc::encode(x.lib)));

            if let Some(lib) = &lib {
                self.inner.libs_cache.insert(*hash, lib.clone());
            }
        }

        Ok(WithMcStateInfo::new(state.mc_state_info, lib))
    }
}

impl BlockSubscriber for AppState {
    type Prepared = tokio::task::JoinHandle<Result<()>>;
    type PrepareBlockFut<'a> = futures_util::future::Ready<Result<Self::Prepared>>;
    type HandleBlockFut<'a> = BoxFuture<'a, Result<()>>;

    fn prepare_block<'a>(&'a self, cx: &'a BlockSubscriberContext) -> Self::PrepareBlockFut<'a> {
        struct LatestBlockInfo {
            id: BlockId,
            start_lt: u64,
            end_lt: u64,
        }

        let block_id = *cx.block.id();
        let mc_seqno = cx.mc_block_id.seqno;

        // TODO: Fill from either archive data or load from disk. However, it may not be
        //       even needed, because we can read it from the compressed data.
        let raw_data_size = 0u64;

        let block = cx.block.clone();
        let db = self.inner.db.clone();
        let handle = tokio::task::spawn_blocking(move || {
            // Skip unusual workchains
            let Ok::<i8, _>(workchain) = block_id.shard.workchain().try_into() else {
                return Ok(());
            };

            let info = block.load_info()?;

            let shard_hashes = block_id
                .is_masterchain()
                .then(|| {
                    let custom = block.load_custom()?;
                    let mut shards = Vec::new();
                    for entry in custom.shards.iter() {
                        let (shard, descr) = entry?;
                        if i8::try_from(shard.workchain()).is_err() {
                            continue;
                        }

                        shards.push(LatestBlockInfo {
                            id: BlockId {
                                shard,
                                seqno: descr.seqno,
                                root_hash: descr.root_hash,
                                file_hash: descr.file_hash,
                            },
                            start_lt: descr.start_lt,
                            end_lt: descr.end_lt,
                        })
                    }
                    Ok::<_, anyhow::Error>(shards)
                })
                .transpose()?;

            let mut batch = rocksdb::WriteBatch::default();

            // Prepare common key
            let mut key = [0; tables::BlocksByMcSeqno::KEY_LEN];
            key[0..4].copy_from_slice(&mc_seqno.to_be_bytes());
            key[4] = workchain as u8;
            key[5..13].copy_from_slice(&block_id.shard.prefix().to_be_bytes());
            key[13..17].copy_from_slice(&block_id.seqno.to_be_bytes());

            // Reserve value buffer
            let mut value = Vec::<u8>::with_capacity(256);

            // Fill known_blocks entry
            value.extend_from_slice(block_id.root_hash.as_array());
            value.extend_from_slice(block_id.file_hash.as_array());
            value.extend_from_slice(&mc_seqno.to_le_bytes());
            value.extend_from_slice(&raw_data_size.to_le_bytes());
            // TODO: Build state proof

            batch.put_cf(&db.known_blocks.cf(), &key[4..], &value);

            // Fill blocks_by_mc_seqno entry
            value.clear();
            value.extend_from_slice(block_id.root_hash.as_array());
            value.extend_from_slice(block_id.file_hash.as_array());
            value.extend_from_slice(&info.start_lt.to_le_bytes());
            value.extend_from_slice(&info.end_lt.to_le_bytes());
            if let Some(shard_hashes) = &shard_hashes {
                value.extend_from_slice(&(shard_hashes.len() as u32).to_le_bytes());
                for item in shard_hashes {
                    value.push(item.id.shard.workchain() as i8 as u8);
                    value.extend_from_slice(&item.id.shard.prefix().to_le_bytes());
                    value.extend_from_slice(&item.id.seqno.to_le_bytes());
                    value.extend_from_slice(item.id.root_hash.as_array());
                    value.extend_from_slice(item.id.file_hash.as_array());
                    value.extend_from_slice(&item.start_lt.to_le_bytes());
                    value.extend_from_slice(&item.end_lt.to_le_bytes());
                }
            }

            batch.put_cf(&db.blocks_by_mc_seqno.cf(), key.as_slice(), value);

            // Write batch
            db.rocksdb()
                .write_opt(batch, db.known_blocks.write_config())?;

            // Done
            Ok::<_, anyhow::Error>(())
        });

        futures_util::future::ready(Ok(handle))
    }

    fn handle_block<'a>(
        &'a self,
        _cx: &'a BlockSubscriberContext,
        prepared: Self::Prepared,
    ) -> Self::HandleBlockFut<'a> {
        Box::pin(async move {
            prepared.await?.context("failed to store block metadata")?;

            Ok(())
        })
    }
}

impl StateSubscriber for AppState {
    type HandleStateFut<'a> = BoxFuture<'a, Result<()>>;

    fn handle_state<'a>(
        &'a self,
        cx: &'a tycho_core::block_strider::StateSubscriberContext,
    ) -> Self::HandleStateFut<'a> {
        Box::pin(async move {
            if !cx.block.id().is_masterchain() {
                // TODO: Handle shard blocks as well
                return Ok(());
            }

            let state = cx.state.as_ref();

            let libraries = state.libraries.clone();
            let (accounts, _) = state.load_accounts()?.into_parts();
            let mc_state_info = McStateInfo {
                mc_seqno: state.seqno,
                lt: state.gen_lt,
                utime: state.gen_utime,
            };

            self.inner.latest_mc_state.store(Some(Arc::new(CachedState {
                state: cx.state.clone(),
                libraries,
                accounts,
                mc_state_info,
            })));

            Ok(())
        })
    }
}

struct Inner {
    is_ready: AtomicBool,
    storage: CoreStorage,
    libs_cache: moka::sync::Cache<HashBytes, Bytes, FastHasherState>,
    latest_mc_state: ArcSwapOption<CachedState>,
    block_data_chunk_size: u64,
    download_block_semaphore: Arc<Semaphore>,
    db: TonApiDb,
}

#[derive(Clone)]
struct CachedState {
    state: ShardStateStuff,
    libraries: Dict<HashBytes, LibDescr>,
    accounts: ShardAccountsDict,
    mc_state_info: McStateInfo,
}

type ShardAccountsDict = Dict<HashBytes, (DepthBalanceInfo, ShardAccount)>;

pub struct WithMcStateInfo<T> {
    pub mc_state_info: McStateInfo,
    pub data: T,
}

impl<T> WithMcStateInfo<T> {
    #[inline]
    fn new(mc_state_info: McStateInfo, data: T) -> Self {
        Self {
            mc_state_info,
            data,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct McStateInfo {
    pub mc_seqno: u32,
    pub lt: u64,
    pub utime: u32,
}

#[derive(Debug, Clone, Copy)]
pub enum AtBlock {
    Latest,
    BySeqno(u32),
    ByRootHash(HashBytes),
}

#[derive(Debug, Clone, Copy)]
pub enum QueryBlock {
    BySeqno(BlockIdShort),
    ById(BlockId),
}

pub struct BlockDataStream {
    total_size: u64,
    chunk_size: u64,
    block_id: BlockId,
    // TODO: Stream data from disk
    data: Bytes,
    offset: u64,
    _permit: OwnedSemaphorePermit,
}

impl BlockDataStream {
    #[inline]
    pub fn total_size(&self) -> u64 {
        self.total_size
    }

    #[inline]
    pub fn chunk_size(&self) -> u64 {
        self.chunk_size
    }

    #[inline]
    pub fn block_id(&self) -> &BlockId {
        &self.block_id
    }
}

impl Stream for BlockDataStream {
    type Item = std::io::Result<Bytes>;

    fn poll_next(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = &mut *self.as_mut();
        let data_len = this.data.len() as u64;
        if this.offset >= data_len {
            return Poll::Ready(None);
        }

        let next_offset = std::cmp::min(this.offset.saturating_add(this.chunk_size), data_len);
        let data = this.data.slice(this.offset as usize..next_offset as usize);
        this.offset = next_offset;
        Poll::Ready(Some(Ok(data)))
    }
}

pub type StateResult<T> = Result<WithMcStateInfo<T>, StateError>;

#[derive(Debug, thiserror::Error)]
pub enum StateError {
    #[error("state is not ready")]
    NotReady,
    #[error("internal error: {0}")]
    Internal(#[source] anyhow::Error),
}

impl From<tycho_types::error::Error> for StateError {
    #[inline]
    fn from(value: tycho_types::error::Error) -> Self {
        Self::Internal(value.into())
    }
}
