//! JUnit XML writer for scenario outcome records.
//!
//! The writer produces a single `<testsuite>` document that callers can feed to
//! CI systems expecting JUnit reports. Each skipped scenario includes a
//! `<skipped>` child with an optional `message` attribute to preserve the
//! originating reason.

use std::fmt::{self, Write};

use super::{ScenarioRecord, ScenarioStatus, snapshot};

const FAIL_ON_SKIPPED_MESSAGE: &str = "Scenario skipped with fail_on_skipped enabled";

/// Render the supplied scenario records as a `JUnit` XML document.
///
/// # Examples
/// ```
/// use rstest_bdd::reporting::{junit, ScenarioRecord, ScenarioStatus};
///
/// let records = vec![ScenarioRecord::new(
///     "feature",
///     "scenario",
///     1,
///     Vec::new(),
///     ScenarioStatus::Passed,
/// )];
/// let mut output = String::new();
/// junit::write(&mut output, &records).unwrap();
/// assert!(output.contains("<testsuite"));
/// ```
///
/// # Errors
/// Returns an error if writing to the provided formatter fails.
pub fn write<W: Write>(writer: &mut W, records: &[ScenarioRecord]) -> fmt::Result {
    let tests = records.len();
    let skipped = records
        .iter()
        .filter(|record| matches!(record.status(), ScenarioStatus::Skipped(_)))
        .count();
    let failures = records
        .iter()
        .filter(|record| {
            matches!(
                record.status(),
                ScenarioStatus::Skipped(details) if details.forced_failure()
            )
        })
        .count();
    writer.write_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n")?;
    writeln!(
        writer,
        "<testsuite name=\"rstest-bdd\" tests=\"{tests}\" failures=\"{failures}\" skipped=\"{skipped}\">",
    )?;
    for record in records {
        writer.write_str("  <testcase name=\"")?;
        write_escaped(writer, record.scenario_name())?;
        writer.write_str("\" classname=\"")?;
        write_escaped(writer, record.feature_path())?;
        writer.write_char('"')?;
        match record.status() {
            ScenarioStatus::Passed => {
                writer.write_str(" />\n")?;
            }
            ScenarioStatus::Skipped(details) => {
                writer.write_str(">\n")?;
                writer.write_str("    <skipped")?;
                if let Some(message) = details.message() {
                    writer.write_str(" message=\"")?;
                    write_escaped(writer, message)?;
                    writer.write_char('"')?;
                }
                writer.write_str(" />\n")?;
                if details.forced_failure() {
                    writer.write_str("    <failure type=\"fail_on_skipped\">")?;
                    writer.write_str(FAIL_ON_SKIPPED_MESSAGE)?;
                    writer.write_str("</failure>\n")?;
                }
                writer.write_str("  </testcase>\n")?;
            }
        }
    }
    writer.write_str("</testsuite>\n")
}

/// Render the collector snapshot as a `JUnit` XML document.
///
/// # Examples
/// ```
/// use rstest_bdd::reporting::{junit, record, ScenarioRecord, ScenarioStatus};
///
/// record(ScenarioRecord::new(
///     "feature",
///     "scenario",
///     1,
///     Vec::new(),
///     ScenarioStatus::Passed,
/// ));
/// let mut output = String::new();
/// junit::write_snapshot(&mut output).unwrap();
/// assert!(output.contains("</testsuite>"));
/// ```
///
/// # Errors
/// Returns an error if writing to the provided formatter fails.
pub fn write_snapshot<W: Write>(writer: &mut W) -> fmt::Result {
    let snapshot = snapshot();
    write(writer, &snapshot)
}

fn write_escaped<W: Write>(writer: &mut W, value: &str) -> fmt::Result {
    const INVALID_REPLACEMENT: &str = "&#xFFFD;";
    for character in value.chars() {
        if !is_valid_xml_character(character) {
            writer.write_str(INVALID_REPLACEMENT)?;
            continue;
        }
        match character {
            '&' => writer.write_str("&amp;")?,
            '<' => writer.write_str("&lt;")?,
            '>' => writer.write_str("&gt;")?,
            '"' => writer.write_str("&quot;")?,
            '\'' => writer.write_str("&apos;")?,
            other => writer.write_char(other)?,
        }
    }
    Ok(())
}

fn is_valid_xml_character(character: char) -> bool {
    matches!(
        u32::from(character),
        0x09 | 0x0A | 0x0D
            | 0x20..=0xD7FF
            | 0xE000..=0xFFFD
            | 0x1_0000..=0x10_FFFF
    )
}
