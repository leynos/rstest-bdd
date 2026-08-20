//! Unit tests for server lifecycle handling.

use super::*;
use crate::config::ServerConfig;
use async_lsp::MainLoop;
use async_lsp::router::Router;
use lsp_types::{ClientCapabilities, WorkspaceFolder};
use rstest::{fixture, rstest};
use std::ops::ControlFlow;
use std::str::FromStr;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use tempfile::TempDir;
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

#[fixture]
fn create_test_state() -> ServerState {
    ServerState::new(ServerConfig::default())
}

#[fixture]
fn create_init_params() -> InitializeParams {
    InitializeParams {
        capabilities: ClientCapabilities::default(),
        workspace_folders: None,
        ..Default::default()
    }
}

/// Fixture providing a platform-specific test path.
#[fixture]
fn platform_test_path() -> PathBuf {
    #[cfg(windows)]
    let path = PathBuf::from("C:\\test\\path");
    #[cfg(not(windows))]
    let path = PathBuf::from("/test/path");
    path
}

#[rstest]
fn handle_initialize_stores_client_capabilities(
    mut create_test_state: ServerState,
    create_init_params: InitializeParams,
) {
    let result = handle_initialise(&mut create_test_state, create_init_params);

    assert!(result.is_ok());
    assert!(create_test_state.client_capabilities().is_some());
}

#[rstest]
fn handle_initialize_returns_server_info(
    mut create_test_state: ServerState,
    create_init_params: InitializeParams,
) {
    let result = handle_initialise(&mut create_test_state, create_init_params);
    let outcome = result.expect("initialization should succeed");

    let init_result = &outcome.result;
    assert!(init_result.server_info.is_some());
    let info = init_result
        .server_info
        .as_ref()
        .expect("should have server info");
    assert_eq!(info.name, "rstest-bdd-lsp");
    assert!(info.version.is_some());
}

#[rstest]
fn handle_initialize_fails_when_already_initialized(
    mut create_test_state: ServerState,
    create_init_params: InitializeParams,
) {
    create_test_state.mark_initialised();

    let result = handle_initialise(&mut create_test_state, create_init_params);

    assert!(result.is_err());
}

#[rstest]
fn handle_initialized_marks_state_as_initialized(mut create_test_state: ServerState) {
    assert!(!create_test_state.is_initialised());

    handle_initialised(&mut create_test_state, InitializedParams {});

    assert!(create_test_state.is_initialised());
}

#[rstest]
fn handle_shutdown_returns_ok(mut create_test_state: ServerState) {
    let result = handle_shutdown(&mut create_test_state);

    assert!(result.is_ok());
}

#[test]
fn url_to_path_returns_none_for_non_file_url() {
    let url = Url::from_str("https://example.com/path").expect("valid URL");
    let path = url_to_path(&url);

    assert!(path.is_none());
}

#[rstest]
fn url_to_path_handles_file_url(platform_test_path: PathBuf) {
    let url = Url::from_file_path(&platform_test_path).expect("valid path");
    let path = url_to_path(&url);

    assert!(path.is_some());
    assert_eq!(path.expect("should have path"), platform_test_path);
}

#[rstest]
#[case::from_workspace_folders(true, None)]
#[case::from_root_uri(false, Some("root_uri"))]
#[expect(
    clippy::used_underscore_binding,
    reason = "rstest uses this parameter; name matches review instructions"
)]
fn extract_workspace_path_from_various_sources(
    platform_test_path: PathBuf,
    #[case] use_folders: bool,
    #[case] _description: Option<&str>,
) {
    let url = Url::from_file_path(&platform_test_path).expect("valid path");

    let (folders, root_uri) = if use_folders {
        (
            vec![WorkspaceFolder {
                uri: url.clone(),
                name: "folder".to_string(),
            }],
            None,
        )
    } else {
        (Vec::new(), Some(url.clone()))
    };

    let path = extract_workspace_path(&folders, root_uri.as_ref());

    assert!(path.is_some());
    assert_eq!(path.expect("should have path"), platform_test_path);
}

#[test]
fn extract_workspace_path_returns_none_when_empty() {
    let path = extract_workspace_path(&[], None);

    assert!(path.is_none());
}

