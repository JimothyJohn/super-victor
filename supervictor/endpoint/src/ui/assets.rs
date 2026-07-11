//! Embedded static assets for the dashboard. `include_str!`-compiled into the
//! binary so deployments stay a single artifact (no static-file serving).

use axum::http::header;
use axum::response::IntoResponse;

/// Dashboard stylesheet. Palette lifted from `docs/index.html` so the
/// dashboard and landing page read as one product.
pub const STYLESHEET: &str = include_str!("style.css");

/// `GET /ui/assets/style.css`
pub async fn stylesheet() -> impl IntoResponse {
    (
        [
            (header::CONTENT_TYPE, "text/css; charset=utf-8"),
            (header::CACHE_CONTROL, "max-age=300"),
        ],
        STYLESHEET,
    )
}
