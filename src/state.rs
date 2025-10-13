use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use anyhow::Result;
use arc_swap::ArcSwapOption;
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use tycho_block_util::state::ShardStateStuff;
use tycho_core::storage::CoreStorage;
use tycho_types::models::{DepthBalanceInfo, LibDescr, ShardAccount};
use tycho_types::prelude::*;
use tycho_util::FastHasherState;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppStateConfig {
    pub libs_cache_capacity: u64,
}

impl Default for AppStateConfig {
    fn default() -> Self {
        Self {
            libs_cache_capacity: 100,
        }
    }
}

#[derive(Clone)]
#[repr(transparent)]
pub struct AppState {
    inner: Arc<Inner>,
}

impl AppState {
    pub fn new(storage: CoreStorage, config: AppStateConfig) -> Self {
        Self {
            inner: Arc::new(Inner {
                is_ready: AtomicBool::new(false),
                storage,
                libs_cache: moka::sync::Cache::builder()
                    .max_capacity(config.libs_cache_capacity)
                    .build_with_hasher(Default::default()),
                latest_mc_state: Default::default(),
            }),
        }
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

struct Inner {
    is_ready: AtomicBool,
    storage: CoreStorage,
    libs_cache: moka::sync::Cache<HashBytes, Bytes, FastHasherState>,
    latest_mc_state: ArcSwapOption<CachedState>,
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
