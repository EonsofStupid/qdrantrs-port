use super::{shard_selector, ColName};
use crate::{Handler, QdrantRequest};
use api::rest::schema::{PointInsertOperations, PointsBatch, PointsList, ShardKeySelector, UpdateVectors};
use async_trait::async_trait;
use collection::operations::{
    point_ops::{FilterSelector, PointIdsList, PointsSelector, WriteOrdering},
    shard_selector_internal::ShardSelectorInternal,
    types::{CountRequest, CountResult, PointRequest, UpdateResult},
    vector_ops::DeleteVectors,
};
use common::counter::hardware_accumulator::HwMeasurementAcc;
use segment::types::Filter;
use serde::{Deserialize, Serialize};
use shard::operations::{
    payload_ops::{DeletePayloadOp, PayloadOps, SetPayloadOp},
    point_ops::{PointInsertOperationsInternal, PointOperations, PointStructPersisted, VectorStructPersisted, VectorPersisted},
    vector_ops::{PointVectorsPersisted, UpdateVectorsOp, VectorOperations},
    CollectionUpdateOperations,
};
use std::collections::HashMap;
use storage::content_manager::{errors::StorageError, toc::TableOfContent};
use storage::rbac::Access;

// Re-export payload types from collection for handler use
use collection::operations::payload_ops::{DeletePayload, SetPayload};

pub type ShardId = u32;

#[derive(Debug, Deserialize)]
pub enum PointsRequest {
    /// get points with given info
    Get((ColName, PointRequest)),
    /// count points for given collection
    Count((ColName, CountRequest)),
    /// delete points with given info
    Delete((ColName, PointsSelector)),
    /// upsert points with given info
    Upsert((ColName, PointInsertOperations)),
    /// update point vectors
    UpdateVectors((ColName, UpdateVectors)),
    /// delete point vectors
    DeleteVectors((ColName, DeleteVectors)),
    /// set point payload
    SetPayload((ColName, SetPayload)),
    /// overwrite point payload
    OverwritePayload((ColName, SetPayload)),
    /// delete point payload
    DeletePayload((ColName, DeletePayload)),
    /// clear point payload
    ClearPayload((ColName, PointsSelector)),
}

/// Local record type for serialization
#[derive(Debug, Serialize)]
pub struct LocalRecord {
    pub id: String,
    pub payload: Option<serde_json::Value>,
    pub vector: Option<Vec<f32>>,
}

#[derive(Debug, Serialize)]
pub enum PointsResponse {
    /// get points result
    Get(Vec<LocalRecord>),
    /// count status
    Count(CountResult),
    /// delete status
    Delete(UpdateResult),
    /// upsert status
    Upsert(UpdateResult),
    /// update status
    UpdateVectors(UpdateResult),
    /// delete status
    DeleteVectors(UpdateResult),
    /// set payload status
    SetPayload(UpdateResult),
    /// overwrite payload status
    OverwritePayload(UpdateResult),
    /// delete payload status
    DeletePayload(UpdateResult),
    /// clear payload status
    ClearPayload(UpdateResult),
}

#[async_trait]
impl Handler for PointsRequest {
    type Response = PointsResponse;
    type Error = StorageError;

