//! Tests for the plan's lifetime-free, zero-copy construction paths.

use std::sync::Arc;

use crate::{
    StepKeyword,
    runner::{ScenarioPlan, ScenarioPlanBuilder, SourceLocation, StepInvocation},
};

/// The macro path must not copy: text and tags reach the plan as
/// `Cow::Borrowed` over the original `'static` literal.
///
/// Observed by pointer identity, because an accessor returning `&str` cannot
/// distinguish a borrow from a copy of equal contents. A builder that allocated
/// a `String` for either would leave the pointers unequal.
#[test]
fn macro_path_allocates_no_step_text() {
    const TEXT: &str = "a calculator";
    const TAG: &str = "@allow_skipped";
    const NAME: &str = "Add two numbers";
    const SOURCE: &str = "notes/arithmetic.md";

    let plan = ScenarioPlanBuilder::new(NAME, SOURCE)
        .tag(TAG)
        .step_at(StepKeyword::Given, TEXT, 43)
        .build();

    let step = plan.steps().first().expect("one step was added");
    assert!(
        std::ptr::eq(step.text().as_ptr(), TEXT.as_ptr()),
        "step text must borrow the static literal, not copy it",
    );

    let tag = plan.tags().next().expect("one tag was added");
    assert!(
        std::ptr::eq(tag.as_ptr(), TAG.as_ptr()),
        "tag must borrow the static literal, not copy it",
    );

    assert!(
        std::ptr::eq(plan.name().as_ptr(), NAME.as_ptr()),
        "the scenario name must borrow the static literal, not copy it",
    );
}

/// A dynamically parsed plan outlives the buffer it was parsed from.
///
/// Written as a parser over borrowed input rather than as struct construction,
/// because the ergonomic risk lives in parsing: a plan with a lifetime
/// parameter cannot be returned from a helper like this one (`E0515`), which
/// would make it the primary API for exactly the audience this work exists for.
///
/// The parser *copies* each piece it keeps, and that is the point rather than
/// an oversight. `step_at` demands `Cow<'static, str>`, so a `&'buffer str`
/// taken from the input cannot reach the builder — attempting it is `E0521`,
/// which is D3's no-lifetime decision enforced at the only place it could have
/// leaked. Ownership therefore ends at the parse, and the plan outlives the
/// buffer by construction rather than by convention.
#[test]
fn parses_and_outlives_its_buffer() {
    /// Parse a minimal `name`/`step` document, one directive per line.
    ///
    /// The real frontends are richer; what matters here is that every string in
    /// the result is owned by the plan, so the borrow of `buffer` ends when the
    /// function returns.
    fn parse(buffer: &str) -> ScenarioPlan {
        let mut builder = ScenarioPlanBuilder::new("untitled", "notes/demo.md");
        for line in buffer.lines() {
            let Some((directive, rest)) = line.split_once(' ') else {
                continue;
            };
            let rest = rest.trim().to_owned();
            match directive {
                "name" => builder = ScenarioPlanBuilder::new(rest, "notes/demo.md"),
                "line" => {
                    if let Ok(line_number) = rest.parse::<u32>() {
                        builder = builder.at_line(line_number);
                    }
                }
                "tag" => builder = builder.tag(rest),
                "allow_skipped" => builder = builder.allow_skipped(rest == "true"),
                "given" => {
                    builder = builder.step_at(StepKeyword::Given, rest, 1);
                }
                "when" => builder = builder.step_at(StepKeyword::When, rest, 2),
                "then" => builder = builder.step_at(StepKeyword::Then, rest, 3),
                _ => {}
            }
        }
        builder.build()
    }

    // The document is built, consumed by the parser, and then *dropped* before
    // the plan is used, so a plan borrowing from it could not compile.
    let plan = {
        let document = String::from(
            "name Buffer demo\nline 12\ntag @smoke\nallow_skipped true\ngiven a plan sourced from \
             a buffer\nwhen the buffer goes away\nthen the plan still works\n",
        );
        let plan = parse(&document);
        drop(document);
        plan
    };

    assert_eq!(plan.name(), "Buffer demo");
    assert_eq!(plan.source_line(), Some(12));
    assert_eq!(plan.tags().collect::<Vec<_>>(), ["@smoke"]);
    assert!(plan.allow_skipped());
    assert_eq!(plan.steps().len(), 3);
    assert_eq!(
        plan.steps().first().map(StepInvocation::text),
        Some("a plan sourced from a buffer")
    );
}

/// A dynamic frontend allocates one path per scenario, not one per step.
///
/// The plan's steps share a single `Arc`, so the strong count is the number of
/// steps plus the scenario's own reference — not one allocation per step.
#[test]
fn shares_one_source_path_across_steps() {
    let path: Arc<str> = Arc::from("spec/cases.toml");

    let plan = ScenarioPlanBuilder::new("shared path", path.clone())
        .step_at(StepKeyword::Given, "a first step", 1)
        .step_at(StepKeyword::When, "a second step", 2)
        .step_at(StepKeyword::Then, "a third step", 3)
        .build();

    assert_eq!(plan.steps().len(), 3);

    // Every step's path must be the *same allocation* as the local `path` the
    // builder was handed, not merely equal to it. Pointer identity is the only
    // observation that separates a share from a copy of equal contents.
    let step_paths = plan
        .steps()
        .iter()
        .filter_map(|step| step.source().map(SourceLocation::source_path))
        .collect::<Vec<_>>();
    assert_eq!(step_paths.len(), 3, "every step recorded a source");

    for step_path in &step_paths {
        assert!(
            matches!(
                step_path,
                crate::runner::SourcePath::Shared(shared) if Arc::ptr_eq(shared, &path),
            ),
            "every step must share the builder's one path allocation",
        );
    }

    // Five owners, all of them legitimate: the local `path` binding, the plan,
    // and one handle per step. The count is the second half of the argument —
    // `ptr_eq` alone would not notice a plan that allocated a second `Arc` for
    // its own use, whereas the count pins the total number of owners exactly.
    assert_eq!(
        Arc::strong_count(&path),
        5,
        "the local, the plan, and three steps must be the only owners",
    );
}

/// A plan is `Clone` and `'static`, with no lifetime parameter anywhere.
///
/// The `'static` claim is checked by a trait bound rather than by inspection.
#[test]
fn plan_is_clone_and_static() {
    fn assert_static<T: 'static>() {}

    let plan = ScenarioPlanBuilder::new("demo", "notes/demo.md")
        .step_at(StepKeyword::Given, "a step", 1)
        .build();
    let clone = plan.clone();

    assert_eq!(clone.name(), plan.name());
    assert_static::<crate::runner::ScenarioPlan>();

    // `spawn` demands `'static`, so this is the `'static` claim exercised
    // rather than merely asserted.
    let handle = std::thread::spawn(move || clone.steps().len());
    assert_eq!(handle.join().ok(), Some(1));
}
