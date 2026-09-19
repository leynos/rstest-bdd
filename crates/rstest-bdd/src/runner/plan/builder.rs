//! The single construction path for a [`ScenarioPlan`].

use std::borrow::Cow;

use crate::{
    StepKeyword,
    runner::{
        plan::{ScenarioPlan, StepInvocation},
        source::{SourceLocation, SourcePath},
    },
};

/// Builds a [`ScenarioPlan`].
///
/// This is the only way to construct a plan, so every rule about what a plan
/// can contain lives here rather than being re-derived by each caller.
///
/// # Examples
///
/// ```
/// use rstest_bdd::{StepKeyword, runner::ScenarioPlanBuilder};
///
/// let plan = ScenarioPlanBuilder::new("Add two numbers", "notes/arithmetic.md")
///     .at_line(42)
///     .allow_skipped(false)
///     .step_at(StepKeyword::Given, "a calculator", 43)
///     .step_at(StepKeyword::When, "I add 2 and 2", 44)
///     .step_at(StepKeyword::Then, "the result is 4", 45)
///     .build();
///
/// assert_eq!(plan.steps().len(), 3);
/// ```
#[derive(Debug, Clone)]
pub struct ScenarioPlanBuilder {
    /// The scenario's name.
    name: Cow<'static, str>,
    /// The frontend's own identifier for the document.
    source: SourcePath,
    /// Where in that document the scenario is declared, when known.
    source_line: Option<u32>,
    /// Tags applied so far.
    tags: Vec<Cow<'static, str>>,
    /// Whether the scenario explicitly permits skipping.
    allow_skipped: bool,
    /// Step invocations accumulated so far.
    steps: Vec<StepInvocation>,
}

impl ScenarioPlanBuilder {
    /// Start a plan from a name and a source identity.
    ///
    /// # Examples
    ///
    /// ```
    /// use rstest_bdd::runner::ScenarioPlanBuilder;
    ///
    /// let plan = ScenarioPlanBuilder::new("demo", "notes/demo.md").build();
    /// assert_eq!(plan.source(), "notes/demo.md");
    /// ```
    #[must_use]
    pub fn new(name: impl Into<Cow<'static, str>>, source: impl Into<SourcePath>) -> Self {
        Self {
            name: name.into(),
            source: source.into(),
            source_line: None,
            tags: Vec::new(),
            allow_skipped: false,
            steps: Vec::new(),
        }
    }

    /// Record the line on which the scenario itself is declared.
    ///
    /// # Panics
    ///
    /// Panics when `line` is zero, because a source line is one-based. Active
    /// in every profile, for the reason
    /// [`SourceLocation::new`](crate::runner::SourceLocation::new) gives: the
    /// value reaches an outcome as a rendered `path:line`, and a zero that
    /// survived would be read as a real position. This mirrors that
    /// constructor, which guards the same coordinate for steps; without it
    /// `at_line` would be the one entry point that accepts a position no parser
    /// can have observed.
    ///
    /// # Examples
    ///
    /// ```
    /// use rstest_bdd::runner::ScenarioPlanBuilder;
    ///
    /// let plan = ScenarioPlanBuilder::new("demo", "notes/demo.md")
    ///     .at_line(42)
    ///     .build();
    /// assert_eq!(plan.source_line(), Some(42));
    /// ```
    #[must_use]
    pub fn at_line(mut self, line: u32) -> Self {
        assert!(line >= 1, "a source line is one-based");
        self.source_line = Some(line);
        self
    }

    /// Apply a tag to the scenario.
    ///
    /// # Examples
    ///
    /// ```
    /// use rstest_bdd::runner::ScenarioPlanBuilder;
    ///
    /// let plan = ScenarioPlanBuilder::new("demo", "notes/demo.md")
    ///     .tag("@smoke")
    ///     .build();
    /// assert_eq!(plan.tags().collect::<Vec<_>>(), ["@smoke"]);
    /// ```
    #[must_use]
    pub fn tag(mut self, tag: impl Into<Cow<'static, str>>) -> Self {
        self.tags.push(tag.into());
        self
    }

    /// State whether the scenario explicitly permits skipping.
    ///
    /// This is the value a Gherkin frontend derives from the `@allow_skipped`
    /// tag; a frontend with another tagging convention supplies its own answer.
    ///
    /// # Examples
    ///
    /// ```
    /// use rstest_bdd::runner::ScenarioPlanBuilder;
    ///
    /// let plan = ScenarioPlanBuilder::new("demo", "notes/demo.md")
    ///     .allow_skipped(true)
    ///     .build();
    /// assert!(plan.allow_skipped());
    /// ```
    #[must_use]
    pub fn allow_skipped(mut self, allow: bool) -> Self {
        self.allow_skipped = allow;
        self
    }

    /// Add a fully-specified step invocation.
    ///
    /// # Examples
    ///
    /// ```
    /// use rstest_bdd::{
    ///     StepKeyword,
    ///     runner::{ScenarioPlanBuilder, StepInvocation},
    /// };
    ///
    /// let step = StepInvocation::new(StepKeyword::Given, "a calculator")
    ///     .with_docstring("only for arithmetic");
    /// let plan = ScenarioPlanBuilder::new("demo", "notes/demo.md")
    ///     .step(step)
    ///     .build();
    /// assert_eq!(
    ///     plan.steps().first().map(|step| step.docstring()),
    ///     Some(Some("only for arithmetic"))
    /// );
    /// ```
    #[must_use]
    pub fn step(mut self, step: StepInvocation) -> Self {
        self.steps.push(step);
        self
    }

    /// Add a step located in the scenario's own source, sharing its path.
    ///
    /// A dedicated method rather than a `.step(..).at(..)` pair, because the
    /// pair would make `.at(..)` with no preceding step type-legal and force a
    /// panic or a silent no-op — a poor look in a crate that denies
    /// `unwrap_used` and whose runner must not panic.
    ///
    /// # Examples
    ///
    /// ```
    /// use rstest_bdd::{StepKeyword, runner::ScenarioPlanBuilder};
    ///
    /// let plan = ScenarioPlanBuilder::new("demo", "notes/demo.md")
    ///     .step_at(StepKeyword::Given, "a calculator", 43)
    ///     .build();
    /// let step = plan.steps().first().expect("one step was added");
    /// assert_eq!(
    ///     step.source().map(|source| source.path()),
    ///     Some("notes/demo.md")
    /// );
    /// ```
    #[must_use]
    pub fn step_at(
        mut self,
        keyword: StepKeyword,
        text: impl Into<Cow<'static, str>>,
        line: u32,
    ) -> Self {
        let source = SourceLocation::new(self.source.clone(), line, None);
        self.steps
            .push(StepInvocation::new(keyword, text).at(source));
        self
    }

    /// Consume the builder and produce the plan.
    ///
    /// # Examples
    ///
    /// ```
    /// use rstest_bdd::runner::ScenarioPlanBuilder;
    ///
    /// let plan = ScenarioPlanBuilder::new("demo", "notes/demo.md").build();
    /// assert!(plan.steps().is_empty());
    /// ```
    #[must_use]
    pub fn build(self) -> ScenarioPlan {
        ScenarioPlan::from_parts(
            self.name,
            self.tags,
            self.source,
            self.source_line,
            self.steps,
            self.allow_skipped,
        )
    }
}
