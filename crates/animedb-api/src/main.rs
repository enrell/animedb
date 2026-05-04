use std::env;
use std::net::SocketAddr;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let database_path =
        env::var("ANIMEDB_DATABASE_PATH").unwrap_or_else(|_| "/data/animedb.sqlite".to_string());
    let listen_addr =
        env::var("ANIMEDB_LISTEN_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".to_string());

    let db_path = std::path::Path::new(&database_path);
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("failed to create data dir: {e}"))?;
    }

    let (app, migration_version) = tokio::task::spawn_blocking({
        let database_path = database_path.clone();
        move || {
            let db = animedb::AnimeDb::open(&database_path).map_err(|e| format!("{e}"))?;
            let migration_version = db
                .connection()
                .query_row("PRAGMA schema_version", [], |row| row.get::<_, i64>(0))
                .unwrap_or(-1);
            let schema = animedb_api::build_schema(&database_path);
            Ok::<_, String>((animedb_api::build_router(schema), migration_version))
        }
    })
    .await
    .map_err(|e| format!("join error: {e}"))??;

    eprintln!("animedb API starting");
    eprintln!(
        "  database: {} (schema v{})",
        database_path, migration_version
    );
    eprintln!("  listening on {}", listen_addr);

    let listener = tokio::net::TcpListener::bind(&listen_addr).await?;
    let local_addr: SocketAddr = listener.local_addr()?;
    println!("animedb GraphQL API listening on {local_addr}");

    // Graceful shutdown: tokio runtime handles SIGTERM/SIGINT and cancels the serve future.
    axum::serve(listener, app).await?;

    Ok(())
}
