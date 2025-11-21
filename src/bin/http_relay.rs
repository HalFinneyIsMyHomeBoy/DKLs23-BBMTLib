//! HTTP Relay Server for Distributed DKG
//!
//! This server acts as a message relay between parties participating in distributed DKG.
//! Parties can POST messages to the relay and GET messages intended for them.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::Arc,
};
use tokio::sync::RwLock;
use tower_http::cors::CorsLayer;

/// Message storage structure
#[derive(Clone, Debug, Serialize, Deserialize)]
struct StoredMessage {
    phase: u8,
    sender: u8,
    receiver: u8,
    data: serde_json::Value,
}

/// Server state
#[derive(Clone)]
struct AppState {
    messages: Arc<RwLock<HashMap<String, Vec<StoredMessage>>>>,
}

/// Request to post a message
#[derive(Deserialize)]
struct PostMessageRequest {
    data: serde_json::Value,
}

/// Response for getting messages
#[derive(Serialize)]
struct GetMessagesResponse {
    messages: Vec<StoredMessage>,
}

#[tokio::main]
async fn main() {
    let addr = "127.0.0.1:8080";
    println!("🚀 Starting HTTP Relay Server on http://{}", addr);
    println!("📡 Ready to relay messages between parties\n");

    let app_state = AppState {
        messages: Arc::new(RwLock::new(HashMap::new())),
    };

    let app = Router::new()
        .route("/health", get(health_check))
        .route("/message/:phase/:sender/:receiver", post(post_message))
        .route("/messages/:phase/:receiver", get(get_messages))
        .route("/clear", post(clear_messages))
        .layer(CorsLayer::permissive())
        .with_state(app_state);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    println!("✅ Server listening on http://{}\n", addr);
    axum::serve(listener, app).await.unwrap();
}

async fn health_check() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "status": "ok" }))
}

async fn post_message(
    Path((phase, sender, receiver)): Path<(u8, u8, u8)>,
    State(state): State<AppState>,
    Json(payload): Json<PostMessageRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let message = StoredMessage {
        phase,
        sender,
        receiver,
        data: payload.data,
    };

    let key = format!("{}/{}", phase, receiver);
    let mut messages = state.messages.write().await;
    messages.entry(key).or_insert_with(Vec::new).push(message);

    println!("📨 Received message: Phase {} from Party {} to Party {}", phase, sender, receiver);

    Ok(Json(serde_json::json!({
        "status": "ok",
        "message": "Message stored"
    })))
}

async fn get_messages(
    Path((phase, receiver)): Path<(u8, u8)>,
    State(state): State<AppState>,
) -> Json<GetMessagesResponse> {
    let key = format!("{}/{}", phase, receiver);
    let messages = state.messages.read().await;
    let messages_for_receiver = messages
        .get(&key)
        .cloned()
        .unwrap_or_default();

    println!("📬 Retrieved {} messages for Party {} in Phase {}", messages_for_receiver.len(), receiver, phase);

    Json(GetMessagesResponse {
        messages: messages_for_receiver,
    })
}

async fn clear_messages(State(state): State<AppState>) -> Json<serde_json::Value> {
    let mut messages = state.messages.write().await;
    messages.clear();
    println!("🗑️  Cleared all messages");
    Json(serde_json::json!({ "status": "ok", "message": "All messages cleared" }))
}

