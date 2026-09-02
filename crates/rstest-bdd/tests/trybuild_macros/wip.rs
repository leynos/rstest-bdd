//! Locates and clears trybuild work-in-progress diagnostic files.

use std::{env, io, path::Path as StdPath};

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs::Dir};

pub(super) fn remove_stale_wip_stderr(test_path: &Utf8Path) -> io::Result<()> {
    let actual_path = wip_stderr_path(test_path.as_std_path());
    let current_dir = Dir::open_ambient_dir(".", ambient_authority())?;
    let crate_dir = Dir::open_ambient_dir(
        Utf8Path::new(env!("CARGO_MANIFEST_DIR")).as_std_path(),
        ambient_authority(),
    )?;

    remove_file_if_present(&current_dir, actual_path.as_std_path())?;
    remove_file_if_present(&crate_dir, actual_path.as_std_path())
}

pub(super) fn read_wip_stderr(
    current_dir: &Dir,
    crate_dir: &Dir,
    path: &StdPath,
) -> io::Result<(String, bool)> {
    match current_dir.read_to_string(path) {
        Ok(actual) => Ok((actual, true)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            crate_dir.read_to_string(path).map(|actual| (actual, false))
        }
        Err(error) => Err(error),
    }
}

pub(super) fn wip_stderr_path(test_path: &StdPath) -> Utf8PathBuf {
    let Some(file_name) = test_path.file_name() else {
        panic!("trybuild test path must include file name");
    };
    let Some(file_name) = file_name.to_str() else {
        panic!("file name must be valid UTF-8");
    };
    let mut path = Utf8PathBuf::from(file_name);
    path.set_extension("stderr");
    Utf8PathBuf::from("wip").join(path)
}

fn remove_file_if_present(crate_dir: &Dir, path: &StdPath) -> io::Result<()> {
    match crate_dir.remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}
