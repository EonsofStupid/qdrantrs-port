use crate::{
    AliasRequest, AliasResponse, ColName, CollectionRequest, CollectionResponse, LocalRecord,
    PointsRequest, PointsResponse, RroClient, RROError, RROMsg, RRORequest,
    RROResponse, RROResult, QueryRequest, QueryResponse, LocalScoredPoint,
    QueryPointsRequest,
};
use api::rest::schema::{PointStruct, PointVectors, UpdateVectors};
use collection::operations::{
    payload_ops::{DeletePayload, SetPayload},
    point_ops::PointsSelector,
    types::{
        CollectionError, CollectionInfo, CountRequest, CountRequestInternal, PointGroup,
        PointRequest, RecommendGroupsRequest, RecommendRequest, RecommendRequestBatch,
        SearchGroupsRequest, SearchRequest, SearchRequestBatch, UpdateResult, VectorsConfig,
    },
    vector_ops::DeleteVectors,
};
use segment::types::Filter;
use std::{mem::ManuallyDrop, thread, time::{Duration, Instant}};
use storage::content_manager::collection_meta_ops::{CreateCollection, UpdateCollection};
use tokio::sync::{
    mpsc,
    oneshot::{self, error::TryRecvError},
};
use tracing::{info, warn};

/// Maximum time to wait for graceful shutdown
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(30);

impl Drop for RroClient {
    fn drop(&mut self) {
        // Drop the tx channel to signal the rro thread to terminate
        unsafe {
            ManuallyDrop::drop(&mut self.tx);
        }
        
        // Wait for graceful shutdown with timeout
        let start = Instant::now();
        loop {
            match self.terminated_rx.try_recv() {
                Ok(()) => {
                    info!("RRO instance terminated gracefully");
                    break;
                }
                Err(TryRecvError::Empty) => {
                    if start.elapsed() > SHUTDOWN_TIMEOUT {
                        warn!("RRO shutdown timeout after {:?}, forcing termination", SHUTDOWN_TIMEOUT);
                        break;
                    }
                    thread::sleep(Duration::from_millis(50));
                }
                Err(TryRecvError::Closed) => {
                    // Channel closed means thread already exited
                    break;
                }
            }
        }
    }
}

impl RroClient {
    /// Check if the RRO instance is healthy and accepting requests.
    /// 
    /// This is a quick synchronous check that verifies the channel is open.
    /// For a full async health check that verifies the instance responds, use `health_check_async`.
    pub fn is_healthy(&self) -> bool {
        !self.tx.is_closed()
    }

    /// Async health check that verifies the RRO instance is responding.
    /// 
    /// Attempts to list collections with a short timeout to verify the instance
    /// is operational.
    pub async fn health_check(&self) -> Result<(), RROError> {
        // Use a short timeout for health checks
        let timeout = Duration::from_secs(5);
        let (tx, rx) = oneshot::channel::<RROResult>();
        let msg = CollectionRequest::List.into();
        
        self.tx.send((msg, tx)).await.map_err(|_| RROError::ChannelClosed)?;
        
        match tokio::time::timeout(timeout, rx).await {
            Ok(Ok(_)) => Ok(()),
            Ok(Err(_)) => Err(RROError::ChannelClosed),
            Err(_) => Err(RROError::Timeout(timeout)),
        }
    }

    /// Create a new collection.
    pub async fn create_collection(
        &self,
        name: impl Into<String>,
        config: VectorsConfig,
    ) -> Result<bool, RROError> {
        let data = CreateCollection {
            vectors: config,
            shard_number: None,
            sharding_method: None,
            replication_factor: None,
            write_consistency_factor: None,
            on_disk_payload: None,
            hnsw_config: None,
            wal_config: None,
            optimizers_config: None,
            quantization_config: None,
            sparse_vectors: None,
            strict_mode_config: None,
            uuid: None,
            metadata: None,
        };

        let msg = CollectionRequest::Create((name.into(), data));
        match send_request(&self.tx, msg.into()).await {
            Ok(RROResponse::Collection(CollectionResponse::Create(v))) => Ok(v),

            Err(e) => Err(e),
            res => Err(RROError::unexpected("expected response", res)),
        }
    }

