use anyhow::Result;
use axum::{
    extract::{Path, State},
    response::IntoResponse,
    routing::{get, post},
    Json,
};
use http::{header::LOCATION, StatusCode};
use nanoid::nanoid;
use serde::{Deserialize, Serialize};
use sqlx::{prelude::FromRow, PgPool};
use tokio::net::TcpListener;
use tracing::{info, level_filters::LevelFilter, warn};
use tracing_subscriber::{fmt::Layer, layer::SubscriberExt, util::SubscriberInitExt, Layer as _};

#[derive(Debug, Deserialize)]
struct ShortenReq {
    url: String,
}

#[derive(Debug, Serialize)]
struct ShortenRes {
    url: String,
}

#[derive(Debug, Clone)]
struct AppState {
    db: PgPool,
}

#[derive(Debug, FromRow)]
struct UrlRecord {
    #[sqlx(default)]
    id: String,
    #[sqlx(default)]
    url: String,
}
const LISTEN_ADDR: &str = "127.0.0.1:9876";

#[tokio::main]
async fn main() -> Result<()> {
    let layer = Layer::new().pretty().with_filter(LevelFilter::INFO);
    tracing_subscriber::registry().with(layer).init();

    let url = "postgres://postgres:postgres@localhost:23432/shortener";
    let state = AppState::try_new(url).await?;
    info!("Connected to database:{url}");
    let listener = TcpListener::bind(LISTEN_ADDR).await?;
    let app = axum::Router::new()
        .route("/", post(shorten))
        .route("/:id", get(redirect))
        .with_state(state);
    info!("Listening on {}", LISTEN_ADDR);
    axum::serve(listener, app.into_make_service()).await?;
    Ok(())
}
async fn shorten(
    State(state): State<AppState>,
    Json(data): Json<ShortenReq>,
) -> Result<impl IntoResponse, StatusCode> {
    let id = state.shorten(&data.url).await.map_err(|e| {
        warn!("Failed to shorten URL: {:?}", e);
        StatusCode::UNPROCESSABLE_ENTITY
    })?;
    let body = Json(ShortenRes {
        url: format!("http://{}/{}", LISTEN_ADDR, id),
    });
    Ok((StatusCode::CREATED, body))
}
async fn redirect(
    Path(id): Path<String>,
    State(state): State<AppState>,
) -> Result<axum::http::Response<axum::body::Body>, StatusCode> {
    let url = state
        .get_url(&id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    Ok(axum::http::Response::builder()
        .status(StatusCode::PERMANENT_REDIRECT)
        .header(LOCATION, url)
        .body(axum::body::Body::empty())
        .unwrap())
}

impl AppState {
    async fn try_new(url: &str) -> Result<Self> {
        let db = PgPool::connect(url).await?;
        sqlx::query(
            r#"CREATE TABLE IF NOT EXISTS urls  (
							id CHAR(6) PRIMARY KEY,
							url TEXT NOT NULL UNIQUE
						)"#,
        )
        .execute(&db)
        .await?;
        Ok(Self { db })
    }

    async fn shorten(&self, url: &str) -> Result<String> {
        let id = nanoid!(6);
        let ret: UrlRecord = sqlx::query_as(
            "INSERT INTO urls (id, url) VALUES ($1, $2) ON CONFLICT(url) do update set url=excluded.url RETURNING *",
        )
        .bind(&id)
        .bind(url)
        .fetch_one(&self.db)
        .await?;
        Ok(ret.id)
    }

    async fn get_url(&self, id: &str) -> Result<Option<String>> {
        let record = sqlx::query_as::<_, UrlRecord>("SELECT url FROM urls WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.db)
            .await?;
        Ok(record.map(|item| item.url))
    }
}
