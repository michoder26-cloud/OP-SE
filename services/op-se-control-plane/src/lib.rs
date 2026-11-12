use std::{
    path::Path,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

use axum::{
    extract::{Path as RoutePath, State},
    http::{header::AUTHORIZATION, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use subtle::ConstantTimeEq;
use uuid::Uuid;

#[derive(Clone)]
pub struct AppState {
    database: Arc<Mutex<Connection>>,
    admin_token: Arc<String>,
}

impl AppState {
    pub fn open(path: impl AsRef<Path>, admin_token: String) -> rusqlite::Result<Self> {
        let connection = Connection::open(path)?;
        connection.execute_batch(
            "PRAGMA foreign_keys = ON;
             CREATE TABLE IF NOT EXISTS organizations (
               id TEXT PRIMARY KEY,
               slug TEXT NOT NULL UNIQUE,
               name TEXT NOT NULL,
               created_at INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS members (
               organization_id TEXT NOT NULL REFERENCES organizations(id),
               email TEXT NOT NULL,
               role TEXT NOT NULL CHECK (role IN ('admin', 'member')),
               PRIMARY KEY (organization_id, email)
             );
             CREATE TABLE IF NOT EXISTS usage_events (
               organization_id TEXT NOT NULL REFERENCES organizations(id),
               event_id TEXT NOT NULL,
               meter TEXT NOT NULL,
               quantity INTEGER NOT NULL CHECK (quantity > 0),
               occurred_at INTEGER NOT NULL,
               PRIMARY KEY (organization_id, event_id)
             );",
        )?;
        Ok(Self {
            database: Arc::new(Mutex::new(connection)),
            admin_token: Arc::new(admin_token),
        })
    }
}

pub fn app(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/v1/self-host/entitlements", get(entitlements))
        .route("/v1/organizations", post(create_organization))
        .route("/v1/organizations/{id}", get(get_organization))
        .route(
            "/v1/organizations/{id}/members",
            post(upsert_member).get(list_members),
        )
        .route("/v1/organizations/{id}/usage-events", post(record_usage))
        .route("/v1/organizations/{id}/usage-summary", get(usage_summary))
        .with_state(state)
}

#[derive(Debug)]
enum ApiError {
    Unauthorized,
    BadRequest,
    NotFound,
    Conflict,
    Internal,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, code) = match self {
            Self::Unauthorized => (StatusCode::UNAUTHORIZED, "unauthorized"),
            Self::BadRequest => (StatusCode::BAD_REQUEST, "bad_request"),
            Self::NotFound => (StatusCode::NOT_FOUND, "not_found"),
            Self::Conflict => (StatusCode::CONFLICT, "conflict"),
            Self::Internal => (StatusCode::INTERNAL_SERVER_ERROR, "internal_error"),
        };
        (status, Json(serde_json::json!({ "error": code }))).into_response()
    }
}

fn require_admin(state: &AppState, headers: &HeaderMap) -> Result<(), ApiError> {
    let supplied = headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .ok_or(ApiError::Unauthorized)?;
    if bool::from(supplied.as_bytes().ct_eq(state.admin_token.as_bytes())) {
        Ok(())
    } else {
        Err(ApiError::Unauthorized)
    }
}

fn now_seconds() -> Result<i64, ApiError> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| ApiError::Internal)?;
    i64::try_from(duration.as_secs()).map_err(|_| ApiError::Internal)
}

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "status": "ok" }))
}

async fn entitlements() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "mode": "self_hosted",
        "billing_required": false,
        "features": ["ide", "ai_byok", "local_models"]
    }))
}

#[derive(Deserialize)]
struct NewOrganization {
    slug: String,
    name: String,
}

#[derive(Serialize)]
struct Organization {
    id: String,
    slug: String,
    name: String,
    created_at: i64,
}

async fn create_organization(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<NewOrganization>,
) -> Result<(StatusCode, Json<Organization>), ApiError> {
    require_admin(&state, &headers)?;
    if input.slug.len() < 3
        || input.slug.len() > 40
        || input.slug.starts_with('-')
        || input.slug.ends_with('-')
        || !input
            .slug
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        || input.name.trim().is_empty()
        || input.name.len() > 120
    {
        return Err(ApiError::BadRequest);
    }
    let organization = Organization {
        id: Uuid::new_v4().to_string(),
        slug: input.slug,
        name: input.name.trim().to_owned(),
        created_at: now_seconds()?,
    };
    let database = state.database.lock().map_err(|_| ApiError::Internal)?;
    match database.execute(
        "INSERT INTO organizations (id, slug, name, created_at) VALUES (?1, ?2, ?3, ?4)",
        params![
            organization.id,
            organization.slug,
            organization.name,
            organization.created_at
        ],
    ) {
        Ok(_) => Ok((StatusCode::CREATED, Json(organization))),
        Err(error)
            if error.sqlite_error_code() == Some(rusqlite::ErrorCode::ConstraintViolation) =>
        {
            Err(ApiError::Conflict)
        }
        Err(_) => Err(ApiError::Internal),
    }
}

async fn get_organization(
    State(state): State<AppState>,
    headers: HeaderMap,
    RoutePath(id): RoutePath<String>,
) -> Result<Json<Organization>, ApiError> {
    require_admin(&state, &headers)?;
    Uuid::parse_str(&id).map_err(|_| ApiError::BadRequest)?;
    let database = state.database.lock().map_err(|_| ApiError::Internal)?;
    let result = database
        .query_row(
            "SELECT id, slug, name, created_at FROM organizations WHERE id = ?1",
            [id],
            |row| {
                Ok(Organization {
                    id: row.get(0)?,
                    slug: row.get(1)?,
                    name: row.get(2)?,
                    created_at: row.get(3)?,
                })
            },
        )
        .optional()
        .map_err(|_| ApiError::Internal)?;
    result.map(Json).ok_or(ApiError::NotFound)
}

