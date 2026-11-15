//! Derive Cargo's target root from the running test executable's location.

use std::env;

use camino::{Utf8Path, Utf8PathBuf};

pub(super) fn target_directory_from_running_test_executable() -> Option<Utf8PathBuf> {
    let executable = Utf8PathBuf::from_path_buf(env::current_exe().ok()?).ok()?;
    target_directory_from_test_executable(executable.as_path())
}

/// Extract Cargo's target root from an integration-test executable path. This
/// helper serves only the trybuild test-support module. Its callers must
/// use the returned root for both staged inputs and inspection artefacts.
///
/// Handles the classic `<target>/<profile>/deps/<exe>` layout and Cargo's
/// build-directory layout,
/// `<target>/<profile>/build/<package>/<hash>/out/<exe>`.
pub(super) fn target_directory_from_test_executable(executable: &Utf8Path) -> Option<Utf8PathBuf> {
    let parent = executable.parent()?;
    let profile = match parent.file_name() {
        Some("deps") => parent.parent()?,
        Some("out") => {
            let build = parent.ancestors().nth(3)?;
            (build.file_name() == Some("build")).then_some(build.parent()?)?
        }
        _ => return None,
    };
    Some(profile.parent()?.to_owned())
}