#[rstest]
fn handle_initialize_prefers_config_workspace_root_over_client(
    mut create_init_params: InitializeParams,
) {
    let override_path = PathBuf::from("/override/workspace");
    let client_workspace = TempDir::new().expect("client workspace");
    let client_uri = Url::from_file_path(client_workspace.path()).expect("client workspace URI");
    create_init_params.workspace_folders = Some(vec![WorkspaceFolder {
        uri: client_uri,
        name: "client-workspace".to_owned(),
    }]);
    let config = ServerConfig::default().with_workspace_root(override_path.clone());
    let mut state = ServerState::new(config);

    let result = handle_initialise(&mut state, create_init_params);
    let outcome = result.expect("initialization should succeed");

    // The config workspace root is used as the discovery path rather than the
    // client workspace folder.
    assert_eq!(outcome.workspace_path, Some(override_path.clone()));
    assert_eq!(state.config().workspace_root, Some(override_path));
}

/// Create a temporary Cargo workspace so `discover_workspace` succeeds.
fn cargo_workspace() -> std::io::Result<TempDir> {
    let dir = TempDir::new()?;
    let cargo_toml = dir.path().join("Cargo.toml");
    std::fs::write(
        cargo_toml,
        "[package]\nname = \"test-project\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )?;
    let src = dir.path().join("src");
    std::fs::create_dir_all(&src)?;
    std::fs::write(src.join("lib.rs"), "")?;
    Ok(dir)
}

#[test]
fn prepare_workspace_discovers_and_opens_capability() {
    let workspace = cargo_workspace().expect("create Cargo workspace");
    let preparation = prepare_workspace(workspace.path());

    match preparation {
        WorkspacePreparation::Discovered(info, _) => {
            assert_eq!(info.root, workspace.path());
            assert!(info.packages.contains(&"test-project".to_string()));
        }
        other => panic!("expected discovered workspace, got {other:?}"),
    }
}

#[test]
fn prepare_workspace_reports_non_fatal_failure_for_missing_path() {
    let path = std::path::Path::new("/nonexistent/rstest-bdd-test/workspace");
    let preparation = prepare_workspace(path);

    match preparation {
        WorkspacePreparation::DiscoveryAndRootOpenFailed { .. }
        | WorkspacePreparation::DiscoveryFailed { .. } => {}
        other => panic!("expected non-fatal workspace failure, got {other:?}"),
    }
}

#[test]
fn handle_workspace_ready_installs_capability() {
    let workspace = cargo_workspace().expect("create Cargo workspace");
    let mut state = ServerState::new(ServerConfig::default());
    let preparation = prepare_workspace(workspace.path());
    let WorkspacePreparation::Discovered(info, root) = preparation else {
        panic!("expected discovered workspace");
    };

    handle_workspace_ready(
        &mut state,
        WorkspaceReadyEvent {
            preparation: WorkspacePreparation::Discovered(info.clone(), root),
        },
    );

    assert_eq!(state.workspace_info().map(|i| &i.root), Some(&info.root));
    assert!(
        state
            .feature_index(&info.root.join("does_not_exist.feature"))
            .is_none()
    );
}

#[tokio::test]
async fn initialize_async_installs_workspace_capability_through_router_event() {
    let workspace = cargo_workspace().expect("create Cargo workspace");
    let feature_path = workspace.path().join("scenario.feature");
    std::fs::write(&feature_path, "Feature: async initialization\n").expect("write feature file");
    let installed = Arc::new(AtomicBool::new(false));
    let installed_for_router = Arc::clone(&installed);
    let feature_path_for_router = feature_path.clone();
    let (mainloop, client) = MainLoop::new_server(move |_| {
        let mut router = Router::new(ServerState::new(ServerConfig::default()));
        router.event::<WorkspaceReadyEvent>(move |state, event| {
            handle_workspace_ready(state, event);
            installed_for_router.store(
                state.index_feature_file(&feature_path_for_router).is_ok(),
                Ordering::Relaxed,
            );
            ControlFlow::Continue(())
        });
        router
    });
    let outcome = InitializeOutcome {
        workspace_path: Some(workspace.path().to_path_buf()),
        result: initialize_result(),
    };

    let result = initialize_async(Ok(outcome), Some(client.clone())).await;

    assert!(result.is_ok());
    let _ = mainloop
        .run_buffered(
            tokio::io::empty().compat(),
            tokio::io::sink().compat_write(),
        )
        .await;
    drop(client);
    assert!(
        installed.load(Ordering::Relaxed),
        "the router event should install a readable workspace capability"
    );
}

#[tokio::test]
async fn initialize_async_keeps_unavailable_workspace_non_fatal() {
    let outcome = InitializeOutcome {
        workspace_path: Some(PathBuf::from("/nonexistent/rstest-bdd-test/workspace")),
        result: initialize_result(),
    };

    let result = initialize_async(Ok(outcome), None).await;

    assert!(result.is_ok());
}
