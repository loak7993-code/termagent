use axum::body::Body;
use axum::extract::State as AxumState;
use axum::http::{header, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use include_dir::{include_dir, Dir};

use crate::state::State;

static DIST: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../../web/dist");

pub async fn handler(AxumState(state): AxumState<State>, uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };

    if let Some(file) = DIST.get_file(path) {
        let mime = mime_guess::from_path(path).first_or_octet_stream();
        let bytes = file.contents();
        if path == "index.html" {
            return index(bytes, &state);
        }
        return Response::builder()
            .header(header::CONTENT_TYPE, mime.as_ref())
            .header(
                header::CACHE_CONTROL,
                if path.starts_with("assets/") {
                    "public, max-age=31536000, immutable"
                } else {
                    "no-store"
                },
            )
            .body(Body::from(bytes.to_vec()))
            .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response());
    }

    // SPA routes fall back to the shell; missing files stay 404.
    if !path.contains('.') || path.starts_with("s/") {
        if let Some(file) = DIST.get_file("index.html") {
            return index(file.contents(), &state);
        }
    }
    (StatusCode::NOT_FOUND, "not found").into_response()
}

fn index(bytes: &[u8], state: &State) -> Response {
    let raw = String::from_utf8_lossy(bytes);
    let html = raw
        .replace("__TERMAGENT_TOKEN__", &state.token)
        .replace("__TERMAGENT_PORT__", &state.port.to_string());
    Response::builder()
        .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
        .header(header::CACHE_CONTROL, "no-store")
        .body(Body::from(html))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}
