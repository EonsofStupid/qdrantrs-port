use std::time::Duration;

use super::{shard_selector, ColName};
use crate::{Handler, QdrantRequest};
use api::rest::schema::SearchGroupsRequestInternal;
use async_trait::async_trait;
use collection::{
    common::batching::batch_requests,
    operations::{
        consistency_params::ReadConsistency,
        shard_selector_internal::ShardSelectorInternal,
        types::{
            GroupsResult, RecommendGroupsRequest, RecommendGroupsRequestInternal,
            RecommendRequest, RecommendRequestBatch, SearchGroupsRequest, SearchRequest,
            SearchRequestBatch,
        },

        universal_query::collection_query::{
            CollectionPrefetch, CollectionQueryRequest, Mmr, NearestWithMmr, Query,
            VectorInputInternal, VectorQuery,
        },
        universal_query::formula::FormulaInternal,
        universal_query::shard_query::{FusionInternal, SampleInternal},
    },
};
use api::rest::schema as rest;
use ordered_float::OrderedFloat;
use segment::data_types::order_by::OrderBy;
use segment::data_types::vectors::{DEFAULT_VECTOR_NAME, MultiDenseVectorInternal, VectorInternal};
use segment::vector_storage::query::{ContextPair, ContextQuery, DiscoveryQuery, RecoQuery};
use common::counter::hardware_accumulator::HwMeasurementAcc;
use serde::{Deserialize, Serialize};
use shard::search::{CoreSearchRequest, CoreSearchRequestBatch};
use storage::content_manager::{errors::StorageError, toc::TableOfContent};
use storage::rbac::Access;

/// Local scored point type (segment::types::ScoredPoint doesn't impl Serialize in v1.16)
#[derive(Debug, Serialize, Clone)]
pub struct LocalScoredPoint {
    pub id: String,
    pub score: f32,
    pub payload: Option<serde_json::Value>,
    pub vector: Option<Vec<f32>>,
}

impl From<segment::types::ScoredPoint> for LocalScoredPoint {
    fn from(p: segment::types::ScoredPoint) -> Self {
        Self {
            id: format!("{:?}", p.id),
            score: p.score,
            payload: p.payload.map(|p| serde_json::to_value(p).unwrap_or_default()),
            vector: None, // Skip vector for serialization
        }
    }
}

#[derive(Debug, Deserialize)]
pub enum QueryRequest {
    /// search for vectors
    Search((ColName, SearchRequest)),
    /// search for vectors in batch
    SearchBatch((ColName, SearchRequestBatch)),
    /// search group by
    SearchGroup((ColName, SearchGroupsRequest)),
    /// recommend points
    Recommend((ColName, RecommendRequest)),
    /// recommend points in batch
    RecommendBatch((ColName, RecommendRequestBatch)),
    /// recommend group by
    RecommendGroup((ColName, RecommendGroupsRequest)),
    /// universal query
    Query((ColName, rest::QueryRequest)),
}

#[derive(Debug, Serialize)]
pub enum QueryResponse {
    /// search result
    Search(Vec<LocalScoredPoint>),
    /// search result in batch
    SearchBatch(Vec<Vec<LocalScoredPoint>>),
    /// search group by result
    SearchGroup(GroupsResult),
    /// recommend result
    Recommend(Vec<LocalScoredPoint>),
    /// recommend result in batch
    RecommendBatch(Vec<Vec<LocalScoredPoint>>),
    /// recommend group by result
    RecommendGroup(GroupsResult),
    /// universal query result
    Query(Vec<LocalScoredPoint>),
}

#[async_trait]
impl Handler for QueryRequest {
    type Response = QueryResponse;
    type Error = StorageError;

