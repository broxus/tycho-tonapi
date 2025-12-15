use anyhow::{Context, Result};
use clap::Parser;
use serde::{Deserialize, Serialize};
use tycho_core::block_strider::{
    BlockProviderExt, BlockSubscriberExt, MetricsSubscriber, ShardStateApplier,
};
use tycho_core::node::{LightNodeConfig, LightNodeContext, NodeBaseConfig};
use tycho_tonapi::grpc::GrpcConfig;
use tycho_tonapi::state::{AppState, AppStateConfig};
use tycho_util::cli;
use tycho_util::cli::config::ThreadPoolConfig;
use tycho_util::cli::logger::LoggerConfig;
use tycho_util::cli::metrics::MetricsConfig;
use tycho_util::config::PartialConfig;
use tycho_util::futures::JoinTask;

#[derive(Parser)]
pub struct Cmd {
    #[clap(flatten)]
    args: tycho_core::node::CmdRunArgs,
}

impl Cmd {
    pub fn run(self) -> Result<()> {
        self.args.init_config_or_run_light_node(async move |ctx| {
            let LightNodeContext {
                node,
                boot_args,
                global_config,
                config: NormalizedNodeConfig(config),
                ..
            } = ctx;

            let state = AppState::new(
                node.blockchain_rpc_client.clone(),
                node.core_storage.clone(),
                global_config.zerostate,
                config.app,
                config.base.core_storage.blocks_gc,
            )?;

            // Bind gRPC
            let _grpc_task = {
                let server = tycho_tonapi::grpc::serve(state.clone(), config.grpc.clone());

                JoinTask::new(async move {
                    if let Err(e) = server.await {
                        // TODO: Stop the node on server error
                        tracing::error!("gRPC server stopped with error: {e:?}");
                    }
                })
            };

            // Sync node.
            let init_block_id = node.init_ext(boot_args).await?;
            node.update_validator_set_from_shard_state(&init_block_id)
                .await?;

            // Finish app state initialization.
            state
                .init(&init_block_id)
                .await
                .context("failed to init app state")?;
            let backpressure = state.backpressure();

            // Build strider.
            let archive_block_provider = node.build_archive_block_provider();
            let storage_block_provider = node.build_storage_block_provider();
            let blockchain_block_provider = node
                .build_blockchain_block_provider()
                .with_fallback(archive_block_provider.clone());

            let block_strider = node.build_strider(
                archive_block_provider.chain((blockchain_block_provider, storage_block_provider)),
                (
                    state.clone(),
                    ShardStateApplier::new(node.core_storage.clone(), state),
                    node.validator_resolver().clone(),
                    MetricsSubscriber,
                )
                    .chain(backpressure),
            );

            // Run block strider
            tracing::info!("block strider started");
            block_strider.run().await?;
            tracing::info!("block strider finished");

            Ok(())
        })
    }
}

#[derive(PartialConfig, Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
struct NodeConfig {
    #[partial]
    #[serde(flatten)]
    base: NodeBaseConfig,
    #[important]
    threads: ThreadPoolConfig,
    #[important]
    logger_config: LoggerConfig,
    #[important]
    metrics: Option<MetricsConfig>,
    #[important]
    app: AppStateConfig,
    #[important]
    grpc: GrpcConfig,
}

impl Default for NodeConfig {
    fn default() -> Self {
        Self {
            base: NodeBaseConfig::default(),
            threads: ThreadPoolConfig::default(),
            logger_config: LoggerConfig::default(),
            metrics: Some(MetricsConfig::default()),
            app: AppStateConfig::default(),
            grpc: GrpcConfig::default(),
        }
    }
}

#[derive(Default, Debug, Clone, PartialConfig, Serialize)]
#[serde(transparent)]
struct NormalizedNodeConfig(#[partial] NodeConfig);

impl<'de> Deserialize<'de> for NormalizedNodeConfig {
    fn deserialize<D: serde::Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        use serde::de::Error;

        let mut config = NodeConfig::deserialize(de)?;

        config
            .app
            .modify_base_config(&mut config.base)
            .map_err(Error::custom)?;

        Ok(Self(config))
    }
}

impl LightNodeConfig for NormalizedNodeConfig {
    fn base(&self) -> &NodeBaseConfig {
        &self.0.base
    }

    fn threads(&self) -> &cli::config::ThreadPoolConfig {
        &self.0.threads
    }

    fn metrics(&self) -> Option<&cli::metrics::MetricsConfig> {
        self.0.metrics.as_ref()
    }

    fn logger(&self) -> Option<&cli::logger::LoggerConfig> {
        Some(&self.0.logger_config)
    }
}
