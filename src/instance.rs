use crate::{
    helpers::{create_general_purpose_runtime, create_search_runtime, create_update_runtime},
    AliasRequest, AliasResponse, CollectionRequest, CollectionResponse, Handler, PointsRequest,
    PointsResponse, RroClient, RROError, RROMsg, QueryRequest, QueryResponse, Settings,
};
use async_trait::async_trait;
use collection::shards::channel_service::ChannelService;
use common::budget::ResourceBudget;
use common::cpu::get_num_cpus;
use serde::{Deserialize, Serialize};
use std::{mem::ManuallyDrop, sync::Arc, thread, time::Duration};
use storage::content_manager::{
    consensus::persistent::Persistent, errors::StorageError, toc::TableOfContent,
};
use tokio::{
    runtime::Handle,
    sync::{mpsc, oneshot},
};
use tracing::{debug, warn};

const RRO_CHANNEL_BUFFER: usize = 1024;

#[derive(Debug, Deserialize)]
pub enum RRORequest {
    Collection(CollectionRequest),
    Alias(AliasRequest),
    Points(PointsRequest),
    Query(QueryRequest),
}

#[derive(Debug, Serialize)]
pub enum RROResponse {
    Collection(CollectionResponse),
    Alias(AliasResponse),
    Points(PointsResponse),
    Query(QueryResponse),
}

pub struct RROInstance;

impl RROInstance {
    pub fn start(config_path: Option<String>) -> Result<Arc<RroClient>, RROError> {
        let (tx, mut rx) = mpsc::channel::<RROMsg>(RRO_CHANNEL_BUFFER);

        let (terminated_tx, terminated_rx) = oneshot::channel::<()>();

        let handle = thread::Builder::new()
            .name("rro".to_string())
            .spawn(move || {
                let (toc, rt) = start_rro(config_path)?;
                let toc_clone = toc.clone();
                rt.block_on(async move {
                    while let Some((msg, resp_sender)) = rx.recv().await {
                        let toc_clone = toc.clone();
                        tokio::spawn(async move {
                            let res = msg.handle(&toc_clone).await;
                            if let Err(e) = resp_sender.send(res) {
                                warn!("Failed to send response: {:?}", e);
                            }
                        });
                    }
                    Ok::<(), RROError>(())
                })?;

                // clean things up
                // see this thread: https://github.com/eonsofstupid/rrorro/issues/1316
                let mut toc_arc = toc_clone;
                loop {
                    match Arc::try_unwrap(toc_arc) {
                        Ok(toc) => {
                            drop(toc);
                            if let Err(e) = terminated_tx.send(()) {
                                warn!("Failed to send termination signal: {:?}", e);
                            }
                            break;
                        }
                        Err(toc) => {
                            toc_arc = toc;
                            warn!("Waiting for ToC to be gracefully dropped");
                            thread::sleep(Duration::from_millis(300));
                        }
                    }
                }
                Ok::<(), RROError>(())
            })
            .unwrap();
        Ok(Arc::new(RroClient {
            tx: ManuallyDrop::new(tx),
            handle,
            terminated_rx,
        }))
    }
}

#[async_trait]
impl Handler for RRORequest {
    type Response = RROResponse;
    type Error = StorageError;

    async fn handle(self, toc: &TableOfContent) -> Result<Self::Response, Self::Error> {
        match self {
            RRORequest::Collection(req) => {
                let resp = req.handle(toc).await?;
                Ok(RROResponse::Collection(resp))
            }
            RRORequest::Alias(req) => {
                let resp = req.handle(toc).await?;
                Ok(RROResponse::Alias(resp))
            }
            RRORequest::Points(req) => {
                let resp = req.handle(toc).await?;
                Ok(RROResponse::Points(resp))
            }
            RRORequest::Query(req) => {
                let resp = req.handle(toc).await?;
                Ok(RROResponse::Query(resp))
            }
        }
    }
}

/// Start RRO and get TableOfContent.
fn start_rro(config_path: Option<String>) -> Result<(Arc<TableOfContent>, Handle), RROError> {
    let settings = Settings::new(config_path).expect("Failed to load settings");

    memory::madvise::set_global(settings.storage.mmap_advice);
    segment::vector_storage::common::set_async_scorer(
        settings.storage.performance.async_scorer.unwrap_or(false),
    );

    if let Some(recovery_warning) = &settings.storage.recovery_mode {
        warn!("RRO is loaded in recovery mode: {}", recovery_warning);
        warn!("Read more: https://devpulse.app/documentation/guides/administration/#recovery-mode");
    }

    // Saved state of the consensus. This is useless for single node mode.
    // Args: path, first_peer, allow_recovery, recovery_snapshot_id
    let persistent_consensus_state =
        Persistent::load_or_init(&settings.storage.storage_path, true, false, None)?;

    // Create and own search runtime out of the scope of async context to ensure correct
    // destruction of it
    let search_runtime = create_search_runtime(settings.storage.performance.max_search_threads)
        .expect("Can't create search runtime.");

    let update_runtime =
        create_update_runtime(settings.storage.performance.max_optimization_runtime_threads)
            .expect("Can't create optimizer runtime.");

    let general_runtime =
        create_general_purpose_runtime().expect("Can't create general purpose runtime.");
    let runtime_handle = general_runtime.handle().clone();

    // Channel service is used to manage connections between peers.
    // It allocates required number of channels and manages proper reconnection handling.
    // This is useless for single node mode.
    let channel_service = ChannelService::new(6333, None);

    // Create optimizer resource budget based on available CPUs
    // Args: cpu_budget, io_budget (using same value for both)
    let num_cpus = get_num_cpus();
    let optimizer_resource_budget = ResourceBudget::new(num_cpus, num_cpus);

    // Table of content manages the list of collections.
    // It is a main entry point for the storage.
    let toc = TableOfContent::new(
        &settings.storage,
        search_runtime,
        update_runtime,
        general_runtime,
        optimizer_resource_budget,
        channel_service.clone(),
        persistent_consensus_state.this_peer_id(),
        None, // No consensus in single-node mode
    );

    toc.clear_all_tmp_directories()?;

    // Here we load all stored collections.
    runtime_handle.block_on(async {
        use storage::rbac::Access;
        let access = Access::full("Embedded");
        for collection_pass in toc.all_collections(&access).await {
            debug!("Loaded collection: {}", collection_pass.name());
        }
    });

    Ok((Arc::new(toc), runtime_handle))
}
