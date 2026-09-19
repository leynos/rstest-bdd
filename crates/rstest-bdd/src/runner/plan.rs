//! The parser-neutral scenario plan.
//!
//! A plan is a name, tags, a source identity, and an ordered list of step
//! invocations. It carries no lifetime: step text and tags are
//! `Cow<'static, str>`, so the macro path borrows its literals while a
//! dynamically parsed frontend owns its strings, and either way the plan
//! outlives the buffer it was parsed from.

use std::borrow::Cow;

use crate::{
    StepKeyword,
    runner::source::{SourceLocation, SourcePath},
};

pub mod builder;

/// One step occurrence within a scenario plan.
///
/// A `StepInvocation` is what a frontend *says* to run. It is not a result:
/// nothing here records what actually happened. That is
/// [`StepOutcome`](crate::runner::StepOutcome)'s job.
///
/// # Examples
///
/// ```
/// use rstest_bdd::{
///     StepKeyword,
///     runner::{SourceLocation, StepInvocation},
/// };
///
/// let step = StepInvocation::new(StepKeyword::Given, "a calculator")
///     .with_docstring("only for arithmetic")
///     .at(SourceLocation::new_static("notes/arithmetic.md", 43, None));
///
/// assert_eq!(step.keyword(), StepKeyword::Given);
/// assert_eq!(step.docstring(), Some("only for arithmetic"));
/// assert_eq!(step.source().map(SourceLocation::line), Some(43));
/// ```
#[derive(Debug, Clone)]
pub struct StepInvocation {
    /// The Gherkin keyword the frontend recorded for this step.
    keyword: StepKeyword,
    /// The step's own text, without the keyword.
    text: Cow<'static, str>,
    /// Optional doc string attached to the step.
    docstring: Option<Cow<'static, str>>,
    /// Optional data table attached to the step.
    table: Option<Vec<Vec<Cow<'static, str>>>>,
    /// Where the step was written, when the frontend knows.
    source: Option<SourceLocation>,
}

impl StepInvocation {
    /// Construct an invocation with no doc string, table, or source.
    ///
    /// # Examples
    ///
    /// ```
    /// use rstest_bdd::{StepKeyword, runner::StepInvocation};
    ///
    /// let step = StepInvocation::new(StepKeyword::When, "I add 2 and 2");
    /// assert_eq!(step.text(), "I add 2 and 2");
    /// ```
    #[must_use]
    pub fn new(keyword: StepKeyword, text: impl Into<Cow<'static, str>>) -> Self {
        Self {
            keyword,
            text: text.into(),
            docstring: None,
            table: None,
            source: None,
        }
    }

    /// Attach a doc string to this invocation.
    ///
    /// # Examples
    ///
    /// ```
    /// use rstest_bdd::{StepKeyword, runner::StepInvocation};
    ///
    /// let step = StepInvocation::new(StepKeyword::Then, "the result is 4")
    ///     .with_docstring("computed by the calculator");
    /// assert_eq!(step.docstring(), Some("computed by the calculator"));
    /// ```
    #[must_use]
    pub fn with_docstring(mut self, docstring: impl Into<Cow<'static, str>>) -> Self {
        self.docstring = Some(docstring.into());
        self
    }

    /// Attach a data table to this invocation.
    ///
    /// # Examples
    ///
    /// ```
    /// use rstest_bdd::{StepKeyword, runner::StepInvocation};
    ///
    /// let step = StepInvocation::new(StepKeyword::Given, "a table")
    ///     .with_table(vec![vec!["a".into(), "b".into()]]);
    /// assert_eq!(step.table().map(<[_]>::len), Some(1));
    /// ```
    #[must_use]
    pub fn with_table(mut self, rows: Vec<Vec<Cow<'static, str>>>) -> Self {
        self.table = Some(rows);
        self
    }

    /// Record where this invocation was written.
    ///
    /// # Examples
    ///
    /// ```
    /// use rstest_bdd::{
    ///     StepKeyword,
    ///     runner::{SourceLocation, StepInvocation},
    /// };
    ///
    /// let step = StepInvocation::new(StepKeyword::Then, "the result is 4")
    ///     .at(SourceLocation::new_static("spec/cases.toml", 8, None));
    /// assert_eq!(
    ///     step.source().map(SourceLocation::path),
    ///     Some("spec/cases.toml")
    /// );
    /// ```
    #[must_use]
    pub fn at(mut self, source: SourceLocation) -> Self {
        self.source = Some(source);
        self
    }

    /// Return the keyword the frontend recorded.
    ///
    /// # Examples
    ///
    /// ```
    /// use rstest_bdd::{StepKeyword, runner::StepInvocation};
    ///
    /// let step = StepInvocation::new(StepKeyword::But, "not this");
    /// assert_eq!(step.keyword(), StepKeyword::But);
    /// ```
    #[must_use]
    pub const fn keyword(&self) -> StepKeyword { self.keyword }

    /// Borrow the step's text, without its keyword.
    ///
    /// # Examples
    ///
    /// ```
    /// use rstest_bdd::{StepKeyword, runner::StepInvocation};
    ///
    /// let step = StepInvocation::new(StepKeyword::And, "also this");
    /// assert_eq!(step.text(), "also this");
    /// ```
    #[must_use]
    pub fn text(&self) -> &str { self.text.as_ref() }

    /// Borrow the doc string, when the step has one.
    ///
    /// # Examples
    ///
    /// ```
    /// use rstest_bdd::{StepKeyword, runner::StepInvocation};
    ///
    /// let step = StepInvocation::new(StepKeyword::Then, "then").with_docstring("because");
    /// assert_eq!(step.docstring(), Some("because"));
    /// ```
    #[must_use]
    pub fn docstring(&self) -> Option<&str> { self.docstring.as_deref() }

