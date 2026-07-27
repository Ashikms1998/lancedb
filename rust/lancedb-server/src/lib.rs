// SPDX-License-Identifier: Apache-2.0
// SPDX-FileCopyrightText: Copyright The LanceDB Authors

use std::sync::Arc;

use arrow_array::types::Float32Type;
use arrow_array::{Array, FixedSizeListArray, Float32Array, RecordBatch, StringArray};
use arrow_schema::{DataType, Field, Schema};
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use futures::TryStreamExt;
use lancedb::connection::Connection;
use lancedb::query::{ExecutableQuery, QueryBase};
use serde::{Deserialize, Serialize};

#[derive(Clone)]
pub struct AppState {
    db: Connection,
}

impl AppState {
    pub async fn connect(data_dir: &str) -> Result<Self, ApiError> {
        let db = lancedb::connect(data_dir).execute().await?;
        Ok(Self { db })
    }
}

pub fn app(state: AppState) -> Router {
    Router::new()
        .route("/v1/ping", get(ping))
        .route("/v1/db/health", get(db_health))
        .route("/v1/tables", get(list_tables).post(create_table))
        .route("/v1/tables/{table}", axum::routing::delete(drop_table))
        .route("/v1/tables/{table}/rows", get(get_rows).post(insert_rows))
        .route(
            "/v1/tables/{table}/rows/{id}",
            axum::routing::patch(update_row).delete(delete_row),
        )
        .route("/v1/tables/{table}/count", get(count_rows))
        .route("/v1/tables/{table}/search", axum::routing::post(search))
        .with_state(state)
}

#[derive(Debug, Serialize)]
struct Envelope<T> {
    ok: bool,
    data: T,
}

fn success<T>(data: T) -> Json<Envelope<T>> {
    Json(Envelope { ok: true, data })
}

async fn ping() -> impl IntoResponse {
    success(PingResponse { message: "pong" })
}

#[derive(Debug, Serialize)]
struct PingResponse {
    message: &'static str,
}

async fn db_health(State(state): State<AppState>) -> Result<impl IntoResponse, ApiError> {
    state.db.table_names().execute().await?;
    Ok(success(HealthResponse {
        status: "ok",
        database: "connected",
    }))
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    status: &'static str,
    database: &'static str,
}

async fn list_tables(State(state): State<AppState>) -> Result<impl IntoResponse, ApiError> {
    let tables = state.db.table_names().execute().await?;
    Ok(success(TablesResponse { tables }))
}