    /// List all collections.
    pub async fn list_collections(&self) -> Result<Vec<String>, RROError> {
        match send_request(&self.tx, CollectionRequest::List.into()).await {
            Ok(RROResponse::Collection(CollectionResponse::List(v))) => Ok(v),
            Err(e) => Err(e),
            res => Err(RROError::unexpected("expected response", res)),
        }
    }

    /// Get collection info by name.
    pub async fn get_collection(
        &self,
        name: impl Into<String>,
    ) -> Result<Option<CollectionInfo>, RROError> {
        match send_request(&self.tx, CollectionRequest::Get(name.into()).into()).await {
            Ok(RROResponse::Collection(CollectionResponse::Get(v))) => Ok(Some(v)),
            Err(RROError::Collection(CollectionError::NotFound { .. })) => Ok(None),
            Err(e) => Err(e),
            res => Err(RROError::unexpected("expected response", res)),
        }
    }

    /// Update collection info by name.
    pub async fn update_collection(
        &self,
        name: impl Into<String>,
        data: UpdateCollection,
    ) -> Result<bool, RROError> {
        let msg = CollectionRequest::Update((name.into(), data));
        match send_request(&self.tx, msg.into()).await {
            Ok(RROResponse::Collection(CollectionResponse::Update(v))) => Ok(v),
            Err(e) => Err(e),
            res => Err(RROError::unexpected("expected response", res)),
        }
    }

    /// Delete collection by name.
    pub async fn delete_collection(&self, name: impl Into<String>) -> Result<bool, RROError> {
        match send_request(&self.tx, CollectionRequest::Delete(name.into()).into()).await {
            Ok(RROResponse::Collection(CollectionResponse::Delete(v))) => Ok(v),
            Err(e) => Err(e),
            res => Err(RROError::unexpected("expected response", res)),
        }
    }

    /// Check if collection exists.
    pub async fn collection_exists(&self, name: impl Into<String>) -> Result<bool, RROError> {
        match send_request(&self.tx, CollectionRequest::Exists(name.into()).into()).await {
            Ok(RROResponse::Collection(CollectionResponse::Exists(v))) => Ok(v),
            Err(e) => Err(e),
            res => Err(RROError::unexpected("expected response", res)),
        }
    }

    /// Create alias for collection.
    pub async fn create_alias(
        &self,
        collection_name: impl Into<String>,
        alias_name: impl Into<String>,
    ) -> Result<bool, RROError> {
        let msg = AliasRequest::Create((collection_name.into(), alias_name.into()));
        match send_request(&self.tx, msg.into()).await {
            Ok(RROResponse::Alias(AliasResponse::Create(v))) => Ok(v),
            Err(e) => Err(e),
            res => Err(RROError::unexpected("expected response", res)),
        }
    }

    /// List all aliases.
    pub async fn list_aliases(&self) -> Result<Vec<(ColName, String)>, RROError> {
        match send_request(&self.tx, AliasRequest::List.into()).await {
            Ok(RROResponse::Alias(AliasResponse::List(v))) => {
                let res = v
                    .aliases
                    .into_iter()
                    .map(|v| (v.collection_name, v.alias_name))
                    .collect();
                Ok(res)
            }
            Err(e) => Err(e),
            res => Err(RROError::unexpected("expected response", res)),
        }
    }

    /// Get aliases for collection.
    pub async fn get_aliases(
        &self,
        collection_name: impl Into<String>,
    ) -> Result<Vec<(ColName, String)>, RROError> {
        match send_request(&self.tx, AliasRequest::Get(collection_name.into()).into()).await {
            Ok(RROResponse::Alias(AliasResponse::Get(v))) => {
                let res = v
                    .aliases
                    .into_iter()
                    .map(|v| (v.collection_name, v.alias_name))
                    .collect();
                Ok(res)
            }
            Err(e) => Err(e),
            res => Err(RROError::unexpected("expected response", res)),
        }
    }

