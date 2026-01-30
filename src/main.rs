use askama::Template;
use std::sync::{atomic, Arc};
use axum::{
    Router,
    response::Json,
    extract::State,
};
use axum::http::{StatusCode};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use serde::Serialize;


#[derive(Template)]
#[template(path = "index-grok-v1.html")]
struct IndexTemplate {}

struct HtmlTemplate<T>(T);

impl<T> IntoResponse for HtmlTemplate<T>
where
    T: Template,
{
    fn into_response(self) -> Response {
        match self.0.render() {
            Ok(html) => Html(html).into_response(),
            Err(err) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to render template. Error: {err}"),
            )
                .into_response(),
        }
    }
}


#[derive(Default)]
struct AppState {
    good: atomic::AtomicI64,
    evil: atomic::AtomicI64,
}


#[derive(Serialize)]
struct StateDto {
    good: i64,
    evil: i64,
}

impl AppState {
    fn tap_good(&self) {
        self.good.fetch_add(1, atomic::Ordering::Relaxed);
    }

    fn tap_evil(&self) {
        self.evil.fetch_add(1, atomic::Ordering::Relaxed);
    }

    fn snapshot(&self) -> StateDto {
        StateDto {
            good: self.good.load(atomic::Ordering::Relaxed),
            evil: self.evil.load(atomic::Ordering::Relaxed),
        }
    }
}

#[tokio::main]
async fn main() {
    let state: Arc<AppState> = Arc::new(AppState::default());

    let api_v1 = Router::new()
        .route("/state", get(get_state))
        .route("/tap/good", get(tap_good))
        .route("/tap/evil", get(tap_evil));
    
    
    let templates_router = Router::new()
        .route("/", get(handler_index));
    
    let app = Router::new()
        .merge(templates_router)
        .nest("/api/v1", api_v1)
        .with_state(state);
    

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3500")
        .await
        .unwrap();

    axum::serve(listener, app).await.unwrap();
}

async fn handler_index() -> impl IntoResponse {
    let index = IndexTemplate {};
    HtmlTemplate(index)
}

async fn get_state(State(state): State<Arc<AppState>>) -> Json<StateDto> {
    Json(state.snapshot())
}

async fn tap_good(
    State(state): State<Arc<AppState>>,
) -> Json<StateDto> {
    state.tap_good();
    Json(state.snapshot())
}

async fn tap_evil(
    State(state): State<Arc<AppState>>,
) -> Json<StateDto> {
    state.tap_evil();
    Json(state.snapshot())
}
