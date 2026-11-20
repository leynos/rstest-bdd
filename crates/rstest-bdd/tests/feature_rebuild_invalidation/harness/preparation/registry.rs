//! The registry packages a family's fixture resolves to.
//!
//! A family's stamp has to speak for every unit its preparation builds, and
//! most of those are registry crates: `inventory`, `serial_test` and their
//! trees are compiled into the same profile as the workspace crates, and
//! Cargo rebuilds one whose fingerprint records have gone exactly as it
//! rebuilds a path dependency. Leaving them undeclared filtered their records
//! out of both what the stamp recorded and what the gate found, so an evicted
//! `inventory` compared equal to itself and the timed baseline rebuilt it.
//!
//! The list is read from the fixture's committed lockfile rather than from
//! `cargo tree`. A subprocess here would run before every timed gate and take
//! the package-cache lock, and the named preparation test already runs as
//! many bounded commands as its period allows. The lockfile costs one read.
//! What it cannot do is filter by platform: it names every package for every
//! target, most of which this host never builds. So the names are declared as
//! *owned if built* (`nested_cargo::Preparation::owned_if_built`), which
//! checks the ones that were built and excuses the ones that were not.

use std::{collections::BTreeSet, fs, io};

use super::{Family, WORKSPACE_DEPENDENCIES};

/// The registry packages `family`'s fixture lockfile names.
///
/// The workspace crates are left out because they are declared outright, and
/// the fixture package because its experiment removes it with `cargo clean
/// -p` before every baseline, so no stamp may claim it.
///
/// # Errors
///
/// Returns the read error naming the lockfile, and
/// [`io::ErrorKind::InvalidData`] when the lockfile names no registry package
/// at all: an empty claim would silently restore the gap this closes.
pub(super) fn registry_packages(family: Family) -> io::Result<Vec<String>> {
    let lockfile = (family.source)().join("Cargo.lock");
    let text = fs::read_to_string(&lockfile).map_err(|error| {
        io::Error::new(
            error.kind(),
            format!("cannot read {}: {error}", lockfile.display()),
        )
    })?;
    let mut names = lockfile_package_names(&text);
    names.remove(family.package);
    for workspace_crate in WORKSPACE_DEPENDENCIES {
        names.remove(*workspace_crate);
    }
    if names.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{} names no registry package", lockfile.display()),
        ));
    }
    Ok(names.into_iter().collect())
}

/// The package names in a Cargo lockfile's text.
///
/// Cargo writes each package as a `[[package]]` table whose `name` key starts
/// its own line; a dependency reference is an indented array entry and has no
/// key, so a line that begins `name = "` is always a package's own name.
fn lockfile_package_names(text: &str) -> BTreeSet<String> {
    text.lines()
        .filter_map(|line| line.strip_prefix("name = \""))
        .filter_map(|rest| rest.strip_suffix('"'))
        .map(str::to_owned)
        .collect()
}

#[cfg(test)]
mod tests {
    //! The lockfile reader, against the shapes a lockfile holds.

    use super::lockfile_package_names;

    #[test]
    fn only_a_packages_own_name_is_read() {
        let text = concat!(
            "version = 4\n\n",
            "[[package]]\n",
            "name = \"inventory\"\n",
            "version = \"0.3.25\"\n",
            "dependencies = [\n",
            " \"rustversion\",\n",
            "]\n\n",
            "[[package]]\n",
            "name = \"rustversion\"\n",
        );
        let names: Vec<String> = lockfile_package_names(text).into_iter().collect();
        assert_eq!(
            names,
            ["inventory", "rustversion"],
            "a dependency reference is not a package name"
        );
    }
}