    async fn handle(self, toc: &TableOfContent) -> Result<Self::Response, Self::Error> {
        let access = Access::full("Embedded");
        let hw_acc = HwMeasurementAcc::disposable();

        match self {
            PointsRequest::Get((col_name, request)) => {
                let PointRequest {
                    point_request,
                    shard_key,
                } = request;

                let shard = shard_selector(shard_key);
                let ret = toc
                    .retrieve(
                        &col_name,
                        point_request,
                        None,
                        None,
                        shard,
                        access,
                        hw_acc,
                    )
                    .await?;

                let records: Vec<LocalRecord> = ret
                    .into_iter()
                    .map(|r| LocalRecord {
                        id: format!("{:?}", r.id),
                        payload: r.payload.map(|p| serde_json::to_value(p).unwrap_or_default()),
                        vector: None,
                    })
                    .collect();

                Ok(PointsResponse::Get(records))
            }
            PointsRequest::Count((col_name, request)) => {
                let CountRequest {
                    count_request,
                    shard_key,
                } = request;

                let shard = shard_selector(shard_key);
                let ret = toc
                    .count(&col_name, count_request, None, None, shard, access, hw_acc)
                    .await?;
                Ok(PointsResponse::Count(ret))
            }
            PointsRequest::Delete((col_name, selector)) => {
                let ret = do_delete_points(
                    toc,
                    &col_name,
                    selector,
                    None,
                    false,
                    WriteOrdering::default(),
                    access,
                )
                .await?;
                Ok(PointsResponse::Delete(ret))
            }
            PointsRequest::Upsert((col_name, ops)) => {
                let ret = do_upsert_points(
                    toc,
                    &col_name,
                    ops,
                    None,
                    false,
                    WriteOrdering::default(),
                    access,
                )
                .await?;
                Ok(PointsResponse::Upsert(ret))
            }
            PointsRequest::UpdateVectors((col_name, operations)) => {
                let ret = do_update_vectors(
                    toc,
                    &col_name,
                    operations,
                    None,
                    false,
                    WriteOrdering::default(),
                    access,
                )
                .await?;
                Ok(PointsResponse::UpdateVectors(ret))
            }
            PointsRequest::DeleteVectors((col_name, operations)) => {
                let ret = do_delete_vectors(
                    toc,
                    &col_name,
                    operations,
                    None,
                    false,
                    WriteOrdering::default(),
                    access,
                )
                .await?;
                Ok(PointsResponse::DeleteVectors(ret))
            }
            PointsRequest::SetPayload((col_name, payload)) => {
                let ret = do_set_payload(
                    toc,
                    &col_name,
                    payload,
                    None,
                    false,
                    WriteOrdering::default(),
                    access,
                )
                .await?;
                Ok(PointsResponse::SetPayload(ret))
            }
            PointsRequest::OverwritePayload((col_name, payload)) => {
                let ret = do_overwrite_payload(
                    toc,
                    &col_name,
                    payload,
                    None,
                    false,
                    WriteOrdering::default(),
                    access,
                )
                .await?;
                Ok(PointsResponse::OverwritePayload(ret))
            }
            PointsRequest::DeletePayload((col_name, payload)) => {
                let ret = do_delete_payload(
                    toc,
                    &col_name,
                    payload,
                    None,
                    false,
                    WriteOrdering::default(),
                    access,
                )
                .await?;
                Ok(PointsResponse::DeletePayload(ret))
            }
            PointsRequest::ClearPayload((col_name, selector)) => {
                let ret = do_clear_payload(
                    toc,
                    &col_name,
                    selector,
                    None,
                    false,
                    WriteOrdering::default(),
                    access,
                )
                .await?;
                Ok(PointsResponse::ClearPayload(ret))
            }
        }
    }
}

impl From<PointsRequest> for QdrantRequest {
    fn from(req: PointsRequest) -> Self {
        QdrantRequest::Points(req)
    }
}

/// Convert API VectorStruct to internal VectorStructPersisted
/// Note: Document, Image, Object variants require inference and are not supported in embedded mode
fn convert_vector_struct(vector: api::rest::schema::VectorStruct) -> Result<VectorStructPersisted, StorageError> {
    use api::rest::schema::VectorStruct;
    match vector {
        VectorStruct::Single(v) => Ok(VectorStructPersisted::Single(v)),
        VectorStruct::MultiDense(v) => Ok(VectorStructPersisted::MultiDense(v)),
        VectorStruct::Named(map) => {
            let converted: Result<HashMap<_, _>, _> = map
                .into_iter()
                .map(|(name, v)| {
                    convert_vector(v).map(|vp| (name, vp))
                })
                .collect();
            Ok(VectorStructPersisted::Named(converted?))
        }
        VectorStruct::Document(_) | VectorStruct::Image(_) | VectorStruct::Object(_) => {
            Err(StorageError::bad_request(
                "Document, Image, and Object vector types require inference and are not supported in embedded mode. \
                 Please provide pre-computed vectors.",
            ))
        }
    }
}

/// Convert API Vector to internal VectorPersisted
fn convert_vector(vector: api::rest::schema::Vector) -> Result<VectorPersisted, StorageError> {
    use api::rest::schema::Vector;
    match vector {
        Vector::Dense(v) => Ok(VectorPersisted::Dense(v)),
        Vector::Sparse(v) => Ok(VectorPersisted::Sparse(v)),
        Vector::MultiDense(v) => Ok(VectorPersisted::MultiDense(v)),
        Vector::Document(_) | Vector::Image(_) | Vector::Object(_) => {
            Err(StorageError::bad_request(
                "Document, Image, and Object vector types require inference and are not supported in embedded mode.",
            ))
        }
    }
}

