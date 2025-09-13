use clap::{Parser, Subcommand};
use eyre::Result;
use tracing_subscriber::{EnvFilter, fmt::Subscriber};

mod db;
mod indexer;
mod api;
mod models;

#[derive(Parser, Debug)]
#[command(name = "pol-indexer", version)]
struct Cli {
    /// Path to SQLite database file
    #[arg(long, env = "DB_PATH", default_value = "pol_indexer.sqlite")]
    db_path: String,

    /// Polygon RPC WebSocket URL
    #[arg(long, env = "RPC_URL")]
    rpc_url: String,

    /// POL token contract address (0x... on Polygon)
    #[arg(long, env = "POL_TOKEN_ADDRESS")]
    pol_token: String,

    /// Comma-separated Binance addresses to track (0x..,0x..)
    #[arg(long, env = "BINANCE_ADDRESSES")]
    binance_addresses: String,

    /// Optional: HTTP bind address for the query API (set to empty to disable)
    #[arg(long, env = "HTTP_BIND", default_value = "127.0.0.1:8080")]
    http_bind: String,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Run the real-time indexer (and API server if enabled)
    Run,
    /// Show the latest cumulative net-flow
    Query,
    /// Print the schema used by the indexer
    Schema,
}

#[tokio::main]
async fn main() -> Result<()> {
    let _ = dotenvy::dotenv();
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));
    Subscriber::builder().with_env_filter(filter).init();

    let cli = Cli::parse();

    // Init DB
    let conn = db::init(&cli.db_path)?;

    match cli.command.unwrap_or(Commands::Run) {
        Commands::Run => {
            let addr_list = models::parse_addresses(&cli.binance_addresses)?;
            let pol = models::parse_address(&cli.pol_token)?;

            // Spawn API server (optional)
            let api_handle = if !cli.http_bind.is_empty() {
                let db_path = cli.db_path.clone();
                let handle = tokio::spawn(async move {
                    if let Err(e) = api::serve(db_path, &cli.http_bind).await {
                        tracing::error!(?e, "API server error");
                    }
                });
                Some(handle)
            } else { None };

            // Run indexer (blocking until ctrl-c)
            indexer::run(cli.rpc_url.clone(), pol, addr_list, conn).await?;

            if let Some(h) = api_handle {
                let _ = h.await;
            }
        }
        Commands::Query => {
            let latest = db::get_latest_cumulative(&conn)?;
            println!("{}", serde_json::to_string_pretty(&latest)?);
        }
        Commands::Schema => {
            println!("{}", db::SCHEMA_SQL);
        }
    }

    Ok(())
}
