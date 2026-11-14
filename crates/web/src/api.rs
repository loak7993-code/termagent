use std::collections::HashMap;

use axum::extract::{Path, Query, Request, State as AxumState};
use axum::http::StatusCode;
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, patch, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use termagent_core::session::{self, Session};
use termagent_core::{Effort, Registry};

use crate::state::State;

pub fn router(state: State) -> Router<State> {
    Router::new()
        .route("/api/state", get(get_state))
        .route("/api/models", get(get_models))
        .route("/api/onboard", post(onboard))
        .route("/api/model", post(set_model))
        .route("/api/registry/refresh", post(refresh_registry))
        .route("/api/sessions", get(list_sessions).post(new_session))
        .route(
            "/api/sessions/{id}",
            patch(rename_session).delete(delete_session),
        )
        .route("/api/sessions/{id}/events", get(session_events))
        .route_layer(middleware::from_fn_with_state(state, auth))
}

async fn auth(AxumState(state): AxumState<State>, req: Request, next: Next) -> Response {
    let token = req
        .headers()
        .get("x-termagent-token")
        .and_then(|v| v.to_str().ok());
    if token != Some(state.token.as_str()) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "missing token"})),
        )
            .into_response();
    }
    next.run(req).await
}

fn bad(status: StatusCode, message: impl Into<String>) -> Response {
    (status, Json(json!({"error": message.into()}))).into_response()
}

async fn get_state(AxumState(state): AxumState<State>) -> Response {
    Json(state.snapshot().await).into_response()
}

async fn get_models(
    AxumState(state): AxumState<State>,
    Query(q): Query<HashMap<String, String>>,
) -> Response {
    let Some(id) = q.get("provider") else {
        return bad(
            StatusCode::BAD_REQUEST,
            "provider query parameter is required",
        );
    };
    let registry = state.registry.read().await;
    let Some(provider) = registry.provider(id) else {
        return bad(StatusCode::NOT_FOUND, format!("unknown provider {id}"));
    };
    Json(json!({
        "id": provider.id,
        "name": provider.name,
        "protocol": provider.protocol.label(),
        "env": provider.env,
        "api": provider.api,
        "models": provider.models,
    }))
    .into_response()
}

#[derive(Deserialize)]
struct OnboardBody {
    provider: String,
    model: String,
    #[serde(default)]
    api_key: String,
    #[serde(default)]
    effort: Option<Effort>,
    #[serde(default)]
    base_url: Option<String>,
}

async fn onboard(AxumState(state): AxumState<State>, Json(body): Json<OnboardBody>) -> Response {
    let registry = state.registry.read().await.clone();
    let Some(provider) = registry.provider(&body.provider) else {
        return bad(
            StatusCode::BAD_REQUEST,
            format!("unknown provider {}", body.provider),
        );
    };
    if !provider.models.iter().any(|m| m.id == body.model) {
        return bad(
            StatusCode::BAD_REQUEST,
            format!("unknown model {}", body.model),
        );
    }
    let mut cfg = state.cfg.read().await.clone();
    cfg.provider = body.provider.clone();
    cfg.model = body.model.clone();
    cfg.effort = body.effort.unwrap_or(Effort::Auto);
    if !body.api_key.trim().is_empty() {
        cfg.keys
            .insert(body.provider.clone(), body.api_key.trim().to_string());
    }
    if let Some(base) = body.base_url.filter(|b| !b.trim().is_empty()) {
        cfg.base_url = Some(base);
    }
    if cfg.key_for(provider).is_none() {
        return bad(
            StatusCode::BAD_REQUEST,
            format!(
                "no key for {}; set {} or paste one",
                provider.name,
                provider
                    .env
                    .first()
                    .cloned()
                    .unwrap_or_else(|| "an API key".into())
            ),
        );
    }
    if let Err(e) = cfg.save() {
        return bad(StatusCode::INTERNAL_SERVER_ERROR, e.to_string());
    }
    *state.cfg.write().await = cfg;
    match state.start_agent(None).await {
        Ok(_) => Json(state.snapshot().await).into_response(),
        Err(e) => bad(StatusCode::BAD_REQUEST, e.to_string()),
    }
}

