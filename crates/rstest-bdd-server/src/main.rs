//! Language server binary for rstest-bdd.
//!
//! This binary provides an LSP server for Integrated Development Environment
//! (IDE) integration with the rstest-bdd Behaviour-Driven Development (BDD)
//! testing framework. It communicates via JSON-RPC over stdin/stdout.

use std::{ops::ControlFlow, path::PathBuf};

use async_lsp::{
    concurrency::ConcurrencyLayer,
    panic::CatchUnwindLayer,
    router::Router,
    server::LifecycleLayer,
    tracing::TracingLayer,
};
use clap::Parser;
use lsp_types::{notification, request};
use rstest_bdd_server::{
    config::{LogLevel, ServerConfig},
    error::ServerError,
    handlers::{,
    DeferredDocumentSavesIndexed,
    WorkspaceReadyEvent,
    handle_deferred_document_saves_indexed,
    handle_definition,
    handle_did_save_text_document,
    handle_implementation,
    handle_initialise,
    handle_initialised,
    handle_shutdown,
    handle_workspace_ready,
    launch_workspace_preparation,
    },
    logging::init_logging,
    server::ServerState,
};
use tower::ServiceBuilder;
use tracing::{info, warn};


//! Language server binary for rstest-bdd.
//!
//! This binary provides an LSP server for Integrated Development Environment
//! (IDE) integration with the rstest-bdd Behaviour-Driven Development (BDD)
//! testing framework. It communicates via JSON-RPC over stdin/stdout.
};
    handlers::{
};

/// LSP server for rstest-bdd Behaviour-Driven Development (BDD) testing framework.
#[derive(Parser, Debug)]
#[command(name = "rstest-bdd-lsp", version, about)]
struct Args {
    /// Log level (trace, debug, info, warn, error).
    #[arg(long)]
    log_level: Option<LogLevel>,

    /// Debounce interval in milliseconds for file change events.
    #[arg(long)]
    debounce_ms: Option<u64>,

    /// Override workspace root path for discovery.
    ///
    /// When provided, this path is used instead of the LSP client's
    /// root URI or workspace folders for workspace discovery.
    #[arg(long)]
    workspace_root: Option<PathBuf>,
}

fn main() {
    let args = Args::parse();

    let config = match build_config(&args) {
        Ok(config) => config,
        Err(e) => {
            let fallback = ServerConfig::default();
            init_logging(&fallback);
            tracing::error!(error = %e, "invalid configuration");
            std::process::exit(2);
        }
    };
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

fn build_config(args: &Args) -> Result<ServerConfig, ServerError> {
    let config = ServerConfig::from_env()?;
    Ok(config.apply_overrides(
        args.log_level,
        args.debounce_ms,
        args.workspace_root.clone(),
    ))
}

/// Asynchronously run the language server main loop.
async fn run_server_async(config: ServerConfig) -> std::io::Result<()> {
    let (server, _client) = async_lsp::MainLoop::new_server(|client| {
        let mut state = ServerState::new(config.clone());
        state.set_client(client.clone());
        let mut router = Router::new(state);
        configure_router(&mut router);

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

/// Register every LSP request, notification, and event on `router`.
fn configure_router(router: &mut Router<ServerState>) {
    router
        .request::<request::Initialize, _>(|st, params| {
            let outcome = handle_initialise(st, params);
            let client = st.client().cloned();
            let initialization = launch_workspace_preparation(outcome, client);
            let result = match initialization {
                Ok((result, Some(task))) => {
                    st.replace_workspace_task(task);
                    Ok(result)
                }
                Ok((result, None)) => Ok(result),
                Err(error) => Err(error),
            };
            async move { result }
        })
        .request::<request::Shutdown, _>(|st, _params| {
            let result = handle_shutdown(st);
            let task = st.take_workspace_task();
            async move {
                if let Some(task) = task {
                    task.abort();
                    if let Err(error) = task.await
                        && !error.is_cancelled()
                    {
                        warn!(error = %error, "workspace task failed during shutdown");
                    }
                }
                result
            }
        })
        .request::<request::GotoDefinition, _>(|st, params| {
            let result = handle_definition(st, &params);
            std::future::ready(result)
        })
        .request::<request::GotoImplementation, _>(|st, params| {
            let result = handle_implementation(st, &params);
            std::future::ready(result)
        })
        .notification::<notification::Initialized>(|st, params| {
            handle_initialised(st, params);
            ControlFlow::Continue(())
        })
        .notification::<notification::Exit>(|st, ()| {
            if let Some(task) = st.take_workspace_task() {
                task.abort();
            }
            ControlFlow::Break(Ok(()))
        })
        .notification::<notification::DidOpenTextDocument>(|_, _| ControlFlow::Continue(()))
        .notification::<notification::DidChangeTextDocument>(|_, _| ControlFlow::Continue(()))
        .notification::<notification::DidSaveTextDocument>(|st, params| {
            handle_did_save_text_document(st, params);
            ControlFlow::Continue(())
        })
        .notification::<notification::DidCloseTextDocument>(|_, _| ControlFlow::Continue(()));

    router.event::<WorkspaceReadyEvent>(|state, event| {
        handle_workspace_ready(state, event);
        ControlFlow::Continue(())
    });
    router.event::<DeferredDocumentSavesIndexed>(|state, event| {
        handle_deferred_document_saves_indexed(state, event);
        ControlFlow::Continue(())
    });
}

#[cfg(test)]
mod tests {
    //! Protocol-wiring tests for the language-server binary.

    use std::future::pending;

    use tower::Service;

    use super::*;

    struct CancellationSignal(Option<tokio::sync::oneshot::Sender<()>>);

    impl Drop for CancellationSignal {
        fn drop(&mut self) {
            if let Some(sender) = self.0.take() {
                let _ = sender.send(());
            }
        }
    }

    #[tokio::test]
    async fn shutdown_aborts_and_awaits_the_owned_workspace_task() {
        let (cancelled_sender, cancelled) = tokio::sync::oneshot::channel();
        let (started_sender, started) = tokio::sync::oneshot::channel();
        let workspace_task = tokio::spawn(async move {
            let _cancellation_signal = CancellationSignal(Some(cancelled_sender));
            if started_sender.send(()).is_err() {
                return;
            }
            pending::<()>().await;
        });
        let mut state = ServerState::new(ServerConfig::default());
        state.replace_workspace_task(workspace_task);
        assert!(started.await.is_ok(), "workspace task should start");
        let mut router = Router::new(state);
        configure_router(&mut router);

        let response = router
            .call(
                serde_json::from_value(serde_json::json!({
                    "id": 0,
                    "method": "shutdown",
                }))
                .expect("shutdown request must deserialize"),
            )
            .await;

        assert!(response.is_ok(), "shutdown request should succeed");
        assert!(
            cancelled.await.is_ok(),
            "shutdown should abort and await the retained workspace task"
        );
    }
}
