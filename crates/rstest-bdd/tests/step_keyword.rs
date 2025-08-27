//! Tests for `StepKeyword` conversions.

use gherkin::StepType;
use rstest_bdd::StepKeyword;

#[test]
fn from_str_parses_all_keywords() {
    assert_eq!(StepKeyword::from("Given"), StepKeyword::Given);
    assert_eq!(StepKeyword::from("When"), StepKeyword::When);
    assert_eq!(StepKeyword::from("Then"), StepKeyword::Then);
    assert_eq!(StepKeyword::from("And"), StepKeyword::And);
    assert_eq!(StepKeyword::from("But"), StepKeyword::But);
}

#[test]
fn from_step_type_maps_primary_keywords() {
    assert_eq!(StepKeyword::from(StepType::Given), StepKeyword::Given);
    assert_eq!(StepKeyword::from(StepType::When), StepKeyword::When);
    assert_eq!(StepKeyword::from(StepType::Then), StepKeyword::Then);
}
