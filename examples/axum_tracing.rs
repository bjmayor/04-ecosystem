use std::time::Duration;

use anyhow::Ok;
use axum::routing::get;
use tokio::{
    net::TcpListener,
    time::{sleep, Instant},
};
use tracing::{debug, info, instrument, level_filters::LevelFilter, warn};
use tracing_subscriber::{
    fmt::{self, format::FmtSpan},
    layer::SubscriberExt,
    util::SubscriberInitExt,
    Layer,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let file_appender = tracing_appender::rolling::daily("/tmp/logs", "ecosystem.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);
    let console = fmt::Layer::new()
        .with_ansi(true)
        .with_span_events(FmtSpan::CLOSE)
        .pretty()
        .with_filter(LevelFilter::INFO);

    let file = fmt::Layer::new()
        .with_writer(non_blocking)
        .with_ansi(false)
        .with_span_events(FmtSpan::CLOSE)
        .with_filter(LevelFilter::DEBUG);

    tracing_subscriber::registry()
        .with(console)
        .with(file)
        .init();
    // tracing_subscriber::fmt::init();
    let addr = "0.0.0.0:8080";
    let listener = TcpListener::bind(addr).await?;
    let app = axum::Router::new().route("/", get(index));
    info!("Listening on {}", addr);
    axum::serve(listener, app.into_make_service()).await?;
    Ok(())
}

#[instrument]
async fn index() -> &'static str {
    debug!("index handler started");
    sleep(Duration::from_millis(10)).await;
    let ret = long_task().await;
    info!(http.status = 200, "index handler completed");
    ret
}

#[instrument]
async fn long_task() -> &'static str {
    let start = Instant::now();
    sleep(Duration::from_millis(112)).await;
    let elapsed = start.elapsed().as_millis() as u64;
    warn!(app.task_duration = elapsed, "task takes too long");
    "Hello, World!"
}
