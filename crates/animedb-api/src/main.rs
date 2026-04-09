use animedb_api::{build_router, build_schema};
use std::env;
use std::net::SocketAddr;

/// Runs the GraphQL API process.
///
/// Environment variables:
/// - `ANIMEDB_DATABASE_PATH`, default `/data/animedb.sqlite`
/// - `ANIMEDB_LISTEN_ADDR`, default `0.0.0.0:8080`
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let database_path =
        env::var("ANIMEDB_DATABASE_PATH").unwrap_or_else(|_| "/data/animedb.sqlite".to_string());
    let listen_addr =
        env::var("ANIMEDB_LISTEN_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".to_string());

    let schema = build_schema(database_path);
    let app = build_router(schema);
    let listener = tokio::net::TcpListener::bind(listen_addr).await?;
    let local_addr: SocketAddr = listener.local_addr()?;

    println!("animedb GraphQL API listening on {local_addr}");
    axum::serve(listener, app).await?;
    Ok(())
}
