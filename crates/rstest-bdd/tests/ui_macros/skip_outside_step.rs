use rstest_bdd as bdd;

fn misuse_skip_macro() {
    bdd::skip!("outside step");
}
