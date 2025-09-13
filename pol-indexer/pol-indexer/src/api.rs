use axum::{routing::get, Router, response::IntoResponse, Json};
use eyre::Result;
use rusqlite::Connection;
use std::{net::SocketAddr, sync::Arc};
use tokio::sync::Mutex;
use crate::db;

pub async fn serve(db_path: String, bind: &str) -> Result<()> {
    let conn = Arc::new(Mutex::new(Connection::open(db_path)?));

    let app = Router::new().route("/netflow", get({
        let conn = conn.clone();
        move || {
            let conn = conn.clone();
            async move {
                let conn = conn.lock().await;
                let latest = db::get_latest_cumulative(&conn).map_err(|e| format!("{e}"))?;
                Ok::<_, String>(Json(latest)).into_response()
            }
        }
    }));

    let addr: SocketAddr = bind.parse().expect("invalid bind address");
    tracing::info!(%addr, "HTTP API listening");
    axum::serve(tokio::net::TcpListener::bind(addr).await?, app).await?;
    Ok(())
}
