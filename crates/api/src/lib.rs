use std::sync::Arc;

use axum::{
    Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{delete, get, post},
    Json,
};
use include_dir::{Dir, include_dir};
use serde::{Deserialize, Serialize};
use tower_http::{cors::CorsLayer, trace::TraceLayer};

use superdupermemory_core::{Extractor, Fact};
use superdupermemory_embed::Embedder;
use superdupermemory_store::{App, Org, OrgStats, SqliteStore, UserWithCount};

static WEBAPP: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../../webapp/dist");

// ── shared state ───────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct ApiState {
    pub store: Arc<SqliteStore>,
    pub extractor: Arc<dyn Extractor>,
    pub embedder: Arc<dyn Embedder>,
}

// ── error helper ───────────────────────────────────────────────────────────

struct ApiError(anyhow::Error);

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": self.0.to_string() })),
        )
            .into_response()
    }
}

impl<E: Into<anyhow::Error>> From<E> for ApiError {
    fn from(e: E) -> Self {
        ApiError(e.into())
    }
}

type ApiResult<T> = Result<T, ApiError>;

// ── auth helpers ───────────────────────────────────────────────────────────

fn bearer_token(headers: &HeaderMap) -> Option<String> {
    headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|s| s.to_string())
}

fn admin_token(headers: &HeaderMap) -> Option<String> {
    headers
        .get("x-admin-token")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
}

// ── request / response types ───────────────────────────────────────────────

#[derive(Deserialize)]
struct CreateOrgReq {
    name: String,
    slug: String,
}

#[derive(Serialize)]
struct CreateOrgResp {
    org: Org,
    admin_token: String,
}

#[derive(Deserialize)]
struct CreateAppReq {
    name: String,
}

#[derive(Serialize)]
struct CreateAppResp {
    app: App,
    api_key: String,
}

#[derive(Serialize)]
struct ListAppsResp {
    apps: Vec<App>,
}

#[derive(Deserialize)]
struct RememberReq {
    text: String,
    user_id: String,
    source: Option<String>,
}

#[derive(Serialize)]
struct RememberResp {
    facts: Vec<Fact>,
}

#[derive(Deserialize)]
struct RecallQuery {
    user_id: String,
    q: Option<String>,
    limit: Option<usize>,
}

#[derive(Serialize)]
struct RecallResp {
    facts: Vec<Fact>,
}

#[derive(Deserialize)]
struct ForgetQuery {
    user_id: String,
}

#[derive(Serialize)]
struct StatsResp {
    stats: OrgStats,
}

#[derive(Serialize)]
struct UsersResp {
    users: Vec<UserWithCount>,
}

// ── route handlers ─────────────────────────────────────────────────────────

async fn health() -> impl IntoResponse {
    Json(serde_json::json!({ "status": "ok", "version": env!("CARGO_PKG_VERSION") }))
}

async fn create_org(
    State(s): State<ApiState>,
    Json(body): Json<CreateOrgReq>,
) -> ApiResult<impl IntoResponse> {
    let (org, admin_token) = s.store.create_org(&body.name, &body.slug).await?;
    Ok((StatusCode::CREATED, Json(CreateOrgResp { org, admin_token })))
}

async fn list_apps(
    State(s): State<ApiState>,
    Path(org_id): Path<String>,
    headers: HeaderMap,
) -> ApiResult<impl IntoResponse> {
    let token = admin_token(&headers)
        .ok_or_else(|| anyhow::anyhow!("missing X-Admin-Token"))?;
    let org = s.store.get_org_by_token(&token).await?
        .ok_or_else(|| anyhow::anyhow!("invalid admin token"))?;
    if org.id != org_id {
        return Err(anyhow::anyhow!("forbidden").into());
    }
    let apps = s.store.list_apps_for_org(&org_id).await?;
    Ok(Json(ListAppsResp { apps }))
}

async fn create_app(
    State(s): State<ApiState>,
    Path(org_id): Path<String>,
    headers: HeaderMap,
    Json(body): Json<CreateAppReq>,
) -> ApiResult<impl IntoResponse> {
    let token = admin_token(&headers)
        .ok_or_else(|| anyhow::anyhow!("missing X-Admin-Token"))?;
    let org = s.store.get_org_by_token(&token).await?
        .ok_or_else(|| anyhow::anyhow!("invalid admin token"))?;
    if org.id != org_id {
        return Err(anyhow::anyhow!("forbidden").into());
    }
    let (app, api_key) = s.store.create_app(&org_id, &body.name).await?;
    Ok((StatusCode::CREATED, Json(CreateAppResp { app, api_key })))
}

async fn org_stats(
    State(s): State<ApiState>,
    Path(org_id): Path<String>,
    headers: HeaderMap,
) -> ApiResult<impl IntoResponse> {
    let token = admin_token(&headers)
        .ok_or_else(|| anyhow::anyhow!("missing X-Admin-Token"))?;
    let org = s.store.get_org_by_token(&token).await?
        .ok_or_else(|| anyhow::anyhow!("invalid admin token"))?;
    if org.id != org_id {
        return Err(anyhow::anyhow!("forbidden").into());
    }
    let stats = s.store.org_stats(&org_id).await?;
    Ok(Json(StatsResp { stats }))
}

