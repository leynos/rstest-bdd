//! Unit tests for workspace-path conversion.

use std::{path::PathBuf, str::FromStr};

use lsp_types::Url;
use rstest::rstest;

use super::{super::url_to_path, platform_test_path};

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