    async fn handle(self, toc: &TableOfContent) -> Result<Self::Response, Self::Error> {
        let access = Access::full("Embedded");
        let hw_acc = HwMeasurementAcc::disposable();

        match self {
            QueryRequest::Search((collection_name, request)) => {
                let SearchRequest {
                    search_request,
                    shard_key,
                } = request;

                let shard = shard_selector(shard_key);
                let res = do_core_search_points(
                    toc,
                    &collection_name,
                    search_request.into(),
                    None,
                    shard,
                    access,
                    None,
                    hw_acc,
                )
                .await?;
                Ok(QueryResponse::Search(
                    res.into_iter().map(Into::into).collect(),
                ))
            }
            QueryRequest::SearchBatch((collection_name, request)) => {
                let requests = request
                    .searches
                    .into_iter()
                    .map(|req| {
                        let SearchRequest {
                            search_request,
                            shard_key,
                        } = req;
                        let shard = shard_selector(shard_key);
                        let core_request: CoreSearchRequest = search_request.into();

                        (core_request, shard)
                    })
                    .collect();

                let res = do_search_batch_points(
                    toc,
                    &collection_name,
                    requests,
                    None,
                    access,
                    None,
                    hw_acc,
                )
                .await?;
                Ok(QueryResponse::SearchBatch(
                    res.into_iter()
                        .map(|v| v.into_iter().map(Into::into).collect())
                        .collect(),
                ))
            }
            QueryRequest::SearchGroup((collection_name, request)) => {
                let SearchGroupsRequest {
                    search_group_request,
                    shard_key,
                } = request;

                let shard = shard_selector(shard_key);
                let res = do_search_point_groups(
                    toc,
                    &collection_name,
                    search_group_request,
                    None,
                    shard,
                    access,
                    None,
                    hw_acc,
                )
                .await?;
                Ok(QueryResponse::SearchGroup(res))
            }
            QueryRequest::Recommend((collection_name, request)) => {
                let RecommendRequest {
                    recommend_request,
                    shard_key,
                } = request;

                let shard = shard_selector(shard_key);
                let res = toc
                    .recommend(
                        &collection_name,
                        recommend_request,
                        None,
                        shard,
                        access,
                        None,
                        hw_acc,
                    )
                    .await?;
                Ok(QueryResponse::Recommend(
                    res.into_iter().map(Into::into).collect(),
                ))
            }
            QueryRequest::RecommendBatch((collection_name, request)) => {
                let res = do_recommend_batch_points(
                    toc,
                    &collection_name,
                    request,
                    None,
                    access,
                    None,
                    hw_acc,
                )
                .await?;
                Ok(QueryResponse::RecommendBatch(
                    res.into_iter()
                        .map(|v| v.into_iter().map(Into::into).collect())
                        .collect(),
                ))
            }
            QueryRequest::RecommendGroup((collection_name, request)) => {
                let RecommendGroupsRequest {
                    recommend_group_request,
                    shard_key,
                } = request;

                let shard = shard_selector(shard_key);
                let res = do_recommend_point_groups(
                    toc,
                    &collection_name,
                    recommend_group_request,
                    None,
                    shard,
                    access,
                    None,
                    hw_acc,
                )
                .await?;
                Ok(QueryResponse::RecommendGroup(res))
            }
            QueryRequest::Query((collection_name, request)) => {
                let shard = shard_selector(request.shard_key.clone());
                
                let collection_query_request = convert_query_request_from_rest(request.internal)?;

                let requests = vec![(collection_query_request, shard)];
                let res = toc
                    .query_batch(
                        &collection_name,
                        requests,
                        None, // read_consistency
                        access,
                        None, // timeout
                        hw_acc,
                    )
                    .await?;
                
                let points = res.into_iter().next().unwrap_or_default();
                Ok(QueryResponse::Query(
                    points.into_iter().map(Into::into).collect(),
                ))
            }
        }
    }
}

