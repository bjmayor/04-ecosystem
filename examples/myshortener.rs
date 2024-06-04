use anyhow::Result;
use axum::{
    debug_handler,
    extract::{Path, State},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json,
};
use http::{header::LOCATION, StatusCode};
use nanoid::nanoid;
use serde::{Deserialize, Serialize};
use serde_with::DisplayFromStr;
use sqlx::{prelude::FromRow, PgPool};
use thiserror::Error;
use tokio::net::TcpListener;

use tracing::level_filters::LevelFilter;
use tracing::{info, warn};
use tracing_subscriber::fmt::Layer;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::Layer as _;
const LISTEN_ADDR: &str = "127.0.0.1:9876";

#[derive(Debug, Error)]
enum AppError {
    #[error("db Error: {0}")]
    Sqlx(String),

    #[error("conflict Error: {0}")]
    Conflict(String),

    #[error("anyhow Error: {0}")]
    Anyhow(#[from] anyhow::Error),

    #[error("not found for {0}")]
    HttpNotFound(String),

    #[error("internal server error")]
    InternalServerError,
}

impl From<sqlx::Error> for AppError {
    fn from(e: sqlx::Error) -> Self {
        warn!("sqlx error: {:?}", e);
        if e.to_string()
            .contains("duplicate key value violates unique constraint")
        {
            return Self::Conflict(e.to_string());
        }

        Self::Sqlx(e.to_string())
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        #[serde_with::serde_as]
        #[serde_with::skip_serializing_none]
        #[derive(serde::Serialize)]
        struct ErrorResponse<'a> {
            // Serialize the `Display` output as the error message
            #[serde_as(as = "DisplayFromStr")]
            message: &'a AppError,
        }

        // Normally you wouldn't just print this, but it's useful for debugging without
        // using a logging framework.
        println!("API error: {self:?}");

        (self.status_code(), Json(ErrorResponse { message: &self })).into_response()
    }
}

impl AppError {
    fn status_code(&self) -> StatusCode {
        use AppError::*;
        match self {
            Sqlx(_) | Anyhow(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Conflict(_) => StatusCode::CONFLICT,
            HttpNotFound(_) => StatusCode::NOT_FOUND,
            InternalServerError => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

#[derive(Debug, Deserialize)]
struct ShortenReq {
    url: String,
}

#[derive(Debug, Serialize)]
struct ShortenRes {
    url: String,
}

// db model
#[derive(Debug, Clone, FromRow)]
struct UrlRecord {
    #[sqlx(default)]
    id: String,
    #[sqlx(default)]
    url: String,
}

// db is cheap to clone
#[derive(Debug, Clone)]
struct AppState {
    db: sqlx::PgPool,
}

impl AppState {
    async fn try_new(url: &str) -> Result<Self> {
        let db = PgPool::connect(url).await?;
        // create table if not exists
        sqlx::query(
            r#"
						CREATE TABLE IF NOT EXISTS urls (
								id VARCHAR(6) PRIMARY KEY,
								url TEXT NOT NULL UNIQUE
						)
						"#,
        )
        .execute(&db)
        .await?;
        Ok(Self { db })
    }

    // shorten url
    async fn shorten(&self, url: &str) -> Result<String, AppError> {
        loop {
            let id = nanoid!(6);
            let id = match self.create(id.as_str(), url).await {
                Ok(id) => id,
                Err(AppError::Conflict(_)) => continue,
                Err(e) => return Err(e),
            };
            return Ok(id);
        }
    }

    // for test duplicated id
    async fn create(&self, id: &str, url: &str) -> Result<String, AppError> {
        let ret: UrlRecord  = sqlx::query_as("INSERT INTO urls (id, url) VALUES ($1, $2) ON CONFLICT(url) do update set url=excluded.url RETURNING *")
				.bind(id)
				.bind(url)
				.fetch_one(&self.db)
				.await?;
        Ok(ret.id.clone())
    }

    // get url by id
    async fn get_url(&self, id: &str) -> Result<Option<String>> {
        let record = sqlx::query_as::<_, UrlRecord>("SELECT id,url FROM urls WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.db)
            .await?;
        Ok(record.map(|r| r.url))
    }
}
// axum example with 2 handlers
#[tokio::main]
async fn main() -> Result<()> {
    // tracing
    let layer = Layer::new().pretty().with_filter(LevelFilter::INFO);
    tracing_subscriber::registry().with(layer).init();
    let listener = TcpListener::bind(LISTEN_ADDR).await?;
    let url = "postgres://postgres:password@localhost:5432/shortener";
    info!("Listening on {}", LISTEN_ADDR);

    let app_state = AppState::try_new(url).await?;
    let app = axum::Router::new()
        .route("/", post(shorten_handler))
        .route("/:id", get(redirect_handler))
        .with_state(app_state);
    axum::serve(listener, app.into_make_service()).await?;
    Ok(())
}

#[debug_handler]
async fn shorten_handler(
    State(state): State<AppState>,
    Json(req): Json<ShortenReq>,
) -> Result<impl IntoResponse, AppError> {
    let id = state.shorten(&req.url).await?;
    let body = Json(ShortenRes {
        url: format!("http://{}/{}", LISTEN_ADDR, id),
    });
    Ok((StatusCode::CREATED, body))
}

async fn redirect_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<axum::http::Response<axum::body::Body>, AppError> {
    let url = state
        .get_url(&id)
        .await
        .map_err(|_| AppError::InternalServerError)?
        .ok_or(AppError::HttpNotFound(id))?;
    Ok(axum::http::Response::builder()
        .status(StatusCode::PERMANENT_REDIRECT)
        .header(LOCATION, url)
        .body(axum::body::Body::empty())
        .unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_shorten_should_work() {
        let url = "postgres://postgres:password@localhost:5432/shortener_test";
        let state = AppState::try_new(url).await.unwrap();
        // insert ok
        let id = state.shorten("https://www.google.com").await.unwrap();
        assert_eq!(id.len(), 6);

        let url = state.get_url(&id).await.unwrap().unwrap();
        assert_eq!(url, "https://www.google.com");

        // duplicate insert
        let id = state.shorten("https://www.google.com").await.unwrap();
        let url = state.get_url(&id).await.unwrap().unwrap();

        assert_eq!(url, "https://www.google.com");

        // test duplicated id
        let id = "abcdef";
        let url = "https://www.baidu.com";
        let id = state.create(id, url).await.unwrap();
        let url = state.get_url(id.as_str()).await.unwrap().unwrap();
        assert_eq!(url, "https://www.baidu.com");

        let ret = state
            .create(id.as_str(), "https://www.baidu.com/index")
            .await;
        assert!(ret.is_err());
        // is conflict error
        assert!(matches!(ret.unwrap_err(), AppError::Conflict(_)));

        // drop table for next test
        sqlx::query("delete from urls")
            .execute(&state.db)
            .await
            .unwrap();
    }
}
