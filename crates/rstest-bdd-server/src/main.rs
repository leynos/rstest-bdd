//! Language server binary for rstest-bdd.
//!
//! This binary provides an LSP server for IDE integration with the rstest-bdd
//! BDD testing framework. It communicates via JSON-RPC over stdin/stdout.

use std::ops::ControlFlow;

use async_lsp::concurrency::ConcurrencyLayer;
use async_lsp::panic::CatchUnwindLayer;
use async_lsp::router::Router;
use async_lsp::server::LifecycleLayer;
use async_lsp::tracing::TracingLayer;
use async_lsp::ClientSocket;
use clap::Parser;
use lsp_types::{notification, request};
use tower::ServiceBuilder;
use tracing::info;

use rstest_bdd_server::config::{LogLevel, ServerConfig};
use rstest_bdd_server::handlers::{handle_initialise, handle_initialised, handle_shutdown};
use rstest_bdd_server::logging::init_logging;
use rstest_bdd_server::server::ServerState;

/// LSP server for rstest-bdd BDD testing framework.
#[derive(Parser, Debug)]
#[command(name = "rstest-bdd-lsp", version, about)]
struct Args {
    /// Log level (trace, debug, info, warn, error).
    #[arg(long, default_value = "info")]
    log_level: String,
}

/// Internal server state that includes the client socket.
struct ServerStateWithClient {
    /// The underlying server state.
    state: ServerState,
    /// Socket for sending notifications to the client.
    #[allow(dead_code)]
    client: ClientSocket,
}

fn main() {
    let args = Args::parse();

    // Parse log level from CLI, falling back to environment or default
    let log_level: LogLevel = args
        .log_level
        .parse()
        .unwrap_or_else(|_| LogLevel::default());

    let config = ServerConfig::default().with_log_level(log_level);
    init_logging(&config);

    info!(
        version = env!("CARGO_PKG_VERSION"),
        "starting rstest-bdd-lsp"
    );

    let result = run_server(config);
    if let Err(e) = result {
        tracing::error!(error = %e, "server exited with error");
        std::process::exit(1);
    }
}

/// Run the language server.
fn run_server(config: ServerConfig) -> std::io::Result<()> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(run_server_async(config))
}

/// Asynchronously run the language server main loop.
async fn run_server_async(config: ServerConfig) -> std::io::Result<()> {
    let (server, _client) = async_lsp::MainLoop::new_server(|client| {
        let state = ServerStateWithClient {
            state: ServerState::new(config.clone()),
            client: client.clone(),
        };

        let mut router = Router::new(state);
        router
            .request::<request::Initialize, _>(|st, params| {
                let result = handle_initialise(&mut st.state, params);
                std::future::ready(result)
            })
            .request::<request::Shutdown, _>(|st, _params| {
                let result = handle_shutdown(&mut st.state);
                std::future::ready(result)
            })
            .notification::<notification::Initialized>(|st, params| {
                handle_initialised(&mut st.state, params);
                ControlFlow::Continue(())
            })
            .notification::<notification::Exit>(|_, ()| ControlFlow::Break(Ok(())))
            .notification::<notification::DidOpenTextDocument>(|_, _| ControlFlow::Continue(()))
            .notification::<notification::DidChangeTextDocument>(|_, _| ControlFlow::Continue(()))
            .notification::<notification::DidSaveTextDocument>(|_, _| ControlFlow::Continue(()))
            .notification::<notification::DidCloseTextDocument>(|_, _| ControlFlow::Continue(()));

        ServiceBuilder::new()
            .layer(TracingLayer::default())
            .layer(LifecycleLayer::default())
            .layer(CatchUnwindLayer::default())
            .layer(ConcurrencyLayer::default())
            .service(router)
    });

    // Use platform-appropriate stdio with tokio integration
    #[cfg(unix)]
    let (stdin, stdout) = (
        async_lsp::stdio::PipeStdin::lock_tokio()?,
        async_lsp::stdio::PipeStdout::lock_tokio()?,
    );
    #[cfg(not(unix))]
    let (stdin, stdout) = {
        use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};
        (
            tokio::io::stdin().compat(),
            tokio::io::stdout().compat_write(),
        )
    };

    server
        .run_buffered(stdin, stdout)
        .await
        .map_err(std::io::Error::other)?;

    info!("server exited");
    Ok(())
}