fn convert_query_request_from_rest(
    request: rest::QueryRequestInternal,
) -> Result<CollectionQueryRequest, StorageError> {
    let rest::QueryRequestInternal {
        prefetch,
        query,
        using,
        filter,
        score_threshold,
        params,
        limit,
        offset,
        with_vector,
        with_payload,
        lookup_from,
    } = request;

    let prefetch = prefetch
        .map(|prefetches| {
            prefetches
                .into_iter()
                .map(convert_prefetch)
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?
        .unwrap_or_default();

    let query = query
        .map(convert_query)
        .transpose()?;

    Ok(CollectionQueryRequest {
        prefetch,
        query,
        using: using.unwrap_or_else(|| DEFAULT_VECTOR_NAME.to_owned()),
        filter,
        score_threshold,
        limit: limit.unwrap_or(CollectionQueryRequest::DEFAULT_LIMIT),
        offset: offset.unwrap_or(CollectionQueryRequest::DEFAULT_OFFSET),
        params,
        with_vector: with_vector.unwrap_or(CollectionQueryRequest::DEFAULT_WITH_VECTOR),
        with_payload: with_payload.unwrap_or(CollectionQueryRequest::DEFAULT_WITH_PAYLOAD),
        lookup_from,
    })
}

fn convert_prefetch(
    prefetch: rest::Prefetch,
) -> Result<CollectionPrefetch, StorageError> {
    let rest::Prefetch {
        prefetch,
        query,
        using,
        filter,
        score_threshold,
        params,
        limit,
        lookup_from,
    } = prefetch;

    let query = query
        .map(convert_query)
        .transpose()?;
    let nested_prefetches = prefetch
        .map(|prefetches| {
            prefetches
                .into_iter()
                .map(convert_prefetch)
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?
        .unwrap_or_default();

    Ok(CollectionPrefetch {
        prefetch: nested_prefetches,
        query,
        using: using.unwrap_or_else(|| DEFAULT_VECTOR_NAME.to_owned()),
        filter,
        score_threshold: score_threshold.map(OrderedFloat),
        limit: limit.unwrap_or(CollectionQueryRequest::DEFAULT_LIMIT),
        params,
        lookup_from,
    })
}

fn convert_query(
    query: rest::QueryInterface,
) -> Result<Query, StorageError> {
    let query = rest::Query::from(query);
    match query {
        rest::Query::Nearest(rest::NearestQuery { nearest, mmr }) => {
            let vector = convert_vector_input(nearest)?;

            if let Some(mmr) = mmr {
                let mmr = Mmr {
                    diversity: mmr.diversity,
                    candidates_limit: mmr.candidates_limit,
                };
                Ok(Query::Vector(VectorQuery::NearestWithMmr(NearestWithMmr {
                    nearest: vector,
                    mmr,
                })))
            } else {
                Ok(Query::Vector(VectorQuery::Nearest(vector)))
            }
        }
        rest::Query::Recommend(recommend) => {
            let rest::RecommendInput {
                positive,
                negative,
                strategy,
            } = recommend.recommend;
            let positives = positive
                .into_iter()
                .flatten()
                .map(convert_vector_input)
                .collect::<Result<Vec<_>, _>>()?;
            let negatives = negative
                .into_iter()
                .flatten()
                .map(convert_vector_input)
                .collect::<Result<Vec<_>, _>>()?;
            let reco_query = RecoQuery::new(positives, negatives);
            match strategy.unwrap_or_default() {
                rest::RecommendStrategy::AverageVector => Ok(Query::Vector(
                    VectorQuery::RecommendAverageVector(reco_query),
                )),
                rest::RecommendStrategy::BestScore => {
                    Ok(Query::Vector(VectorQuery::RecommendBestScore(reco_query)))
                }
                rest::RecommendStrategy::SumScores => {
                    Ok(Query::Vector(VectorQuery::RecommendSumScores(reco_query)))
                }
            }
        }
        rest::Query::Discover(discover) => {
            let rest::DiscoverInput { target, context } = discover.discover;
            let target = convert_vector_input(target)?;
            let context = context
                .into_iter()
                .flatten()
                .map(context_pair_from_rest)
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Query::Vector(VectorQuery::Discover(DiscoveryQuery::new(
                target, context,
            ))))
        }
        rest::Query::Context(context) => {
            let rest::ContextInput(context) = context.context;
            let context = context
                .into_iter()
                .flatten()
                .map(context_pair_from_rest)
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Query::Vector(VectorQuery::Context(ContextQuery::new(
                context,
            ))))
        }
        rest::Query::OrderBy(order_by) => Ok(Query::OrderBy(OrderBy::from(order_by.order_by))),
        rest::Query::Fusion(fusion) => Ok(Query::Fusion(FusionInternal::from(fusion.fusion))),
        rest::Query::Rrf(rrf) => Ok(Query::Fusion(FusionInternal::from(rrf.rrf))),
        rest::Query::Formula(formula) => Ok(Query::Formula(FormulaInternal::from(formula))),
        rest::Query::Sample(sample) => Ok(Query::Sample(SampleInternal::from(sample.sample))),
    }
}

fn convert_vector_input(
    vector: rest::VectorInput,
) -> Result<VectorInputInternal, StorageError> {
    match vector {
        rest::VectorInput::Id(id) => Ok(VectorInputInternal::Id(id)),
        rest::VectorInput::DenseVector(dense) => {
            Ok(VectorInputInternal::Vector(VectorInternal::Dense(dense)))
        }
        rest::VectorInput::SparseVector(sparse) => {
            Ok(VectorInputInternal::Vector(VectorInternal::Sparse(sparse)))
        }
        rest::VectorInput::MultiDenseVector(multi_dense) => Ok(VectorInputInternal::Vector(
            VectorInternal::MultiDense(MultiDenseVectorInternal::new_unchecked(multi_dense)),
        )),
        rest::VectorInput::Document(_) | rest::VectorInput::Image(_) | rest::VectorInput::Object(_) => {
            Err(StorageError::bad_request("Inference (Document/Image/Object) not supported in embedded mode"))
        }
    }
}

fn context_pair_from_rest(
    value: rest::ContextPair,
) -> Result<ContextPair<VectorInputInternal>, StorageError> {
    let rest::ContextPair { positive, negative } = value;
    Ok(ContextPair {
        positive: convert_vector_input(positive)?,
        negative: convert_vector_input(negative)?,
    })
}

impl From<QueryRequest> for QdrantRequest {
    fn from(req: QueryRequest) -> Self {
        QdrantRequest::Query(req)
    }
}

async fn do_core_search_points(
    toc: &TableOfContent,
    collection_name: &str,
    request: CoreSearchRequest,
    read_consistency: Option<ReadConsistency>,
    shard_selection: ShardSelectorInternal,
    access: Access,
    timeout: Option<Duration>,
    hw_acc: HwMeasurementAcc,
) -> Result<Vec<segment::types::ScoredPoint>, StorageError> {
    let batch_res = do_core_search_batch_points(
        toc,
        collection_name,
        CoreSearchRequestBatch {
            searches: vec![request],
        },
        read_consistency,
        shard_selection,
        access,
        timeout,
        hw_acc,
    )
    .await?;
    batch_res
        .into_iter()
        .next()
        .ok_or_else(|| StorageError::service_error("Empty search result"))
}

async fn do_search_batch_points(
    toc: &TableOfContent,
    collection_name: &str,
    requests: Vec<(CoreSearchRequest, ShardSelectorInternal)>,
    read_consistency: Option<ReadConsistency>,
    access: Access,
    timeout: Option<Duration>,
    hw_acc: HwMeasurementAcc,
) -> Result<Vec<Vec<segment::types::ScoredPoint>>, StorageError> {
    let requests = batch_requests::<
        (CoreSearchRequest, ShardSelectorInternal),
        ShardSelectorInternal,
        Vec<CoreSearchRequest>,
        Vec<_>,
    >(
        requests,
        |(_, shard_selector)| shard_selector,
        |(request, _), core_reqs| {
            core_reqs.push(request);
            Ok(())
        },
        |shard_selector, core_requests, res| {
            if core_requests.is_empty() {
                return Ok(());
            }

            let core_batch = CoreSearchRequestBatch {
                searches: core_requests,
            };

            let req = toc.core_search_batch(
                collection_name,
                core_batch,
                read_consistency,
                shard_selector,
                access.clone(),
                timeout,
                hw_acc.clone(),
            );
            res.push(req);
            Ok(())
        },
    )?;

    let results = futures::future::try_join_all(requests).await?;
    let flatten_results: Vec<Vec<_>> = results.into_iter().flatten().collect();
    Ok(flatten_results)
}

async fn do_core_search_batch_points(
    toc: &TableOfContent,
    collection_name: &str,
    request: CoreSearchRequestBatch,
    read_consistency: Option<ReadConsistency>,
    shard_selection: ShardSelectorInternal,
    access: Access,
    timeout: Option<Duration>,
    hw_acc: HwMeasurementAcc,
) -> Result<Vec<Vec<segment::types::ScoredPoint>>, StorageError> {
    toc.core_search_batch(
        collection_name,
        request,
        read_consistency,
        shard_selection,
        access,
        timeout,
        hw_acc,
    )
    .await
}

async fn do_search_point_groups(
    toc: &TableOfContent,
    collection_name: &str,
    request: SearchGroupsRequestInternal,
    read_consistency: Option<ReadConsistency>,
    shard_selection: ShardSelectorInternal,
    access: Access,
    timeout: Option<Duration>,
    hw_acc: HwMeasurementAcc,
) -> Result<GroupsResult, StorageError> {
    toc.group(
        collection_name,
        request.into(),
        read_consistency,
        shard_selection,
        access,
        timeout,
        hw_acc,
    )
    .await
}

async fn do_recommend_point_groups(
    toc: &TableOfContent,
    collection_name: &str,
    request: RecommendGroupsRequestInternal,
    read_consistency: Option<ReadConsistency>,
    shard_selection: ShardSelectorInternal,
    access: Access,
    timeout: Option<Duration>,
    hw_acc: HwMeasurementAcc,
) -> Result<GroupsResult, StorageError> {
    toc.group(
        collection_name,
        request.into(),
        read_consistency,
        shard_selection,
        access,
        timeout,
        hw_acc,
    )
    .await
}

async fn do_recommend_batch_points(
    toc: &TableOfContent,
    collection_name: &str,
    request: RecommendRequestBatch,
    read_consistency: Option<ReadConsistency>,
    access: Access,
    timeout: Option<Duration>,
    hw_acc: HwMeasurementAcc,
) -> Result<Vec<Vec<segment::types::ScoredPoint>>, StorageError> {
    let requests = request
        .searches
        .into_iter()
        .map(|req| {
            let shard = shard_selector(req.shard_key);
            (req.recommend_request, shard)
        })
        .collect();

    toc.recommend_batch(collection_name, requests, read_consistency, access, timeout, hw_acc)
        .await
}
