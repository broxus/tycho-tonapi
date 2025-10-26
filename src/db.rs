use tycho_storage::kv::{
    Migrations, NamedTables, StateVersionProvider, TableContext, WithMigrations,
};
use tycho_util::sync::CancellationFlag;
use weedb::{MigrationError, WeeDb};

pub type TonApiDb = WeeDb<TonApiTables>;

weedb::tables! {
    pub struct TonApiTables<TableContext> {
        pub state: tables::State,
        pub blocks_by_mc_seqno: tables::BlocksByMcSeqno,
        pub known_blocks: tables::KnownBlocks,
    }
}

impl NamedTables for TonApiTables {
    const NAME: &'static str = "tonapi";
}

impl WithMigrations for TonApiTables {
    const VERSION: weedb::Semver = [0, 0, 1];

    type VersionProvider = StateVersionProvider<tables::State>;

    fn new_version_provider() -> Self::VersionProvider {
        StateVersionProvider::new::<Self>()
    }

    fn register_migrations(
        _migrations: &mut Migrations<Self::VersionProvider, Self>,
        _cancelled: CancellationFlag,
    ) -> Result<(), MigrationError> {
        // migrations.register([0, 0, 1], [0, 0, 2], move |db| Ok(()))?;

        Ok(())
    }
}

pub mod tables {
    pub use tycho_core::storage::tables::State;
    use tycho_storage::kv::{TableContext, zstd_block_based_table_factory};
    use weedb::rocksdb::Options;
    use weedb::{ColumnFamily, ColumnFamilyOptions};

    /// Processed blocks meta.
    /// - Key: `workchain: i8, shard: u64 (BE), seqno: u32 (BE)`
    /// - Value: `root_hash: [u8; 32], file_hash: [u8; 32], mc_seqno: u32 (LE), raw_data_size: u64,
    ///   state_root_proof_len: u32 (LE), state_root_proof: bytes`
    pub struct KnownBlocks;

    impl KnownBlocks {
        pub const KEY_LEN: usize = 1 + 8 + 4;

        pub const STATE_PROOF_OFFSET: usize = 32 + 32 + 4 + 8;
    }

    impl ColumnFamily for KnownBlocks {
        const NAME: &'static str = "known_blocks";
    }

    impl ColumnFamilyOptions<TableContext> for KnownBlocks {
        fn options(opts: &mut Options, ctx: &mut TableContext) {
            zstd_block_based_table_factory(opts, ctx);
        }
    }

    /// Blocks by logical time.
    ///
    /// - Key: `mc_seqno: u32 (BE), workchain: i8, shard: u64 (BE), seqno: u32 (BE)`
    /// - Value: `root_hash: [u8; 32], file_hash: [u8; 32], start_lt: u64 (LE), end_lt: u64 (LE)`
    ///
    /// # Additional value for masterchain
    /// - `shard_count: u32 (LE), shard_count * (workchain: i8, shard: u64 (LE), seqno: u32 (LE)),
    ///   root_hash: [u8; 32], file_hash: [u8; 32], start_lt: u64 (LE), end_lt: u64 (LE), reg_mc_seqno: u32 (LE))`
    pub struct BlocksByMcSeqno;

    impl BlocksByMcSeqno {
        pub const KEY_LEN: usize = 4 + 1 + 8 + 4;

        pub const SHARD_HASHES_OFFSET: usize = 32 + 32 + 8 + 8;
        pub const SHARD_HASHES_ITEM_LEN: usize = 1 + 8 + 4 + 32 + 32 + 8 + 8 + 4;
    }

    impl ColumnFamily for BlocksByMcSeqno {
        const NAME: &'static str = "blocks_by_mc_seqno";
    }

    impl ColumnFamilyOptions<TableContext> for BlocksByMcSeqno {
        fn options(opts: &mut Options, ctx: &mut TableContext) {
            zstd_block_based_table_factory(opts, ctx);
        }
    }
}