#[derive(Debug, Serialize)]
struct TablesResponse {
    tables: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct CreateTableRequest {
    name: String,
    vector_dimension: usize,
}

async fn create_table(
    State(state): State<AppState>,
    Json(request): Json<CreateTableRequest>,
) -> Result<impl IntoResponse, ApiError> {
    validate_table_name(&request.name)?;
    if request.vector_dimension == 0 || request.vector_dimension > i32::MAX as usize {
        return Err(ApiError::bad_request("INVALID_VECTOR_DIMENSION"));
    }
    let schema = document_schema(request.vector_dimension);
    state
        .db
        .create_empty_table(&request.name, schema)
        .execute()
        .await?;
    Ok((
        StatusCode::CREATED,
        success(TableResponse { name: request.name }),
    ))
}

#[derive(Debug, Serialize)]
struct TableResponse {
    name: String,
}

async fn drop_table(
    State(state): State<AppState>,
    Path(table): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    validate_table_name(&table)?;
    state.db.drop_table(&table, &[]).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Document {
    pub id: String,
    pub text: String,
    pub vector: Vec<f32>,
}

#[derive(Debug, Deserialize)]
struct InsertRequest {
    rows: Vec<Document>,
}

async fn insert_rows(
    State(state): State<AppState>,
    Path(table): Path<String>,
    Json(request): Json<InsertRequest>,
) -> Result<impl IntoResponse, ApiError> {
    validate_table_name(&table)?;
    if request.rows.is_empty() {
        return Err(ApiError::bad_request("ROWS_REQUIRED"));
    }
    let table = state.db.open_table(&table).execute().await?;
    let dimension = vector_dimension(table.schema().await?.as_ref())?;
    table
        .add(documents_to_batch(&request.rows, dimension)?)
        .execute()
        .await?;
    Ok((
        StatusCode::CREATED,
        success(CountResponse {
            count: request.rows.len(),
        }),
    ))
}

#[derive(Debug, Serialize)]
struct CountResponse {
    count: usize,
}

#[derive(Debug, Deserialize)]
struct RowsQuery {
    limit: Option<usize>,
}

async fn get_rows(
    State(state): State<AppState>,
    Path(table): Path<String>,
    Query(query): Query<RowsQuery>,
) -> Result<impl IntoResponse, ApiError> {
    validate_table_name(&table)?;
    let table = state.db.open_table(&table).execute().await?;
    let batches = table
        .query()
        .limit(query.limit.unwrap_or(100).min(1000))
        .execute()
        .await?
        .try_collect::<Vec<_>>()
        .await?;
    Ok(success(RowsResponse {
        rows: batches_to_documents(&batches)?,
    }))
}

#[derive(Debug, Serialize)]
struct RowsResponse {
    rows: Vec<Document>,
}

async fn count_rows(
    State(state): State<AppState>,
    Path(table): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    validate_table_name(&table)?;
    let table = state.db.open_table(&table).execute().await?;
    Ok(success(CountResponse {
        count: table.count_rows(None).await?,
    }))
}

#[derive(Debug, Deserialize)]
struct UpdateRequest {
    text: String,
}

async fn update_row(
    State(state): State<AppState>,
    Path((table, id)): Path<(String, String)>,
    Json(request): Json<UpdateRequest>,
) -> Result<impl IntoResponse, ApiError> {
    validate_table_name(&table)?;
    let table = state.db.open_table(&table).execute().await?;
    let result = table
        .update()
        .only_if(format!("id = {}", sql_string(&id)))
        .column("text", sql_string(&request.text))
        .execute()
        .await?;
    Ok(success(CountResponse {
        count: result.rows_updated as usize,
    }))
}

async fn delete_row(
    State(state): State<AppState>,
    Path((table, id)): Path<(String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    validate_table_name(&table)?;
    let table = state.db.open_table(&table).execute().await?;
    let predicate = format!("id = {}", sql_string(&id));
    let result = table.delete(&predicate).await?;
    Ok(success(CountResponse {
        count: result.num_deleted_rows as usize,
    }))
}

#[derive(Debug, Deserialize)]
struct SearchRequest {
    vector: Vec<f32>,
    limit: Option<usize>,
}

#[derive(Debug, Serialize)]
struct SearchHit {
    id: String,
    text: String,
    vector: Vec<f32>,
    distance: f32,
}

async fn search(
    State(state): State<AppState>,
    Path(table): Path<String>,
    Json(request): Json<SearchRequest>,
) -> Result<impl IntoResponse, ApiError> {
    validate_table_name(&table)?;
    let table = state.db.open_table(&table).execute().await?;
    if request.vector.len() != vector_dimension(table.schema().await?.as_ref())? {
        return Err(ApiError::bad_request("VECTOR_DIMENSION_MISMATCH"));
    }
    let batches = table
        .query()
        .nearest_to(request.vector)?
        .limit(request.limit.unwrap_or(10).clamp(1, 100))
        .execute()
        .await?
        .try_collect::<Vec<_>>()
        .await?;
    Ok(success(SearchResponse {
        rows: batches_to_search_hits(&batches)?,
    }))
}

#[derive(Debug, Serialize)]
struct SearchResponse {
    rows: Vec<SearchHit>,
}

fn document_schema(dimension: usize) -> Arc<Schema> {
    Arc::new(Schema::new(vec![
        Field::new("id", DataType::Utf8, false),
        Field::new("text", DataType::Utf8, false),
        Field::new(
            "vector",
            DataType::FixedSizeList(
                Arc::new(Field::new("item", DataType::Float32, true)),
                dimension as i32,
            ),
            false,
        ),
    ]))
}

fn documents_to_batch(rows: &[Document], dimension: usize) -> Result<RecordBatch, ApiError> {
    if rows.iter().any(|row| row.vector.len() != dimension) {
        return Err(ApiError::bad_request("VECTOR_DIMENSION_MISMATCH"));
    }
    let vectors = FixedSizeListArray::from_iter_primitive::<Float32Type, _, _>(
        rows.iter()
            .map(|row| Some(row.vector.iter().copied().map(Some).collect::<Vec<_>>())),
        dimension as i32,
    );
    Ok(RecordBatch::try_new(
        document_schema(dimension),
        vec![
            Arc::new(StringArray::from_iter_values(
                rows.iter().map(|row| row.id.as_str()),
            )),
            Arc::new(StringArray::from_iter_values(
                rows.iter().map(|row| row.text.as_str()),
            )),
            Arc::new(vectors),
        ],
    )?)
}

fn vector_dimension(schema: &Schema) -> Result<usize, ApiError> {
    match schema.field_with_name("vector")?.data_type() {
        DataType::FixedSizeList(_, dimension) => Ok(*dimension as usize),
        _ => Err(ApiError::internal("INVALID_VECTOR_COLUMN")),
    }
}

fn batches_to_documents(batches: &[RecordBatch]) -> Result<Vec<Document>, ApiError> {
    let mut rows = Vec::new();
    for batch in batches {
        let ids = string_column(batch, "id")?;
        let texts = string_column(batch, "text")?;
        let vectors = vector_column(batch)?;
        for row in 0..batch.num_rows() {
            rows.push(Document {
                id: ids.value(row).to_owned(),
                text: texts.value(row).to_owned(),
                vector: vector_value(vectors, row)?,
            });
        }
    }
    Ok(rows)
}

fn batches_to_search_hits(batches: &[RecordBatch]) -> Result<Vec<SearchHit>, ApiError> {
    let mut rows = Vec::new();
    for batch in batches {
        let documents = batches_to_documents(std::slice::from_ref(batch))?;
        let distances = batch
            .column_by_name("_distance")
            .and_then(|column| column.as_any().downcast_ref::<Float32Array>())
            .ok_or_else(|| ApiError::internal("MISSING_DISTANCE_COLUMN"))?;
        for (row, document) in documents.into_iter().enumerate() {
            rows.push(SearchHit {
                id: document.id,
                text: document.text,
                vector: document.vector,
                distance: distances.value(row),
            });
        }
    }
    Ok(rows)
}

fn string_column<'a>(batch: &'a RecordBatch, name: &str) -> Result<&'a StringArray, ApiError> {
    batch
        .column_by_name(name)
        .and_then(|column| column.as_any().downcast_ref::<StringArray>())
        .ok_or_else(|| ApiError::internal("INVALID_STRING_COLUMN"))
}

fn vector_column(batch: &RecordBatch) -> Result<&FixedSizeListArray, ApiError> {
    batch
        .column_by_name("vector")
        .and_then(|column| column.as_any().downcast_ref::<FixedSizeListArray>())
        .ok_or_else(|| ApiError::internal("INVALID_VECTOR_COLUMN"))
}

fn vector_value(vectors: &FixedSizeListArray, row: usize) -> Result<Vec<f32>, ApiError> {
    let values = vectors.value(row);
    let values = values
        .as_any()
        .downcast_ref::<Float32Array>()
        .ok_or_else(|| ApiError::internal("INVALID_VECTOR_VALUES"))?;
    Ok(values.values().to_vec())
}

fn validate_table_name(name: &str) -> Result<(), ApiError> {
    if name.is_empty()
        || name.len() > 128
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
    {
        return Err(ApiError::bad_request("INVALID_TABLE_NAME"));
    }
    Ok(())
}

fn sql_string(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    code: &'static str,
    message: String,
}

impl ApiError {
    fn bad_request(code: &'static str) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            code,
            message: code.replace('_', " ").to_ascii_lowercase(),
        }
    }