/// Convert API PointStruct to internal PointStructPersisted
fn convert_point_struct(point: api::rest::schema::PointStruct) -> Result<PointStructPersisted, StorageError> {
    Ok(PointStructPersisted {
        id: point.id,
        vector: convert_vector_struct(point.vector)?,
        payload: point.payload,
    })
}

/// Convert API PointVectors to internal PointVectorsPersisted
fn convert_point_vectors(pv: api::rest::schema::PointVectors) -> Result<PointVectorsPersisted, StorageError> {
    Ok(PointVectorsPersisted {
        id: pv.id,
        vector: convert_vector_struct(pv.vector)?,
    })
}

/// Convert API PointInsertOperations to internal format
/// Returns the internal operation, shard key, and optional update filter
fn convert_point_insert_operations(
    operation: PointInsertOperations,
) -> Result<(PointInsertOperationsInternal, Option<ShardKeySelector>, Option<Filter>), StorageError> {
    match operation {
        PointInsertOperations::PointsList(PointsList { points, shard_key, update_filter }) => {
            let converted: Result<Vec<_>, _> = points.into_iter().map(convert_point_struct).collect();
            Ok((PointInsertOperationsInternal::PointsList(converted?), shard_key, update_filter))
        }
        PointInsertOperations::PointsBatch(PointsBatch { batch, shard_key, update_filter }) => {
            // For batch operations, we need to convert to a list of points
            // The batch format has separate arrays for ids, vectors, payloads
            use api::rest::schema::BatchVectorStruct;

            let ids = batch.ids;
            let payloads = batch.payloads.unwrap_or_default();

            // Convert batch vectors to individual point vectors
            let points: Result<Vec<_>, _> = match batch.vectors {
                BatchVectorStruct::Single(vectors) => {
                    ids.into_iter()
                        .zip(vectors.into_iter())
                        .enumerate()
                        .map(|(i, (id, vec))| {
                            let payload = payloads.get(i).cloned().flatten();
                            Ok(PointStructPersisted {
                                id,
                                vector: VectorStructPersisted::Single(vec),
                                payload,
                            })
                        })
                        .collect()
                }
                BatchVectorStruct::MultiDense(vectors) => {
                    ids.into_iter()
                        .zip(vectors.into_iter())
                        .enumerate()
                        .map(|(i, (id, vec))| {
                            let payload = payloads.get(i).cloned().flatten();
                            Ok(PointStructPersisted {
                                id,
                                vector: VectorStructPersisted::MultiDense(vec),
                                payload,
                            })
                        })
                        .collect()
                }
                BatchVectorStruct::Named(named_vectors) => {
                    ids.into_iter()
                        .enumerate()
                        .map(|(i, id)| -> Result<PointStructPersisted, StorageError> {
                            let payload = payloads.get(i).cloned().flatten();
                            let mut point_vectors = HashMap::new();
                            for (name, vectors) in &named_vectors {
                                if let Some(vec) = vectors.get(i) {
                                    point_vectors.insert(name.clone(), convert_vector(vec.clone())?);
                                }
                            }
                            Ok(PointStructPersisted {
                                id,
                                vector: VectorStructPersisted::Named(point_vectors),
                                payload,
                            })
                        })
                        .collect()
                }
                BatchVectorStruct::Document(_) | BatchVectorStruct::Image(_) | BatchVectorStruct::Object(_) => {
                    return Err(StorageError::bad_request(
                        "Document, Image, and Object batch vector types require inference and are not supported in embedded mode.",
                    ));
                }
            };

            Ok((PointInsertOperationsInternal::PointsList(points?), shard_key, update_filter))
        }
    }
}