#[derive(Deserialize)]
struct ModelBody {
    provider: String,
    model: String,
    #[serde(default)]
    effort: Option<Effort>,
}

async fn set_model(AxumState(state): AxumState<State>, Json(body): Json<ModelBody>) -> Response {
    let registry = state.registry.read().await.clone();
    let Some(provider) = registry.provider(&body.provider) else {
        return bad(
            StatusCode::BAD_REQUEST,
            format!("unknown provider {}", body.provider),
        );
    };
    let Some(model) = provider.models.iter().find(|m| m.id == body.model) else {
        return bad(
            StatusCode::BAD_REQUEST,
            format!("unknown model {}", body.model),
        );
    };
    {
        let mut cfg = state.cfg.write().await;
        cfg.provider = body.provider.clone();
        cfg.model = body.model.clone();
        if let Some(effort) = body.effort {
            cfg.effort = effort;
        } else if !model.reasoning {
            cfg.effort = Effort::Auto;
        }
        if let Err(e) = cfg.save() {
            return bad(StatusCode::INTERNAL_SERVER_ERROR, e.to_string());
        }
    }
    let mut guard = state.agent.lock().await;
    if let Some(agent) = guard.as_mut() {
        let cfg = state.cfg.read().await.clone();
        if let Err(e) = agent.reconfigure(&cfg, &registry) {
            return bad(StatusCode::BAD_REQUEST, e.to_string());
        }
        let meta = agent.session.meta.clone();
        drop(guard);
        *state.session.write().await = Some(meta.clone());
        state.broadcast(json!({"type": "model", "provider": body.provider, "model": body.model, "effort": state.cfg.read().await.effort}));
        state.broadcast(
            json!({"type": "session", "session": crate::state::session_payload(&meta, None)}),
        );
    }
    Json(state.snapshot().await).into_response()
}

async fn refresh_registry(AxumState(state): AxumState<State>) -> Response {
    match Registry::refresh().await {
        Ok(registry) => {
            *state.registry.write().await = registry;
            Json(state.snapshot().await).into_response()
        }
        Err(e) => bad(StatusCode::BAD_GATEWAY, e.to_string()),
    }
}

async fn list_sessions(AxumState(state): AxumState<State>) -> Response {
    let list: Vec<Value> = session::list()
        .iter()
        .map(|m| {
            json!({
                "id": m.id,
                "title": m.title,
                "created": m.created,
                "provider": m.provider,
                "model": m.model,
                "effort": m.effort,
            })
        })
        .collect();
    Json(json!({"sessions": list, "current": state.session.read().await.clone()})).into_response()
}

async fn new_session(AxumState(state): AxumState<State>) -> Response {
    match state.start_agent(None).await {
        Ok(meta) => {
            Json(json!({"session": crate::state::session_payload(&meta, None)})).into_response()
        }
        Err(e) => bad(StatusCode::BAD_REQUEST, e.to_string()),
    }
}

async fn session_events(AxumState(state): AxumState<State>, Path(id): Path<String>) -> Response {
    let _ = &state;
    match Session::open(&id) {
        Ok(s) => Json(json!({
            "meta": s.meta,
            "events": s.events,
            "totals": {"input": s.totals.input, "output": s.totals.output},
        }))
        .into_response(),
        Err(e) => bad(StatusCode::NOT_FOUND, e.to_string()),
    }
}

#[derive(Deserialize)]
struct RenameBody {
    title: String,
}

async fn rename_session(Path(id): Path<String>, Json(body): Json<RenameBody>) -> Response {
    let title = body.title.trim();
    if title.is_empty() {
        return bad(StatusCode::BAD_REQUEST, "title is empty");
    }
    match session::rename(&id, title) {
        Ok(()) => Json(json!({"ok": true, "title": title})).into_response(),
        Err(e) => bad(StatusCode::BAD_REQUEST, e.to_string()),
    }
}

async fn delete_session(AxumState(state): AxumState<State>, Path(id): Path<String>) -> Response {
    if let Err(e) = session::delete(&id) {
        return bad(StatusCode::BAD_REQUEST, e.to_string());
    }
    let current = state.session.read().await.clone();
    if current.map(|m| m.id == id).unwrap_or(false) {
        let _ = state.start_agent(None).await;
    }
    Json(json!({"ok": true})).into_response()
}
