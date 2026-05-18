//! Localized formatting helpers for execution errors.

use super::{ExecutionError, MissingFixturesDetails};

impl ExecutionError {
    /// Render the error message using the provided Fluent loader.
    ///
    /// This allows formatting the error using a specific locale loader rather than
    /// the global default. This is useful when you need consistent locale handling
    /// across nested error types.
    ///
    /// # Examples
    ///
    /// ```
    /// use i18n_embed::fluent::fluent_language_loader;
    /// use unic_langid::langid;
    /// use rstest_bdd::execution::ExecutionError;
    ///
    /// let loader = {
    ///     use i18n_embed::LanguageLoader;
    ///     use rstest_bdd::Localizations;
    ///     let loader = fluent_language_loader!();
    ///     i18n_embed::select(&loader, &Localizations, &[langid!("en-US")])
    ///         .expect("en-US locale should always be available");
    ///     loader
    /// };
    /// let error = ExecutionError::Skip { message: Some("not implemented".into()) };
    /// let message = error.format_with_loader(&loader);
    /// assert!(message.contains("skipped"));
    /// assert!(message.contains("not implemented"));
    /// ```
    #[must_use]
    pub fn format_with_loader(&self, loader: &crate::FluentLanguageLoader) -> String {
        match self {
            Self::Skip { message } => format_skip(loader, message.as_ref()),
            Self::StepNotFound { .. } => self.format_step_not_found(loader),
            Self::MissingFixtures(details) => format_missing_fixtures(loader, details),
            Self::HandlerFailed { .. } => self.format_handler_failed(loader),
        }
    }

    fn format_step_not_found(&self, loader: &crate::FluentLanguageLoader) -> String {
        let Self::StepNotFound {
            index,
            keyword,
            text,
            feature_path,
            scenario_name,
        } = self
        else {
            unreachable!("format_step_not_found only formats StepNotFound");
        };

        crate::localization::message_with_loader(loader, "execution-error-step-not-found", |args| {
            args.set("index", index.to_string());
            args.set("keyword", keyword.as_str().to_string());
            args.set("text", text.clone());
            args.set("feature_path", feature_path.clone());
            args.set("scenario_name", scenario_name.clone());
        })
    }

    fn format_handler_failed(&self, loader: &crate::FluentLanguageLoader) -> String {
        let Self::HandlerFailed {
            index,
            keyword,
            text,
            error,
            feature_path,
            scenario_name,
        } = self
        else {
            unreachable!("format_handler_failed only formats HandlerFailed");
        };

        crate::localization::message_with_loader(loader, "execution-error-handler-failed", |args| {
            args.set("index", index.to_string());
            args.set("keyword", keyword.as_str().to_string());
            args.set("text", text.clone());
            args.set("error", error.format_with_loader(loader));
            args.set("feature_path", feature_path.clone());
            args.set("scenario_name", scenario_name.clone());
        })
    }
}

fn format_skip(loader: &crate::FluentLanguageLoader, message: Option<&String>) -> String {
    crate::localization::message_with_loader(loader, "execution-error-skip", |args| {
        args.set(
            "has_message",
            if message.is_some() { "yes" } else { "no" }.to_string(),
        );
        args.set("message", message.cloned().unwrap_or_default());
    })
}

fn format_missing_fixtures(
    loader: &crate::FluentLanguageLoader,
    details: &MissingFixturesDetails,
) -> String {
    crate::localization::message_with_loader(loader, "execution-error-missing-fixtures", |args| {
        args.set("step_pattern", details.step_pattern.clone());
        args.set("step_location", details.step_location.clone());
        args.set("required", details.required.join(", "));
        args.set("missing", details.missing.join(", "));
        args.set(
            "missing_requirements",
            details.format_missing_requirements(),
        );
        args.set("available", details.available.join(", "));
        args.set(
            "has_suggestion",
            if details.has_suggestion { "yes" } else { "no" }.to_string(),
        );
        args.set("feature_path", details.feature_path.clone());
        args.set("scenario_name", details.scenario_name.clone());
    })
}