    fn internal(code: &'static str) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            code,
            message: code.replace('_', " ").to_ascii_lowercase(),
        }
    }
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for ApiError {}

impl From<lancedb::Error> for ApiError {
    fn from(error: lancedb::Error) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            code: "LANCEDB_ERROR",
            message: error.to_string(),
        }
    }
}

impl From<arrow_schema::ArrowError> for ApiError {
    fn from(error: arrow_schema::ArrowError) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            code: "ARROW_ERROR",
            message: error.to_string(),
        }
    }
}

#[derive(Serialize)]
struct ErrorEnvelope {
    ok: bool,
    error: ErrorBody,
}

#[derive(Serialize)]
struct ErrorBody {
    code: &'static str,
    message: String,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(ErrorEnvelope {
                ok: false,
                error: ErrorBody {
                    code: self.code,
                    message: self.message,
                },
            }),
        )
            .into_response()
    }
}

#[cfg(test)]
mod tests {
    use axum::body::{Body, to_bytes};
    use axum::http::{Method, Request};
    use serde_json::{Value, json};
    use tower::ServiceExt;

    use super::{AppState, app};

    async fn call(router: &axum::Router, method: Method, uri: &str, body: Value) -> Value {
        let request = Request::builder()
            .method(method)
            .uri(uri)
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap();
        let response = router.clone().oneshot(request).await.unwrap();
        assert!(
            response.status().is_success(),
            "status: {}",
            response.status()
        );
        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        if body.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&body).unwrap()
        }
    }

    #[tokio::test]
    async fn typescript_api_round_trip() {
        let data = tempfile::tempdir().unwrap();
        let state = AppState::connect(data.path().to_str().unwrap())
            .await
            .unwrap();
        let router = app(state);

        let response = call(&router, Method::GET, "/v1/ping", Value::Null).await;
        assert_eq!(response["data"]["message"], "pong");

        call(
            &router,
            Method::POST,
            "/v1/tables",
            json!({"name": "documents", "vector_dimension": 4}),
        )
        .await;
        call(
            &router,
            Method::POST,
            "/v1/tables/documents/rows",
            json!({"rows": [
                {"id": "one", "text": "first", "vector": [1.0, 0.0, 0.0, 0.0]},
                {"id": "two", "text": "second", "vector": [0.0, 1.0, 0.0, 0.0]}
            ]}),
        )
        .await;

        let response = call(
            &router,
            Method::POST,
            "/v1/tables/documents/search",
            json!({"vector": [1.0, 0.0, 0.0, 0.0], "limit": 1}),
        )
        .await;
        assert_eq!(response["data"]["rows"][0]["id"], "one");

        let response = call(
            &router,
            Method::PATCH,
            "/v1/tables/documents/rows/one",
            json!({"text": "updated"}),
        )
        .await;
        assert_eq!(response["data"]["count"], 1);

        call(
            &router,
            Method::DELETE,
            "/v1/tables/documents/rows/two",
            Value::Null,
        )
        .await;
        let response = call(
            &router,
            Method::GET,
            "/v1/tables/documents/count",
            Value::Null,
        )
        .await;
        assert_eq!(response["data"]["count"], 1);
    }
}
