use std::collections::HashMap;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Query, State as AxumState};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use termagent_core::{session, Effort};
use tokio::sync::broadcast::error::RecvError;
use tokio_util::sync::CancellationToken;

use crate::state::State;

pub fn router() -> Router<State> {
    Router::new().route("/ws", get(upgrade))
}

async fn upgrade(
    ws: WebSocketUpgrade,
    AxumState(state): AxumState<State>,
    Query(query): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> Response {
    if !query
        .get("token")
        .map(|t| t == &state.token)
        .unwrap_or(false)
    {
        return (StatusCode::UNAUTHORIZED, "invalid token").into_response();
    }
    if let Some(origin) = headers.get("origin").and_then(|v| v.to_str().ok()) {
        let allowed = origin == format!("http://127.0.0.1:{}", state.port)
            || origin == format!("http://localhost:{}", state.port);
        if !allowed {
            return (StatusCode::FORBIDDEN, "origin not allowed").into_response();
        }
    }
    ws.on_upgrade(move |socket| handle(socket, state))
}

async fn handle(socket: WebSocket, state: State) {
    let (mut sink, mut stream) = socket.split();
    let hello = json!({"type": "hello", "state": state.snapshot().await});
    if sink
        .send(Message::Text(hello.to_string().into()))
        .await
        .is_err()
    {
        return;
    }
    let mut events = state.events.subscribe();
    loop {
        tokio::select! {
            broadcast = events.recv() => {
                match broadcast {
                    Ok(text) => {
                        if sink.send(Message::Text(text.into())).await.is_err() {
                            break;
                        }
                    }
                    Err(RecvError::Lagged(_)) => continue,
                    Err(RecvError::Closed) => break,
                }
            }
            incoming = stream.next() => {
                match incoming {
                    Some(Ok(Message::Text(text))) => {
                        handle_client(&state, text.as_str()).await;
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Err(_)) => break,
                    _ => {}
                }
            }
        }
    }
}

async fn handle_client(state: &State, raw: &str) {
    let Ok(value) = serde_json::from_str::<Value>(raw) else {
        state.broadcast(json!({"type": "error", "message": "invalid json"}));
        return;
    };
    let kind = value.get("type").and_then(Value::as_str).unwrap_or("");
    match kind {
        "message" => {
            let text = value
                .get("text")
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim()
                .to_string();
            if text.is_empty() {
                return;
            }
            if state
                .busy
                .compare_exchange(
                    false,
                    true,
                    std::sync::atomic::Ordering::SeqCst,
                    std::sync::atomic::Ordering::SeqCst,
                )
                .is_err()
            {
                state.broadcast(json!({"type": "error", "message": "a turn is already running"}));
                return;
            }
            let token = CancellationToken::new();
            if let Ok(mut slot) = state.cancel.lock() {
                *slot = Some(token.clone());
            }
            let st = state.clone();
            tokio::spawn(async move {
                let (result, totals, meta) = {
                    let mut guard = st.agent.lock().await;
                    match guard.as_mut() {
                        Some(agent) => {
                            let result = agent.run_turn(&text, token).await;
                            (
                                Some(result),
                                Some(agent.totals()),
                                Some(agent.session.meta.clone()),
                            )
                        }
                        None => (None, None, None),
                    }
                };
                st.busy.store(false, std::sync::atomic::Ordering::SeqCst);
                if let Some(totals) = totals {
                    *st.totals.write().await = totals;
                }
                if let Some(meta) = meta {
                    *st.session.write().await = Some(meta.clone());
                    st.broadcast(json!({"type": "session", "session": crate::state::session_payload(&meta, None)}));
                }
                match result {
                    Some(Ok(())) | Some(Err(termagent_core::Error::Cancelled)) => {}
                    Some(Err(e)) => {
                        st.broadcast(json!({"type": "error", "message": e.to_string()}))
                    }
                    None => {
                        st.broadcast(json!({"type": "error", "message": "session is not ready"}))
                    }
                }
                st.broadcast(json!({"type": "busy", "busy": false}));
            });
        }
        "approve" => {
            let id = value.get("id").and_then(Value::as_str).unwrap_or("");
            let allow = value.get("allow").and_then(Value::as_bool).unwrap_or(false);
            state.approvals.resolve(id, allow);
        }
        "cancel" => {
            if let Ok(slot) = state.cancel.lock() {
                if let Some(token) = slot.as_ref() {
                    token.cancel();
                }
            }
        }
        "set_model" => {
            let provider = value
                .get("provider")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let model = value
                .get("model")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let effort = value
                .get("effort")
                .and_then(Value::as_str)
                .and_then(|e| e.parse::<Effort>().ok());
            let st = state.clone();
            tokio::spawn(async move {
                if let Err(e) = apply_model(&st, provider, model, effort).await {
                    st.broadcast(json!({"type": "error", "message": e}));
                }
            });
        }
        "new_session" => {
            let st = state.clone();
            tokio::spawn(async move {
                match st.start_agent(None).await {
                    Ok(meta) => st.broadcast(json!({
                        "type": "session",
                        "session": crate::state::session_payload(&meta, Some(&[])),
                    })),
                    Err(e) => st.broadcast(json!({"type": "error", "message": e.to_string()})),
                }
            });
        }
        "load_session" => {
            let id = value
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let st = state.clone();
            tokio::spawn(async move {
                match st.start_agent(Some(id)).await {
                    Ok(meta) => {
                        let events = session::Session::open(&meta.id)
                            .map(|s| s.events)
                            .unwrap_or_default();
                        st.broadcast(json!({
                            "type": "session",
                            "session": crate::state::session_payload(&meta, Some(&events)),
                        }));
                    }
                    Err(e) => st.broadcast(json!({"type": "error", "message": e.to_string()})),
                }
            });
        }
        "rename" => {
            let id = value
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let title = value
                .get("title")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            if let Err(e) = session::rename(&id, &title) {
                state.broadcast(json!({"type": "error", "message": e.to_string()}));
            } else {
                state.broadcast(json!({"type": "renamed", "id": id, "title": title}));
            }
        }
        _ => {
            state.broadcast(json!({"type": "error", "message": format!("unknown message {kind}")}))
        }
    }
}

async fn apply_model(
    state: &State,
    provider: String,
    model: String,
    effort: Option<Effort>,
) -> Result<(), String> {
    let registry = state.registry.read().await.clone();
    let Some(p) = registry.provider(&provider) else {
        return Err(format!("unknown provider {provider}"));
    };
    let Some(m) = p.models.iter().find(|m| m.id == model) else {
        return Err(format!("unknown model {model}"));
    };
    {
        let mut cfg = state.cfg.write().await;
        if cfg.provider != provider {
            if provider != "custom" {
                cfg.base_url = None;
            }
            cfg.api_key.clear();
        }
        cfg.provider = provider.clone();
        cfg.model = model.clone();
        if let Some(effort) = effort {
            cfg.effort = effort;
        } else if !m.reasoning {
            cfg.effort = Effort::Auto;
        }
        cfg.save().map_err(|e| e.to_string())?;
    }
    let mut guard = state.agent.lock().await;
    if let Some(agent) = guard.as_mut() {
        let cfg = state.cfg.read().await.clone();
        agent
            .reconfigure(&cfg, &registry)
            .map_err(|e| e.to_string())?;
        let meta = agent.session.meta.clone();
        drop(guard);
        *state.session.write().await = Some(meta);
        let cfg = state.cfg.read().await.clone();
        state.broadcast(
            json!({"type": "model", "provider": provider, "model": model, "effort": cfg.effort}),
        );
    }
    Ok(())
}
