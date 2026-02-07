//! USSP SFU Server
//!
//! Runs the SFU server with REST API control plane.

use clap::Parser;
use std::future::IntoFuture;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::{error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use ussp_control::{create_router, AppState, AuthConfig};
use ussp_sfu::{SfuConfig, SfuServer};

/// USSP SFU Server
#[derive(Parser, Debug)]
#[command(name = "ussp-server")]
#[command(about = "USSP Selective Forwarding Unit Server")]
struct Args {
    /// Server ID
    #[arg(long, env = "USSP_SERVER_ID", default_value = "sfu-1")]
    server_id: String,

    /// Media (UDP) bind address
    #[arg(long, env = "USSP_MEDIA_ADDR", default_value = "0.0.0.0:5000")]
    media_addr: String,

    /// Control (HTTP) bind address
    #[arg(long, env = "USSP_CONTROL_ADDR", default_value = "0.0.0.0:8080")]
    control_addr: String,

    /// JWT secret for authentication
    #[arg(long, env = "USSP_JWT_SECRET", default_value = "change-me-in-production")]
    jwt_secret: String,

    /// Disable authentication (for development)
    #[arg(long, env = "USSP_NO_AUTH")]
    no_auth: bool,

    /// Maximum rooms
    #[arg(long, env = "USSP_MAX_ROOMS", default_value = "100")]
    max_rooms: usize,

    /// Maximum participants per room
    #[arg(long, env = "USSP_MAX_PARTICIPANTS", default_value = "100")]
    max_participants: usize,

    /// Enable failover
    #[arg(long, env = "USSP_FAILOVER")]
    failover: bool,

    /// Log level
    #[arg(long, env = "RUST_LOG", default_value = "info")]
    log_level: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| args.log_level.clone().into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!(
        server_id = %args.server_id,
        media_addr = %args.media_addr,
        control_addr = %args.control_addr,
        "Starting USSP SFU Server"
    );

    // Build SFU config
    let mut sfu_config = SfuConfig::default();
    sfu_config.server_id = args.server_id.clone();
    sfu_config.media_bind_addr = args.media_addr.clone();
    sfu_config.control_bind_addr = args.control_addr.clone();
    sfu_config.max_rooms = args.max_rooms;
    sfu_config.max_participants_per_room = args.max_participants;
    sfu_config.failover.enabled = args.failover;

    // Create SFU server
    let sfu_server = Arc::new(SfuServer::new(sfu_config.clone()).await?);

    // Build auth config
    let auth_config = if args.no_auth {
        info!("Authentication disabled");
        AuthConfig::disabled()
    } else {
        AuthConfig::new(&args.jwt_secret)
    };

    // Create application state
    let app_state = AppState::new(auth_config);
    app_state
        .register_local_server(Arc::clone(&sfu_server), &sfu_config)
        .await;

    // Create API router
    let router = create_router(app_state);

    // Parse control address
    let control_addr: SocketAddr = args.control_addr.parse()?;

    // Start HTTP server
    let listener = TcpListener::bind(control_addr).await?;
    info!(addr = %control_addr, "Control plane listening");

    // Run SFU and HTTP server concurrently
    tokio::select! {
        result = sfu_server.run() => {
            if let Err(e) = result {
                error!(error = %e, "SFU server error");
            }
        }
        result = axum::serve(listener, router).into_future() => {
            if let Err(e) = result {
                error!(error = %e, "HTTP server error");
            }
        }
        _ = tokio::signal::ctrl_c() => {
            info!("Received shutdown signal");
            sfu_server.stop().await;
        }
    }

    info!("Server shutdown complete");
    Ok(())
}
