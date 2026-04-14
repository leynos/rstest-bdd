use rstest_bdd::execution::{
    RuntimeMode as PublicRuntimeMode, TestAttributeHint as PublicTestAttributeHint,
};
use rstest_bdd_policy::{
    RuntimeMode as PolicyRuntimeMode, TestAttributeHint as PolicyTestAttributeHint,
};

fn main() {
    let policy_mode: PolicyRuntimeMode = PublicRuntimeMode::TokioCurrentThread;
    let public_mode: PublicRuntimeMode = PolicyRuntimeMode::Sync;
    let policy_hint: PolicyTestAttributeHint = PublicTestAttributeHint::RstestOnly;
    let public_hint: PublicTestAttributeHint =
        PolicyTestAttributeHint::RstestWithTokioCurrentThread;

    assert_eq!(policy_mode, PublicRuntimeMode::TokioCurrentThread);
    assert_eq!(public_mode, PolicyRuntimeMode::Sync);
    assert_eq!(policy_hint, PublicTestAttributeHint::RstestOnly);
    assert_eq!(
        public_hint,
        PolicyTestAttributeHint::RstestWithTokioCurrentThread
    );
}
