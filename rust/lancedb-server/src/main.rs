// SPDX-License-Identifier: Apache-2.0
// SPDX-FileCopyrightText: Copyright The LanceDB Authors

use std::env;
use std::net::SocketAddr;

use lancedb_server::{AppState, app};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let data_dir = env::var("LANCEDB_DATA_DIR").unwrap_or_else(|_| "./data".to_owned());
    let bind = env::var("LANCEDB_BIND").unwrap_or_else(|_| "127.0.0.1:8080".to_owned());
    let address: SocketAddr = bind.parse()?;

    let state = AppState::connect(&data_dir).await?;
    let listener = tokio::net::TcpListener::bind(address).await?;
    println!("LanceDB server listening on http://{address}");

    axum::serve(listener, app(state))
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

async fn shutdown_signal() {
    if tokio::signal::ctrl_c().await.is_err() {
        std::future::pending::<()>().await;
    }
}
