//! `OBS-FIX-TEXT`: one snapshot of every wording variant.
//!
//! `trybuild` pins one representative per code and channel, which is what
//! proves the wiring and the span. It cannot economically pin every
//! combination of code, slot, framework, signature shape and resolver, so this
//! snapshot does: a wording change anywhere in the matrix fails here.

use proc_macro2::{Ident, Span};
use rstest::rstest;

use super::super::{
    CrateResolver,
    DiagnosticState,
    PolicyInputs,
    SlotIdents,
    classify_harness_path,
    classify_policy_path,
    default_resolver,
    emit::RenderedWarning,
    model::{Framework, SlotClass, diagnose},
    render,
};

/// A resolver for a renamed dependency, used to pin rename-aware fixes.
struct RenamedResolver;

impl CrateResolver for RenamedResolver {
    fn resolved_root(&self, _: &str) -> Option<Ident> { Some(Ident::new("tok", Span::call_site())) }

    fn is_direct_dependency(&self, _: &str) -> bool { true }
}

/// Render one state with both slots spelled `written`.
fn render_with(
    inputs: PolicyInputs,
    written: &str,
    resolver: &'static dyn CrateResolver,
) -> Vec<RenderedWarning> {
    let ident = Ident::new(written, Span::call_site());
    render(&DiagnosticState {
        inputs,
        slots: SlotIdents {
            harness: Some(ident.clone()),
            attributes: Some(ident),
        },
        findings: diagnose(inputs),
        resolver,
    })
}

/// One label plus the warnings it renders.
type Case = (&'static str, Vec<RenderedWarning>);

/// The scenario shape with the supplied slots.
fn scenario(harness: SlotClass, attributes: SlotClass, is_async: bool) -> PolicyInputs {
    PolicyInputs::Scenario {
        harness,
        attributes,
        is_async,
        has_user_test_attr: false,
    }
}

/// Every wording variant the matrix covers.
fn cases() -> Vec<Case> {
    let tokio = SlotClass::Evidenced(Framework::Tokio);
    let gpui = SlotClass::Evidenced(Framework::Gpui);
    let absent = SlotClass::Absent;
    vec![
        (
            "W001 sync scenario harness (tokio)",
            render_with(
                scenario(tokio, absent, false),
                "TokioHarness",
                default_resolver(),
            ),
        ),
        (
            "W001 async scenario harness (tokio)",
            render_with(
                scenario(tokio, absent, true),
                "TokioHarness",
                default_resolver(),
            ),
        ),
        (
            "W001 attributes (tokio)",
            render_with(
                scenario(absent, tokio, false),
                "TokioAttributePolicy",
                default_resolver(),
            ),
        ),
        (
            "W002 scenario harness (gpui)",
            render_with(
                scenario(gpui, absent, false),
                "GpuiHarness",
                default_resolver(),
            ),
        ),
        (
            "W001 attributes (gpui, attribute lost)",
            render_with(
                scenario(absent, gpui, false),
                "GpuiAttributePolicy",
                default_resolver(),
            ),
        ),
        (
            "W001 scenario harness (gpui, user attribute present)",
            render_with(
                PolicyInputs::Scenario {
                    harness: gpui,
                    attributes: absent,
                    is_async: false,
                    has_user_test_attr: true,
                },
                "GpuiHarness",
                default_resolver(),
            ),
        ),
        (
            "W002 renamed resolver",
            render_with(
                scenario(gpui, absent, false),
                "GpuiHarness",
                &RenamedResolver,
            ),
        ),
        (
            "W002 scenarios shape, no alias",
            render_with(
                PolicyInputs::Scenarios {
                    harness: gpui,
                    attributes: absent,
                    runtime_alias: false,
                },
                "GpuiHarness",
                default_resolver(),
            ),
        ),
    ]
}

#[test]
fn rendered_wording_matches_the_snapshot() {
    let text = cases()
        .into_iter()
        .map(|(label, warnings)| {
            let mut section = format!("## {label}\n");
            for warning in warnings {
                section.push_str(&warning.message);
                section.push('\n');
            }
            section
        })
        .collect::<Vec<_>>()
        .join("\n");

    insta::assert_snapshot!("wording_matrix", text);
}
/// The states whose messages carry a suggested path.
fn fix_states() -> Vec<(PolicyInputs, &'static str)> {
    let tokio = SlotClass::Evidenced(Framework::Tokio);
    let gpui = SlotClass::Evidenced(Framework::Gpui);
    let absent = SlotClass::Absent;
    vec![
        (scenario(tokio, absent, false), "TokioHarness"),
        (scenario(absent, tokio, false), "TokioAttributePolicy"),
        (scenario(gpui, absent, false), "GpuiHarness"),
        (scenario(absent, gpui, false), "GpuiAttributePolicy"),
    ]
}

/// Every backticked path a message suggests.
fn suggested_paths(message: &str) -> Vec<String> {
    message
        .split('`')
        .skip(1)
        .step_by(2)
        .filter(|text| text.contains("::") && !text.starts_with('#') && !text.contains(' '))
        .map(str::to_owned)
        .collect()
}

/// `LEM-FIX-CANONICAL`: a suggested path is itself a recognized first-party
/// path under the same resolver that rendered it.
#[rstest]
#[case::default(default_resolver())]
#[case::renamed(&RenamedResolver)]
fn every_suggested_path_is_recognized(#[case] resolver: &'static dyn CrateResolver) {
    let mut checked = 0_usize;
    let mut renamed_roots = 0_usize;
    for (inputs, written) in fix_states() {
        for warning in render_with(inputs, written, resolver) {
            for path in suggested_paths(&warning.message) {
                let parsed = match syn::parse_str::<syn::Path>(&path) {
                    Ok(parsed) => parsed,
                    Err(error) => panic!("suggested path {path} must parse: {error}"),
                };
                let class = if path.ends_with("Policy") {
                    classify_policy_path(&parsed, resolver)
                } else {
                    classify_harness_path(&parsed, resolver)
                };
                assert!(
                    matches!(class, SlotClass::Recognized(_)),
                    "suggested path {path} must be recognized, got {class:?}"
                );
                checked += 1;
                if path.starts_with("tok::") {
                    renamed_roots += 1;
                }
            }
        }
    }

    assert!(checked > 0, "the messages must suggest at least one path");
    assert_eq!(
        renamed_roots > 0,
        std::ptr::eq(resolver, &RenamedResolver),
        "a renamed resolver must render its own root"
    );
}
