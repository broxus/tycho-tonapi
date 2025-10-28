use std::net::{Ipv4Addr, SocketAddr};
use std::pin::pin;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use bytes::Bytes;
use futures_util::future::Either;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tycho_crypto::ed25519;
use tycho_types::prelude::*;
use tycho_util::serde_helpers;

use super::connection::{AdnlConnection, ResolveReceiver};
use super::{AdnlError, proto};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TcpAdnlServerConfig {
    pub listen_addr: SocketAddr,
    #[serde(with = "serde_helpers::humantime")]
    pub connection_timeout: Duration,
    #[serde(with = "serde_helpers::humantime")]
    pub query_timeout: Duration,
}

impl Default for TcpAdnlServerConfig {
    fn default() -> Self {
        Self {
            listen_addr: (Ipv4Addr::UNSPECIFIED, 20000).into(),
            connection_timeout: Duration::from_secs(1),
            query_timeout: Duration::from_secs(60),
        }
    }
}

pub struct QueryContext<'a> {
    pub peer_addr: &'a SocketAddr,
    pub query_id: &'a HashBytes,
}

pub trait QueryHandler: Send + Sync + 'static {
    fn handle_query(
        &self,
        ctx: QueryContext<'_>,
        query: Bytes,
    ) -> impl Future<Output = Option<Bytes>> + Send;
}

pub struct TcpAdnlServer<T> {
    listener: TcpListener,
    shared: Arc<Shared<T>>,
}

impl<T: QueryHandler> TcpAdnlServer<T> {
    pub async fn bind(
        keypair: Arc<ed25519::KeyPair>,
        config: TcpAdnlServerConfig,
        query_handler: T,
    ) -> Result<Self> {
        let listener = TcpListener::bind(config.listen_addr)
            .await
            .context("failed to bind TCP ADNL listener")?;

        let local_id = tl_proto::hash(keypair.public_key.as_tl());
        tracing::info!(public_key = %keypair.public_key, listen_addr = %config.listen_addr);

        Ok(Self {
            listener,
            shared: Arc::new(Shared {
                local_id,
                keypair,
                connection_timeout: config.connection_timeout,
                query_handler,
            }),
        })
    }

    pub async fn serve(self) -> Result<()> {
        let cancellation = CancellationToken::new();
        loop {
            let (socket, addr) = self.listener.accept().await?;

            let shared = self.shared.clone();

            // TODO: Use some common abort handle
            tokio::task::spawn(cancellation.clone().run_until_cancelled_owned(async move {
                if let Err(e) = shared.handle_connection(socket, addr).await {
                    tracing::debug!(%addr, "connection handler failed: {e:?}");
                }
            }));
        }
    }
}

struct Shared<T> {
    local_id: [u8; 32],
    keypair: Arc<ed25519::KeyPair>,
    connection_timeout: Duration,
    query_handler: T,
}

impl<T: QueryHandler> Shared<T> {
    #[tracing::instrument(skip_all, fields(addr = %addr))]
    async fn handle_connection(
        self: Arc<Self>,
        socket: TcpStream,
        addr: SocketAddr,
    ) -> Result<(), AdnlError> {
        enum Answer {
            Raw(Bytes),
            ToQuery(proto::AdnlAnswer),
        }

        const ANSWERS_BUFFER: usize = 2;

        tracing::debug!("new connection");
        scopeguard::defer! {
            tracing::debug!("connection dropped");
        };

        let (rx, tx) = socket.into_split();
        let fut = AdnlConnection::accept(rx, tx, &*self);
        let mut connection = match tokio::time::timeout(self.connection_timeout, fut).await {
            Ok(res) => res?,
            Err(_) => {
                tracing::debug!("connection timeout");
                return Err(AdnlError::Timeout);
            }
        };
        tracing::debug!(peer_id = %connection.peer_id, "connection established");

        let (answers_tx, mut answers_rx) = mpsc::channel::<Answer>(ANSWERS_BUFFER);
        let answers_task = tokio::task::spawn(async move {
            while let Some(item) = answers_rx.recv().await {
                match item {
                    Answer::Raw(bytes) => connection.tx.send(bytes).await,
                    Answer::ToQuery(answer) => connection.tx.send(&answer).await,
                }?;
            }
            Ok::<_, AdnlError>(())
        });

        let cancellation = CancellationToken::new();
        scopeguard::defer! {
            cancellation.cancel();
        }

        let queries_task = async {
            while let Some(packet) = connection.rx.next().await {
                let packet = packet?;

                if packet.len() == 12 {
                    let proto::TcpPing { random_id } = tl_proto::deserialize(&packet)?;

                    let answers_tx = answers_tx.clone();
                    tokio::spawn(cancellation.clone().run_until_cancelled_owned(async move {
                        let answer = tl_proto::serialize(proto::TcpPong { random_id });
                        answers_tx.send(Answer::Raw(answer.into())).await.ok();
                    }));
                } else {
                    let proto::AdnlQuery { query_id, query } = proto::deserialize_exact(&packet)?;
                    let query = packet.slice_ref(query);
                    let this = self.clone();
                    let answers_tx = answers_tx.clone();
                    tokio::spawn(cancellation.clone().run_until_cancelled_owned(async move {
                        if let Some(answer) = this
                            .query_handler
                            .handle_query(
                                QueryContext {
                                    peer_addr: &addr,
                                    query_id: &query_id,
                                },
                                query,
                            )
                            .await
                        {
                            answers_tx
                                .send(Answer::ToQuery(proto::AdnlAnswer { query_id, answer }))
                                .await
                                .ok();
                        }
                    }));
                }
            }
            Ok::<_, AdnlError>(())
        };

        match futures_util::future::select(pin!(queries_task), answers_task).await {
            Either::Left((res, other)) => {
                other.abort();
                res
            }
            Either::Right((res, _)) => res.map_err(|_| AdnlError::Cancelled)?,
        }
    }
}

impl<T> ResolveReceiver for Shared<T> {
    fn resolve_receiver<'s, 'i>(&'s self, id: &'i [u8; 32]) -> Option<&'s ed25519::KeyPair> {
        if id == &self.local_id {
            Some(&self.keypair)
        } else {
            None
        }
    }
}
