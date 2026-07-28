//! Compiler-channel detection for macro diagnostic selection.

use rustc_version::Channel;

fn main() {
    println!("cargo::rustc-check-cfg=cfg(rstest_bdd_nightly)");
    if rustc_version::version_meta().is_ok_and(|meta| meta.channel == Channel::Nightly) {
        println!("cargo::rustc-cfg=rstest_bdd_nightly");
    }
}