    /// Delete alias.
    pub async fn delete_alias(&self, alias_name: impl Into<String>) -> Result<bool, RROError> {
        let msg = AliasRequest::Delete(alias_name.into());
        match send_request(&self.tx, msg.into()).await {
            Ok(RROResponse::Alias(AliasResponse::Delete(v))) => Ok(v),
            Err(e) => Err(e),
            res => Err(RROError::unexpected("expected response", res)),
        }
    }

    /// Rename alias.
    pub async fn rename_alias(
        &self,
        old_alias_name: impl Into<String>,
        new_alias_name: impl Into<String>,
    ) -> Result<bool, RROError> {
        let msg = AliasRequest::Rename((old_alias_name.into(), new_alias_name.into()));
        match send_request(&self.tx, msg.into()).await {
            Ok(RROResponse::Alias(AliasResponse::Rename(v))) => Ok(v),
            Err(e) => Err(e),
            res => Err(RROError::unexpected("expected response", res)),
        }
    }

    /// get points from collection
    pub async fn get_points(
        &self,
        collection_name: impl Into<String>,
        data: PointRequest,
    ) -> Result<Vec<LocalRecord>, RROError> {
        let msg = PointsRequest::Get((collection_name.into(), data));
        match send_request(&self.tx, msg.into()).await {
            Ok(RROResponse::Points(PointsResponse::Get(v))) => Ok(v),
            Err(e) => Err(e),
            res => Err(RROError::unexpected("expected response", res)),
        }
    }

    /// Scroll through points in a collection with pagination.
    ///
    /// Returns points matching optional filter, with pagination support via offset.
    pub async fn scroll_points(
        &self,
        collection_name: impl Into<String>,
        request: collection::operations::types::ScrollRequest,
    ) -> Result<collection::operations::types::ScrollResult, RROError> {
        let msg = PointsRequest::Scroll((collection_name.into(), request));
        match send_request(&self.tx, msg.into()).await {
            Ok(RROResponse::Points(PointsResponse::Scroll(result))) => Ok(result),
            Err(e) => Err(e),
            res => Err(RROError::unexpected("expected Scroll response", res)),
        }
    }

    /// upsert points to collection
    pub async fn upsert_points(
        &self,
        collection_name: impl Into<String>,
        points: Vec<PointStruct>,
    ) -> Result<UpdateResult, RROError> {
        use api::rest::schema::PointInsertOperations;
        let ops = PointInsertOperations::PointsList(api::rest::schema::PointsList {
            points,
            shard_key: None,
            update_filter: None,
        });
        let msg = PointsRequest::Upsert((collection_name.into(), ops));
        match send_request(&self.tx, msg.into()).await {
            Ok(RROResponse::Points(PointsResponse::Upsert(v))) => Ok(v),
            Err(e) => Err(e),
            res => Err(RROError::unexpected("expected response", res)),
        }
    }

    /// delete points from collection
    pub async fn delete_points(
        &self,
        collection_name: impl Into<String>,
        points: PointsSelector,
    ) -> Result<UpdateResult, RROError> {
        let msg = PointsRequest::Delete((collection_name.into(), points));
        match send_request(&self.tx, msg.into()).await {
            Ok(RROResponse::Points(PointsResponse::Delete(v))) => Ok(v),
            Err(e) => Err(e),
            res => Err(RROError::unexpected("expected response", res)),
        }
    }

    /// count points in collection
    pub async fn count_points(
        &self,
        collection_name: impl Into<String>,
        filter: Option<Filter>,
        exact: bool,
    ) -> Result<usize, RROError> {
        let data = CountRequest {
            count_request: CountRequestInternal { filter, exact },
            shard_key: None,
        };
        let msg = PointsRequest::Count((collection_name.into(), data));
        match send_request(&self.tx, msg.into()).await {
            Ok(RROResponse::Points(PointsResponse::Count(v))) => Ok(v.count),
            Err(e) => Err(e),
            res => Err(RROError::unexpected("expected response", res)),
        }
    }