    /// Borrow the data table, when the step has one.
    ///
    /// # Examples
    ///
    /// ```
    /// use rstest_bdd::{StepKeyword, runner::StepInvocation};
    ///
    /// let step =
    ///     StepInvocation::new(StepKeyword::Given, "a table").with_table(vec![vec!["cell".into()]]);
    /// let table = step.table().expect("a table was supplied");
    /// assert_eq!(
    ///     table.first().and_then(|row| row.first()).map(AsRef::as_ref),
    ///     Some("cell")
    /// );
    /// ```
    #[must_use]
    pub fn table(&self) -> Option<&[Vec<Cow<'static, str>>]> { self.table.as_deref() }

    /// Borrow the recorded source location, when the frontend supplied one.
    ///
    /// # Examples
    ///
    /// ```
    /// use rstest_bdd::{
    ///     StepKeyword,
    ///     runner::{SourceLocation, StepInvocation},
    /// };
    ///
    /// let step = StepInvocation::new(StepKeyword::Then, "then").at(SourceLocation::new_static(
    ///     "notes/a.md",
    ///     4,
    ///     None,
    /// ));
    /// assert_eq!(step.source().map(SourceLocation::line), Some(4));
    /// ```
    #[must_use]
    pub const fn source(&self) -> Option<&SourceLocation> { self.source.as_ref() }
}

/// A parser-neutral plan for one scenario. `Clone` and `'static`.
///
/// Fields are private, so the type is deliberately **not**
/// `#[non_exhaustive]`: external construction and exhaustive destructuring are
/// already impossible, and the attribute would add nothing.
///
/// # Examples
///
/// ```
/// use rstest_bdd::{StepKeyword, runner::ScenarioPlanBuilder};
///
/// let plan = ScenarioPlanBuilder::new("Add two numbers", "notes/arithmetic.md")
///     .at_line(42)
///     .step_at(StepKeyword::Given, "a calculator", 43)
///     .build();
///
/// assert_eq!(plan.name(), "Add two numbers");
/// assert_eq!(plan.source_line(), Some(42));
/// assert_eq!(plan.steps().len(), 1);
/// ```
#[derive(Debug, Clone)]
pub struct ScenarioPlan {
    /// Human-readable scenario name.
    name: Cow<'static, str>,
    /// Tags applied to the scenario, in the order the frontend supplied them.
    tags: Vec<Cow<'static, str>>,
    /// The frontend's own identifier for the document.
    source: SourcePath,
    /// Where in that document the scenario is declared, when known.
    source_line: Option<u32>,
    /// The scenario's step invocations, in plan order.
    steps: Vec<StepInvocation>,
    /// Whether the scenario explicitly permits skipping.
    allow_skipped: bool,
}

impl ScenarioPlan {
    /// Borrow the scenario's name.
    ///
    /// # Examples
    ///
    /// ```
    /// use rstest_bdd::runner::ScenarioPlanBuilder;
    ///
    /// let plan = ScenarioPlanBuilder::new("demo", "notes/demo.md").build();
    /// assert_eq!(plan.name(), "demo");
    /// ```
    #[must_use]
    pub fn name(&self) -> &str { self.name.as_ref() }

    /// Iterate the scenario's tags.
    ///
    /// Tags are carried for the caller and the reporter. The runner itself
    /// reads only [`allow_skipped`](Self::allow_skipped).
    ///
    /// # Examples
    ///
    /// ```
    /// use rstest_bdd::runner::ScenarioPlanBuilder;
    ///
    /// let plan = ScenarioPlanBuilder::new("demo", "notes/demo.md")
    ///     .tag("@allow_skipped")
    ///     .build();
    /// assert_eq!(plan.tags().collect::<Vec<_>>(), ["@allow_skipped"]);
    /// ```
    pub fn tags(&self) -> impl Iterator<Item = &str> + '_ { self.tags.iter().map(AsRef::as_ref) }

    /// Borrow the frontend's own identifier for the document.
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
    pub fn source(&self) -> &str { self.source.as_str() }

    /// Return the scenario's own source line, when the frontend recorded one.
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
    pub const fn source_line(&self) -> Option<u32> { self.source_line }

    /// Borrow the ordered step invocations.
    ///
    /// # Examples
    ///
    /// ```
    /// use rstest_bdd::{StepKeyword, runner::ScenarioPlanBuilder};
    ///
    /// let plan = ScenarioPlanBuilder::new("demo", "notes/demo.md")
    ///     .step_at(StepKeyword::Given, "a calculator", 43)
    ///     .build();
    /// assert_eq!(
    ///     plan.steps().first().map(|step| step.text()),
    ///     Some("a calculator")
    /// );
    /// ```
    #[must_use]
    pub fn steps(&self) -> &[StepInvocation] { &self.steps }

    /// Whether the scenario explicitly permits skipping.
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
    pub const fn allow_skipped(&self) -> bool { self.allow_skipped }

    /// Construct the internal representation. Used by
    /// [`ScenarioPlanBuilder`](crate::runner::ScenarioPlanBuilder) only.
    pub(crate) fn from_parts(
        name: Cow<'static, str>,
        tags: Vec<Cow<'static, str>>,
        source: SourcePath,
        source_line: Option<u32>,
        steps: Vec<StepInvocation>,
        allow_skipped: bool,
    ) -> Self {
        Self {
            name,
            tags,
            source,
            source_line,
            steps,
            allow_skipped,
        }
    }
}
