use axum::{
    http::{StatusCode},
    response::sse::{Event, Sse},
    response::{Html, IntoResponse, Response, Json},
    routing::get,
    Router,
};
use askama::Template;
use std::sync::{atomic, Arc};
use serde::Serialize;
use axum_extra::TypedHeader;
use futures_util::stream::Stream;
use std::{convert::Infallible,time::Duration};
use axum::extract::State;
use serde_json::to_string;
use tokio_stream::StreamExt as _;

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

    let app = app();

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3500")
        .await
        .unwrap();

    axum::serve(listener, app).await.unwrap();
}

fn app() -> Router {
    let state: Arc<AppState> = Arc::new(AppState::default());

    let api_v1 = Router::new()
        .route("/state", get(get_state))
        .route("/tap/good", get(tap_good))
        .route("/tap/evil", get(tap_evil));


    let templates_router = Router::new()
        .route("/", get(handler_index))
        .route("/sse", get(sse_handler));

    let app = Router::new()
        .merge(templates_router)
        .nest("/api/v1", api_v1)
        .with_state(state);

    app
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

async fn sse_handler(
    TypedHeader(user_agent): TypedHeader<headers::UserAgent>,
    State(state): State<Arc<AppState>>
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    println!("`{}` connected", user_agent.as_str());

    // A `Stream` that repeats an event every second
    //
    // You can also create streams from tokio channels using the wrappers in
    // https://docs.rs/tokio-stream
    let stream = tokio_stream::wrappers::IntervalStream::new(
        tokio::time::interval(Duration::from_millis(200)),
    )
    .map(move |_| {
        match to_string(&state.snapshot()) {
            Ok(json) => Event::default().event("state").data(json),
            Err(_) => Event::default().event("error").data("{}"),
        }
    })
    .map(Ok);

    
    Sse::new(stream)
}
