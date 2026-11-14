use std::sync::{Arc, OnceLock};

use axum::body::Body;
use axum::http::{Request, StatusCode};
use futures_util::{SinkExt, StreamExt};
use http_body_util::BodyExt;
use termagent_core::registry::{Model, Protocol, Provider, Registry};
use termagent_core::{Config, Effort};
use termagent_web::{build_state, State};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tower::ServiceExt;

fn config_dir() -> &'static std::path::Path {
    static DIR: OnceLock<tempfile::TempDir> = OnceLock::new();
    let dir = DIR.get_or_init(|| {
        let d = tempfile::tempdir().unwrap();
        std::env::set_var("TERMAGENT_CONFIG_DIR", d.path());
        d
    });
    dir.path()
}

fn configured_cfg(_base: &str) -> Config {
    Config {
        provider: "mock".into(),
        model: "mock-model".into(),
        api_key: "sk-test-secret".into(),
        effort: Effort::Auto,
        ..Config::default()
    }
}

fn mock_registry(base: &str) -> Registry {
    Registry {
        fetched_at: 0,
        source: "test".into(),
        providers: vec![Provider {
            id: "mock".into(),
            name: "Mock".into(),
            env: vec!["MOCK_API_KEY".into()],
            api: base.to_string(),
            protocol: Protocol::OpenAi,
            models: vec![Model {
                id: "mock-model".into(),
                name: "Mock Model".into(),
                reasoning: true,
                efforts: vec!["low".into(), "high".into()],
                tool_call: true,
                context: 64_000,
                cost_in: 0.0,
                cost_out: 0.0,
            }],
        }],
    }
}

async fn mock_server() -> (String, Arc<std::sync::atomic::AtomicUsize>) {
    let body = "data: {\"choices\":[{\"delta\":{\"content\":\"hi there\"}}]}\n\ndata: {\"choices\":[],\"usage\":{\"prompt_tokens\":5,\"completion_tokens\":2}}\n\ndata: [DONE]\n\n";
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let hits = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counter = hits.clone();
    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                break;
            };
            counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let mut buf = vec![0u8; 65536];
            let _ = socket.read(&mut buf).await;
            let resp = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = socket.write_all(resp.as_bytes()).await;
            let _ = socket.shutdown().await;
        }
    });
    (format!("http://{addr}"), hits)
}

async fn state() -> State {
    let _ = config_dir();
    let (base, _hits) = mock_server().await;
    let registry = mock_registry(&base);
    let cwd = tempfile::tempdir().unwrap();
    let st = build_state(
        configured_cfg(&base),
        registry,
        cwd.path().to_path_buf(),
        6033,
    );
    st.start_agent(None).await.unwrap();
    st
}

async fn call(st: &State, req: Request<Body>) -> (StatusCode, String) {
    let app = termagent_web::app(st.clone());
    let resp = app.oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    (status, String::from_utf8_lossy(&bytes).to_string())
}

fn authorized(st: &State, method: &str, uri: &str) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header("x-termagent-token", &st.token)
        .header("content-type", "application/json")
        .body(Body::empty())
        .unwrap()
}

#[tokio::test]
async fn api_requires_the_boot_token() {
    let st = state().await;
    let (status, _) = call(
        &st,
        Request::builder()
            .uri("/api/state")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, body) = call(&st, authorized(&st, "GET", "/api/state")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("\"configured\":true"));
    assert!(!body.contains("sk-test-secret"), "api key leaked: {body}");
    assert!(body.contains("\"providers\""));
    assert!(body.contains("\"session\""));
}

#[tokio::test]
async fn index_injects_the_token() {
    let st = state().await;
    let (status, body) = call(
        &st,
        Request::builder().uri("/").body(Body::empty()).unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains(&st.token));
}

#[tokio::test]
async fn models_endpoint_lists_only_a_provider() {
    let _ = config_dir();
    let registry = Registry::snapshot().unwrap();
    let cwd = tempfile::tempdir().unwrap();
    let st = build_state(Config::default(), registry, cwd.path().to_path_buf(), 6033);
    let (status, body) = call(
        &st,
        authorized(&st, "GET", "/api/models?provider=anthropic"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("claude"));
    assert!(!body.contains("api_key"));
    let (status, _) = call(&st, authorized(&st, "GET", "/api/models?provider=nope")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn onboarding_requires_a_key_then_succeeds() {
    let _ = config_dir();
    std::env::remove_var("ANTHROPIC_API_KEY");
    let registry = Registry::snapshot().unwrap();
    let cwd = tempfile::tempdir().unwrap();
    let st = build_state(Config::default(), registry, cwd.path().to_path_buf(), 6033);
    let body = serde_json::json!({"provider":"anthropic","model":"claude-fable-5-2"}).to_string();
    let req = Request::builder()
        .method("POST")
        .uri("/api/onboard")
        .header("x-termagent-token", &st.token)
        .header("content-type", "application/json")
        .body(Body::from(body))
        .unwrap();
    let (status, text) = call(&st, req).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{text}");

    let body = serde_json::json!({
        "provider":"anthropic","model":"claude-fable-5-2","api_key":"sk-ant-test","effort":"medium"
    })
    .to_string();
    let req = Request::builder()
        .method("POST")
        .uri("/api/onboard")
        .header("x-termagent-token", &st.token)
        .header("content-type", "application/json")
        .body(Body::from(body))
        .unwrap();
    let (status, text) = call(&st, req).await;
    assert_eq!(status, StatusCode::OK, "{text}");
    assert!(text.contains("\"configured\":true"));
}

#[tokio::test]
async fn session_ids_reject_traversal() {
    let st = state().await;
    let (status, _) = call(
        &st,
        authorized(&st, "DELETE", "/api/sessions/..%2F..%2Fetc"),
    )
    .await;
    assert!(
        status == StatusCode::BAD_REQUEST || status == StatusCode::NOT_FOUND,
        "{status}"
    );
}

#[tokio::test]
async fn websocket_streams_a_turn_end_to_end() {
    let st = state().await;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let app = termagent_web::app(st.clone());
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });

    let bad =
        tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}/ws?token=wrong")).await;
    assert!(bad.is_err(), "unauthorized websocket connected");

    let url = format!("ws://127.0.0.1:{port}/ws?token={}", st.token);
    let (mut ws, _) = tokio_tungstenite::connect_async(url).await.unwrap();
    let hello = ws.next().await.unwrap().unwrap().into_text().unwrap();
    assert!(hello.contains("\"hello\""), "{hello}");

    ws.send(tokio_tungstenite::tungstenite::Message::Text(
        serde_json::json!({"type":"message","text":"say hi"})
            .to_string()
            .into(),
    ))
    .await
    .unwrap();

    let mut saw_text = false;
    let mut saw_done = false;
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
    while tokio::time::Instant::now() < deadline && !saw_done {
        let Ok(Some(Ok(msg))) =
            tokio::time::timeout(std::time::Duration::from_millis(800), ws.next()).await
        else {
            continue;
        };
        if !msg.is_text() {
            continue;
        }
        let text = msg.into_text().unwrap();
        if text.contains("\"text\"") && text.contains("hi there") {
            saw_text = true;
        }
        if text.contains("\"done\"") {
            saw_done = true;
        }
    }
    assert!(saw_text, "no streamed text");
    assert!(saw_done, "turn never finished");
}
