use axum::{
    body::{to_bytes, Body},
    http::{Method, Request, StatusCode},
    Router,
};
use op_se_control_plane::{app, AppState};
use serde_json::{json, Value};
use tempfile::tempdir;
use tower::ServiceExt;

const TOKEN: &str = "test-token-with-at-least-32-chars-0001";

async fn request(
    router: &Router,
    method: Method,
    path: &str,
    body: Value,
    authorized: bool,
) -> (StatusCode, Value) {
    let mut builder = Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/json");
    if authorized {
        builder = builder.header("authorization", format!("Bearer {TOKEN}"));
    }
    let response = router
        .clone()
        .oneshot(builder.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}

#[tokio::test]
async fn self_hosted_entitlements_do_not_require_an_account() {
    let directory = tempdir().unwrap();
    let router =
        app(AppState::open(directory.path().join("control.db"), TOKEN.to_owned()).unwrap());
    let (status, body) = request(
        &router,
        Method::GET,
        "/v1/self-host/entitlements",
        json!({}),
        false,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["billing_required"], false);
    assert_eq!(body["mode"], "self_hosted");
}

#[tokio::test]
async fn organization_membership_and_usage_are_authenticated_idempotent_and_persistent() {
    let directory = tempdir().unwrap();
    let database = directory.path().join("control.db");
    let router = app(AppState::open(&database, TOKEN.to_owned()).unwrap());

    let (status, _) = request(
        &router,
        Method::POST,
        "/v1/organizations",
        json!({"slug":"demo-team","name":"Demo Team"}),
        false,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, organization) = request(
        &router,
        Method::POST,
        "/v1/organizations",
        json!({"slug":"demo-team","name":"Demo Team"}),
        true,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let id = organization["id"].as_str().unwrap();

    let (status, _) = request(
        &router,
        Method::POST,
        "/v1/organizations",
        json!({"slug":"demo-team","name":"Duplicate"}),
        true,
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    let (status, _) = request(
        &router,
        Method::POST,
        "/v1/organizations",
        json!({"slug":"../bad","name":"Invalid"}),
        true,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let members_path = format!("/v1/organizations/{id}/members");
    let (status, member) = request(
        &router,
        Method::POST,
        &members_path,
        json!({"email":"ADMIN@example.com","role":"admin"}),
        true,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(member["email"], "admin@example.com");
    let (status, members) = request(&router, Method::GET, &members_path, json!({}), true).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(members.as_array().unwrap().len(), 1);

    let usage_path = format!("/v1/organizations/{id}/usage-events");
    let event = json!({"event_id":"11111111-1111-4111-8111-111111111111","meter":"hosted_compute_seconds","quantity":7});
    let (status, receipt) = request(&router, Method::POST, &usage_path, event.clone(), true).await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(receipt["status"], "recorded");
    let (status, receipt) = request(&router, Method::POST, &usage_path, event.clone(), true).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(receipt["status"], "duplicate");
    let (status, _) = request(&router, Method::POST, &usage_path, json!({"event_id":"11111111-1111-4111-8111-111111111111","meter":"hosted_compute_seconds","quantity":8}), true).await;
    assert_eq!(status, StatusCode::CONFLICT);

    drop(router);
    let reopened = app(AppState::open(&database, TOKEN.to_owned()).unwrap());
    let (status, totals) = request(
        &reopened,
        Method::GET,
        &format!("/v1/organizations/{id}/usage-summary"),
        json!({}),
        true,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(totals[0]["meter"], "hosted_compute_seconds");
    assert_eq!(totals[0]["quantity"], 7);
}