    /// update point vectors
    pub async fn update_vectors(
        &self,
        collection_name: impl Into<String>,
        points: Vec<PointVectors>,
    ) -> Result<UpdateResult, RROError> {
        let data = UpdateVectors {
            points,
            shard_key: None,
            update_filter: None,
        };
        let msg = PointsRequest::UpdateVectors((collection_name.into(), data));
        match send_request(&self.tx, msg.into()).await {
            Ok(RROResponse::Points(PointsResponse::UpdateVectors(v))) => Ok(v),
            Err(e) => Err(e),
            res => Err(RROError::unexpected("expected response", res)),
        }
    }

    /// delete point vectors
    pub async fn delete_vectors(
        &self,
        collection_name: impl Into<String>,
        data: DeleteVectors,
    ) -> Result<UpdateResult, RROError> {
        let msg = PointsRequest::DeleteVectors((collection_name.into(), data));
        match send_request(&self.tx, msg.into()).await {
            Ok(RROResponse::Points(PointsResponse::DeleteVectors(v))) => Ok(v),
            Err(e) => Err(e),
            res => Err(RROError::unexpected("expected response", res)),
        }
    }

    /// set point payload
    pub async fn set_payload(
        &self,
        collection_name: impl Into<String>,
        data: SetPayload,
    ) -> Result<UpdateResult, RROError> {
        let msg = PointsRequest::SetPayload((collection_name.into(), data));
        match send_request(&self.tx, msg.into()).await {
            Ok(RROResponse::Points(PointsResponse::SetPayload(v))) => Ok(v),
            Err(e) => Err(e),
            res => Err(RROError::unexpected("expected response", res)),
        }
    }

    /// delete point payload
    pub async fn delete_payload(
        &self,
        collection_name: impl Into<String>,
        data: DeletePayload,
    ) -> Result<UpdateResult, RROError> {
        let msg = PointsRequest::DeletePayload((collection_name.into(), data));
        match send_request(&self.tx, msg.into()).await {
            Ok(RROResponse::Points(PointsResponse::DeletePayload(v))) => Ok(v),
            Err(e) => Err(e),
            res => Err(RROError::unexpected("expected response", res)),
        }
    }

    /// clear point payload
    pub async fn clear_payload(
        &self,
        collection_name: impl Into<String>,
        points: PointsSelector,
    ) -> Result<UpdateResult, RROError> {
        let msg = PointsRequest::ClearPayload((collection_name.into(), points));
        match send_request(&self.tx, msg.into()).await {
            Ok(RROResponse::Points(PointsResponse::ClearPayload(v))) => Ok(v),
            Err(e) => Err(e),
            res => Err(RROError::unexpected("expected response", res)),
        }
    }

    /// search for vectors
    pub async fn search_points(
        &self,
        collection_name: impl Into<String>,
        data: SearchRequest,
    ) -> Result<Vec<LocalScoredPoint>, RROError> {
        let msg = QueryRequest::Search((collection_name.into(), data));
        match send_request(&self.tx, msg.into()).await {
            Ok(RROResponse::Query(QueryResponse::Search(v))) => Ok(v),
            Err(e) => Err(e),
            res => Err(RROError::unexpected("expected response", res)),
        }
    }

    // search for vectors in batch
    pub async fn search_points_batch(
        &self,
        collection_name: impl Into<String>,
        data: Vec<SearchRequest>,
    ) -> Result<Vec<Vec<LocalScoredPoint>>, RROError> {
        let data = SearchRequestBatch { searches: data };
        let msg = QueryRequest::SearchBatch((collection_name.into(), data));
        match send_request(&self.tx, msg.into()).await {
            Ok(RROResponse::Query(QueryResponse::SearchBatch(v))) => Ok(v),
            Err(e) => Err(e),
            res => Err(RROError::unexpected("expected response", res)),
        }
    }