async fn do_upsert_points(
    toc: &TableOfContent,
    collection_name: &str,
    operation: PointInsertOperations,
    shard_selection: Option<ShardId>,
    wait: bool,
    ordering: WriteOrdering,
    access: Access,
) -> Result<UpdateResult, StorageError> {
    let hw_acc = HwMeasurementAcc::disposable();

    // Convert REST PointInsertOperations to internal format
    let (internal_op, shard_key, update_filter) = convert_point_insert_operations(operation)?;

    // Build the point operation - handle conditional upsert if update_filter is provided
    let point_op = if let Some(filter) = update_filter {
        PointOperations::UpsertPointsConditional(shard::operations::point_ops::ConditionalInsertOperationInternal {
            points_op: internal_op,
            condition: filter,
        })
    } else {
        PointOperations::UpsertPoints(internal_op)
    };

    let collection_operation = CollectionUpdateOperations::PointOperation(point_op);
    let shard_selector = get_shard_selector_for_update(shard_selection, shard_key);

    toc.update(
        collection_name,
        collection_operation.into(),
        wait,
        ordering,
        shard_selector,
        access,
        hw_acc,
    )
    .await
}

async fn do_delete_points(
    toc: &TableOfContent,
    collection_name: &str,
    points: PointsSelector,
    shard_selection: Option<ShardId>,
    wait: bool,
    ordering: WriteOrdering,
    access: Access,
) -> Result<UpdateResult, StorageError> {
    let hw_acc = HwMeasurementAcc::disposable();

    let (point_operation, shard_key) = match points {
        PointsSelector::PointIdsSelector(PointIdsList { points, shard_key }) => {
            (PointOperations::DeletePoints { ids: points }, shard_key)
        }
        PointsSelector::FilterSelector(FilterSelector { filter, shard_key }) => {
            (PointOperations::DeletePointsByFilter(filter), shard_key)
        }
    };
    let collection_operation = CollectionUpdateOperations::PointOperation(point_operation);
    let shard_selector = get_shard_selector_for_update(shard_selection, shard_key);

    toc.update(
        collection_name,
        collection_operation.into(),
        wait,
        ordering,
        shard_selector,
        access,
        hw_acc,
    )
    .await
}

async fn do_update_vectors(
    toc: &TableOfContent,
    collection_name: &str,
    operation: UpdateVectors,
    shard_selection: Option<ShardId>,
    wait: bool,
    ordering: WriteOrdering,
    access: Access,
) -> Result<UpdateResult, StorageError> {
    let hw_acc = HwMeasurementAcc::disposable();
    let UpdateVectors { points, shard_key, update_filter } = operation;

    // Convert API PointVectors to internal format
    let converted_points: Result<Vec<_>, _> = points.into_iter().map(convert_point_vectors).collect();

    let collection_operation = CollectionUpdateOperations::VectorOperation(
        VectorOperations::UpdateVectors(UpdateVectorsOp {
            points: converted_points?,
            update_filter,
        }),
    );

    let shard_selector = get_shard_selector_for_update(shard_selection, shard_key);

    toc.update(
        collection_name,
        collection_operation.into(),
        wait,
        ordering,
        shard_selector,
        access,
        hw_acc,
    )
    .await
}

async fn do_delete_vectors(
    toc: &TableOfContent,
    collection_name: &str,
    operation: DeleteVectors,
    shard_selection: Option<ShardId>,
    wait: bool,
    ordering: WriteOrdering,
    access: Access,
) -> Result<UpdateResult, StorageError> {
    let DeleteVectors {
        vector,
        filter,
        points,
        shard_key,
    } = operation;

    let vector_names: Vec<_> = vector.into_iter().collect();
    let mut result = None;
    let shard_selector = get_shard_selector_for_update(shard_selection, shard_key);

    if let Some(filter) = filter {
        let hw_acc = HwMeasurementAcc::disposable();
        let vectors_operation =
            VectorOperations::DeleteVectorsByFilter(filter, vector_names.clone());
        let collection_operation = CollectionUpdateOperations::VectorOperation(vectors_operation);
        result = Some(
            toc.update(
                collection_name,
                collection_operation.into(),
                wait,
                ordering,
                shard_selector.clone(),
                access.clone(),
                hw_acc,
            )
            .await?,
        );
    }

    if let Some(points) = points {
        let hw_acc = HwMeasurementAcc::disposable();
        let vectors_operation = VectorOperations::DeleteVectors(points.into(), vector_names);
        let collection_operation = CollectionUpdateOperations::VectorOperation(vectors_operation);
        result = Some(
            toc.update(
                collection_name,
                collection_operation.into(),
                wait,
                ordering,
                shard_selector,
                access,
                hw_acc,
            )
            .await?,
        );
    }

    result.ok_or_else(|| StorageError::bad_request("No filter or points provided"))
}