fn organization_exists(database: &Connection, id: &str) -> Result<(), ApiError> {
    Uuid::parse_str(id).map_err(|_| ApiError::BadRequest)?;
    let exists: i64 = database
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM organizations WHERE id = ?1)",
            [id],
            |row| row.get(0),
        )
        .map_err(|_| ApiError::Internal)?;
    if exists == 1 {
        Ok(())
    } else {
        Err(ApiError::NotFound)
    }
}

#[derive(Deserialize)]
struct NewMember {
    email: String,
    role: String,
}

#[derive(Serialize)]
struct Member {
    email: String,
    role: String,
}

async fn upsert_member(
    State(state): State<AppState>,
    headers: HeaderMap,
    RoutePath(id): RoutePath<String>,
    Json(input): Json<NewMember>,
) -> Result<Json<Member>, ApiError> {
    require_admin(&state, &headers)?;
    let email = input.email.trim().to_ascii_lowercase();
    if email.len() > 320
        || !email.contains('@')
        || email.bytes().any(|byte| byte.is_ascii_whitespace())
        || !matches!(input.role.as_str(), "admin" | "member")
    {
        return Err(ApiError::BadRequest);
    }
    let database = state.database.lock().map_err(|_| ApiError::Internal)?;
    organization_exists(&database, &id)?;
    database
        .execute(
            "INSERT INTO members (organization_id, email, role) VALUES (?1, ?2, ?3)
         ON CONFLICT(organization_id, email) DO UPDATE SET role = excluded.role",
            params![id, email, input.role],
        )
        .map_err(|_| ApiError::Internal)?;
    Ok(Json(Member {
        email,
        role: input.role,
    }))
}

async fn list_members(
    State(state): State<AppState>,
    headers: HeaderMap,
    RoutePath(id): RoutePath<String>,
) -> Result<Json<Vec<Member>>, ApiError> {
    require_admin(&state, &headers)?;
    let database = state.database.lock().map_err(|_| ApiError::Internal)?;
    organization_exists(&database, &id)?;
    let mut statement = database
        .prepare("SELECT email, role FROM members WHERE organization_id = ?1 ORDER BY email")
        .map_err(|_| ApiError::Internal)?;
    let members = statement
        .query_map([id], |row| {
            Ok(Member {
                email: row.get(0)?,
                role: row.get(1)?,
            })
        })
        .map_err(|_| ApiError::Internal)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| ApiError::Internal)?;
    Ok(Json(members))
}

#[derive(Deserialize)]
struct NewUsageEvent {
    event_id: String,
    meter: String,
    quantity: u64,
}

#[derive(Serialize)]
struct UsageReceipt {
    event_id: String,
    status: &'static str,
}

async fn record_usage(
    State(state): State<AppState>,
    headers: HeaderMap,
    RoutePath(id): RoutePath<String>,
    Json(input): Json<NewUsageEvent>,
) -> Result<(StatusCode, Json<UsageReceipt>), ApiError> {
    require_admin(&state, &headers)?;
    Uuid::parse_str(&input.event_id).map_err(|_| ApiError::BadRequest)?;
    if !matches!(
        input.meter.as_str(),
        "model_input_tokens"
            | "model_output_tokens"
            | "hosted_compute_seconds"
            | "storage_byte_hours"
    ) || input.quantity == 0
        || input.quantity > i64::MAX as u64
    {
        return Err(ApiError::BadRequest);
    }
    let database = state.database.lock().map_err(|_| ApiError::Internal)?;
    organization_exists(&database, &id)?;
    let existing: Option<(String, i64)> = database
        .query_row(
            "SELECT meter, quantity FROM usage_events WHERE organization_id = ?1 AND event_id = ?2",
            params![id, input.event_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|_| ApiError::Internal)?;
    if let Some((meter, quantity)) = existing {
        if meter == input.meter && quantity == input.quantity as i64 {
            return Ok((
                StatusCode::OK,
                Json(UsageReceipt {
                    event_id: input.event_id,
                    status: "duplicate",
                }),
            ));
        }
        return Err(ApiError::Conflict);
    }
    database
        .execute(
            "INSERT INTO usage_events (organization_id, event_id, meter, quantity, occurred_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                id,
                input.event_id,
                input.meter,
                input.quantity as i64,
                now_seconds()?
            ],
        )
        .map_err(|_| ApiError::Internal)?;
    Ok((
        StatusCode::CREATED,
        Json(UsageReceipt {
            event_id: input.event_id,
            status: "recorded",
        }),
    ))
}

#[derive(Serialize)]
struct MeterTotal {
    meter: String,
    quantity: i64,
}

async fn usage_summary(
    State(state): State<AppState>,
    headers: HeaderMap,
    RoutePath(id): RoutePath<String>,
) -> Result<Json<Vec<MeterTotal>>, ApiError> {
    require_admin(&state, &headers)?;
    let database = state.database.lock().map_err(|_| ApiError::Internal)?;
    organization_exists(&database, &id)?;
    let mut statement = database.prepare(
        "SELECT meter, SUM(quantity) FROM usage_events WHERE organization_id = ?1 GROUP BY meter ORDER BY meter",
    ).map_err(|_| ApiError::Internal)?;
    let totals = statement
        .query_map([id], |row| {
            Ok(MeterTotal {
                meter: row.get(0)?,
                quantity: row.get(1)?,
            })
        })
        .map_err(|_| ApiError::Internal)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| ApiError::Internal)?;
    Ok(Json(totals))
}