    /// search points group by
    pub async fn search_points_group_by(
        &self,
        collection_name: impl Into<String>,
        data: SearchGroupsRequest,
    ) -> Result<Vec<PointGroup>, RROError> {
        let msg = QueryRequest::SearchGroup((collection_name.into(), data));
        match send_request(&self.tx, msg.into()).await {
            Ok(RROResponse::Query(QueryResponse::SearchGroup(v))) => Ok(v.groups),
            Err(e) => Err(e),
            res => Err(RROError::unexpected("expected response", res)),
        }
    }

    /// universal query points
    pub async fn query_points(
        &self,
        collection_name: impl Into<String>,
        data: QueryPointsRequest,
    ) -> Result<Vec<LocalScoredPoint>, RROError> {
        let msg = QueryRequest::Query((collection_name.into(), data));
        match send_request(&self.tx, msg.into()).await {
            Ok(RROResponse::Query(QueryResponse::Query(v))) => Ok(v),
            Err(e) => Err(e),
            res => Err(RROError::unexpected("expected response", res)),
        }
    }

    /// recommend result
    pub async fn recommend_points(
        &self,
        collection_name: impl Into<String>,
        data: RecommendRequest,
    ) -> Result<Vec<LocalScoredPoint>, RROError> {
        let msg = QueryRequest::Recommend((collection_name.into(), data));
        match send_request(&self.tx, msg.into()).await {
            Ok(RROResponse::Query(QueryResponse::Recommend(v))) => Ok(v),
            Err(e) => Err(e),
            res => Err(RROError::unexpected("expected response", res)),
        }
    }

    /// recommend batch
    pub async fn recommend_points_batch(
        &self,
        collection_name: impl Into<String>,
        data: Vec<RecommendRequest>,
    ) -> Result<Vec<Vec<LocalScoredPoint>>, RROError> {
        let data = RecommendRequestBatch { searches: data };
        let msg = QueryRequest::RecommendBatch((collection_name.into(), data));
        match send_request(&self.tx, msg.into()).await {
            Ok(RROResponse::Query(QueryResponse::RecommendBatch(v))) => Ok(v),
            Err(e) => Err(e),
            res => Err(RROError::unexpected("expected response", res)),
        }
    }

    /// recommend group by
    pub async fn recommend_points_group_by(
        &self,
        collection_name: impl Into<String>,
        data: RecommendGroupsRequest,
    ) -> Result<Vec<PointGroup>, RROError> {
        let msg = QueryRequest::RecommendGroup((collection_name.into(), data));
        match send_request(&self.tx, msg.into()).await {
            Ok(RROResponse::Query(QueryResponse::RecommendGroup(v))) => Ok(v.groups),
            Err(e) => Err(e),
            res => Err(RROError::unexpected("expected response", res)),
        }
    }
}

async fn send_request(
    sender: &mpsc::Sender<RROMsg>,
    msg: RRORequest,
) -> Result<RROResponse, RROError> {
    send_request_with_timeout(sender, msg, std::time::Duration::from_secs(30)).await
}

/// Send a request with a configurable timeout
async fn send_request_with_timeout(
    sender: &mpsc::Sender<RROMsg>,
    msg: RRORequest,
    timeout: std::time::Duration,
) -> Result<RROResponse, RROError> {
    let (tx, rx) = oneshot::channel::<RROResult>();
    
    // Send request, return ChannelClosed if instance is shutting down
    sender.send((msg, tx)).await.map_err(|_| RROError::ChannelClosed)?;
    
    // Wait for response with timeout
    match tokio::time::timeout(timeout, rx).await {
        Ok(Ok(result)) => Ok(result?),
        Ok(Err(_)) => Err(RROError::ChannelClosed), // Response channel closed
        Err(_) => Err(RROError::Timeout(timeout)),
    }
}