async fn do_set_payload(
    toc: &TableOfContent,
    collection_name: &str,
    operation: SetPayload,
    shard_selection: Option<ShardId>,
    wait: bool,
    ordering: WriteOrdering,
    access: Access,
) -> Result<UpdateResult, StorageError> {
    let hw_acc = HwMeasurementAcc::disposable();
    let SetPayload {
        points,
        payload,
        filter,
        shard_key,
        key,
    } = operation;

    let collection_operation =
        CollectionUpdateOperations::PayloadOperation(PayloadOps::SetPayload(SetPayloadOp {
            payload,
            points,
            filter,
            key,
        }));

    let shard_selector = get_shard_selector_for_update(shard_selection, shard_key);

    toc.update(
        collection_name,
        collection_operation.into(),
        wait,
        ordering,
        shard_selector,
        access,
        hw_acc,
    )
    .await
}

async fn do_overwrite_payload(
    toc: &TableOfContent,
    collection_name: &str,
    operation: SetPayload,
    shard_selection: Option<ShardId>,
    wait: bool,
    ordering: WriteOrdering,
    access: Access,
) -> Result<UpdateResult, StorageError> {
    let hw_acc = HwMeasurementAcc::disposable();
    let SetPayload {
        points,
        payload,
        filter,
        shard_key,
        key,
    } = operation;

    let collection_operation =
        CollectionUpdateOperations::PayloadOperation(PayloadOps::OverwritePayload(SetPayloadOp {
            payload,
            points,
            filter,
            key,
        }));

    let shard_selector = get_shard_selector_for_update(shard_selection, shard_key);

    toc.update(
        collection_name,
        collection_operation.into(),
        wait,
        ordering,
        shard_selector,
        access,
        hw_acc,
    )
    .await
}

async fn do_delete_payload(
    toc: &TableOfContent,
    collection_name: &str,
    operation: DeletePayload,
    shard_selection: Option<ShardId>,
    wait: bool,
    ordering: WriteOrdering,
    access: Access,
) -> Result<UpdateResult, StorageError> {
    let hw_acc = HwMeasurementAcc::disposable();
    let DeletePayload {
        keys,
        points,
        filter,
        shard_key,
    } = operation;

    let collection_operation =
        CollectionUpdateOperations::PayloadOperation(PayloadOps::DeletePayload(DeletePayloadOp {
            keys,
            points,
            filter,
        }));

    let shard_selector = get_shard_selector_for_update(shard_selection, shard_key);

    toc.update(
        collection_name,
        collection_operation.into(),
        wait,
        ordering,
        shard_selector,
        access,
        hw_acc,
    )
    .await
}

async fn do_clear_payload(
    toc: &TableOfContent,
    collection_name: &str,
    points: PointsSelector,
    shard_selection: Option<ShardId>,
    wait: bool,
    ordering: WriteOrdering,
    access: Access,
) -> Result<UpdateResult, StorageError> {
    let hw_acc = HwMeasurementAcc::disposable();
    let (point_operation, shard_key) = match points {
        PointsSelector::PointIdsSelector(PointIdsList { points, shard_key }) => {
            (PayloadOps::ClearPayload { points }, shard_key)
        }
        PointsSelector::FilterSelector(FilterSelector { filter, shard_key }) => {
            (PayloadOps::ClearPayloadByFilter(filter), shard_key)
        }
    };

    let collection_operation = CollectionUpdateOperations::PayloadOperation(point_operation);
    let shard_selector = get_shard_selector_for_update(shard_selection, shard_key);

    toc.update(
        collection_name,
        collection_operation.into(),
        wait,
        ordering,
        shard_selector,
        access,
        hw_acc,
    )
    .await
}

fn get_shard_selector_for_update(
    shard_selection: Option<ShardId>,
    shard_key: Option<ShardKeySelector>,
) -> ShardSelectorInternal {
    match (shard_selection, shard_key) {
        (Some(shard_selection), None) => ShardSelectorInternal::ShardId(shard_selection),
        (Some(shard_selection), Some(_)) => {
            debug_assert!(
                false,
                "Shard selection and shard key are mutually exclusive"
            );
            ShardSelectorInternal::ShardId(shard_selection)
        }
        (None, Some(shard_key)) => ShardSelectorInternal::from(shard_key),
        (None, None) => ShardSelectorInternal::Empty,
    }
}