async fn list_app_users(
    State(s): State<ApiState>,
    Path(app_id): Path<String>,
    headers: HeaderMap,
) -> ApiResult<impl IntoResponse> {
    let key = bearer_token(&headers)
        .ok_or_else(|| anyhow::anyhow!("missing Authorization Bearer token"))?;
    let app = s.store.get_app_by_key(&key).await?
        .ok_or_else(|| anyhow::anyhow!("invalid api key"))?;
    if app.id != app_id {
        return Err(anyhow::anyhow!("forbidden").into());
    }
    let users = s.store.list_users_for_app(&app_id).await?;
    Ok(Json(UsersResp { users }))
}

async fn remember(
    State(s): State<ApiState>,
    headers: HeaderMap,
    Json(body): Json<RememberReq>,
) -> ApiResult<impl IntoResponse> {
    let key = bearer_token(&headers)
        .ok_or_else(|| anyhow::anyhow!("missing Authorization Bearer token"))?;
    let app = s.store.get_app_by_key(&key).await?
        .ok_or_else(|| anyhow::anyhow!("invalid api key"))?;

    let app_user = s.store.get_or_create_app_user(&app.id, &body.user_id).await?;
    let source = body.source.unwrap_or_else(|| "api".to_string());

    let mut facts = s.extractor.extract(&body.text, &source).await
        .unwrap_or_default();
    if facts.is_empty() {
        facts.push(superdupermemory_core::Fact::new("raw", &body.text, &source));
    }

    for fact in &facts {
        let embedding = s.embedder.embed(&fact.body).await.ok();
        s.store.save_for_user(fact, embedding.as_deref(), &app_user.id).await?;
    }

    Ok((StatusCode::CREATED, Json(RememberResp { facts })))
}

async fn recall(
    State(s): State<ApiState>,
    headers: HeaderMap,
    Query(q): Query<RecallQuery>,
) -> ApiResult<impl IntoResponse> {
    let key = bearer_token(&headers)
        .ok_or_else(|| anyhow::anyhow!("missing Authorization Bearer token"))?;
    let app = s.store.get_app_by_key(&key).await?
        .ok_or_else(|| anyhow::anyhow!("invalid api key"))?;

    let app_user = s.store.get_or_create_app_user(&app.id, &q.user_id).await?;
    let limit = q.limit.unwrap_or(10).min(100);

    let facts = if let Some(query_text) = q.q.filter(|s| !s.is_empty()) {
        let embedding = s.embedder.embed(&query_text).await?;
        s.store
            .search_blended_for_user(&query_text, &embedding, limit, &app_user.id)
            .await?
    } else {
        s.store.list_for_user(limit, &app_user.id).await?
    };

    Ok(Json(RecallResp { facts }))
}

async fn forget(
    State(s): State<ApiState>,
    Path(fact_id): Path<String>,
    headers: HeaderMap,
    Query(q): Query<ForgetQuery>,
) -> ApiResult<impl IntoResponse> {
    let key = bearer_token(&headers)
        .ok_or_else(|| anyhow::anyhow!("missing Authorization Bearer token"))?;
    let app = s.store.get_app_by_key(&key).await?
        .ok_or_else(|| anyhow::anyhow!("invalid api key"))?;

    let app_user = s.store.get_or_create_app_user(&app.id, &q.user_id).await?;
    let deleted = s.store.delete_for_user(&fact_id, &app_user.id).await?;

    Ok(Json(serde_json::json!({ "deleted": deleted })))
}

// ── static file serving ────────────────────────────────────────────────────

async fn serve_static(uri: axum::http::Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };

    if let Some(file) = WEBAPP.get_file(path) {
        let mime = mime_guess::from_path(path).first_or_octet_stream();
        (
            [(header::CONTENT_TYPE, mime.as_ref())],
            file.contents(),
        )
            .into_response()
    } else {
        // SPA fallback
        match WEBAPP.get_file("index.html") {
            Some(file) => (
                [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
                file.contents(),
            )
                .into_response(),
            None => StatusCode::NOT_FOUND.into_response(),
        }
    }
}

// ── router ─────────────────────────────────────────────────────────────────

pub fn router(state: ApiState) -> Router {
    Router::new()
        .route("/api/v1/health", get(health))
        .route("/api/v1/orgs", post(create_org))
        .route("/api/v1/orgs/:org_id/apps", get(list_apps).post(create_app))
        .route("/api/v1/orgs/:org_id/stats", get(org_stats))
        .route("/api/v1/apps/:app_id/users", get(list_app_users))
        .route("/api/v1/memories", post(remember).get(recall))
        .route("/api/v1/memories/:id", delete(forget))
        .fallback(serve_static)
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        .with_state(state)
}

pub async fn serve(state: ApiState, port: u16) -> anyhow::Result<()> {
    let app = router(state);
    let addr = format!("0.0.0.0:{port}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("listening on http://{addr}");
    axum::serve(listener, app).await?;
    Ok(())
}
