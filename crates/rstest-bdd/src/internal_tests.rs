//! Internal unit tests for private helpers and conversions.
//! These tests validate the pattern scanner utilities and the
//! `IntoStepResult` specialisations that normalise step return values.
//! Keeping them here ensures behaviour remains stable while allowing
//! private access from a child module.

use crate::placeholder::{
    RegexBuilder, is_double_brace, is_escaped_brace, is_placeholder_start, parse_double_brace,
    parse_escaped_brace, parse_literal, parse_placeholder,
};

#[test]
fn predicates_detect_expected_tokens() {
    let s = br"\{\}{{}}{a}{_}";
    // Escaped braces
    assert!(is_escaped_brace(s, 0));
    assert!(!is_escaped_brace(s, 1));
    assert!(is_escaped_brace(s, 2));
    // Double braces
    assert!(is_double_brace(s, 4)); // "{{"
    assert!(is_double_brace(s, 6)); // "}}"
    // Placeholder start
    assert!(is_placeholder_start(s, 8)); // "{a"
    assert!(is_placeholder_start(s, 11)); // "{_"
}

#[test]
fn parse_escaped_and_double_braces() {
    // Escaped brace
    let mut st = RegexBuilder::new(r"\{");
    parse_escaped_brace(&mut st);
    assert_eq!(st.position, 2);
    assert!(st.output.ends_with(r"\{"));

    // Double brace
    let mut st2 = RegexBuilder::new("{{");
    parse_double_brace(&mut st2);
    assert_eq!(st2.position, 2);
    assert!(st2.output.ends_with(r"\{"));
}

#[test]
fn parse_placeholder_without_type_and_with_type() {
    // Without type; nested braces in placeholder content
    let mut st = RegexBuilder::new("before {outer {inner}} after");
    // Advance to the '{'
    st.position = "before ".len();
    #[expect(clippy::expect_used, reason = "test helper should fail loudly")]
    parse_placeholder(&mut st).expect("placeholder should parse");
    assert!(st.output.contains("(.+?)"));

    // With integer type
    let mut st2 = RegexBuilder::new("x {n:u32} y");
    st2.position = 2; // at '{'
    #[expect(clippy::expect_used, reason = "test helper should fail loudly")]
    parse_placeholder(&mut st2).expect("placeholder should parse");
    assert!(st2.output.contains(r"(\d+)"));
}

#[test]
fn parse_literal_writes_char() {
    let mut st = RegexBuilder::new("a");
    parse_literal(&mut st);
    assert_eq!(st.position, 1);
    assert!(st.output.ends_with('a'));
}

mod into_step_result {
    use crate::IntoStepResult;

    #[test]
    fn default_impl_boxes_payload() {
        let Ok(Some(boxed)) = 42_i32.into_step_result() else {
            panic!("basic types convert without error");
        };
        let Ok(value) = boxed.downcast::<i32>() else {
            panic!("value should downcast to i32");
        };
        assert_eq!(*value, 42);
    }

    #[test]
    fn unit_specialisation_returns_none() {
        let Ok(None) = ().into_step_result() else {
            panic!("unit conversion should succeed");
        };
    }

    #[test]
    fn result_unit_specialisation_maps_errors() {
        let Ok(None) = Result::<(), &str>::Ok(()).into_step_result() else {
            panic!("unit result conversion should succeed");
        };

        let Err(message) = Result::<(), &str>::Err("boom").into_step_result() else {
            panic!("error should bubble as string");
        };
        assert_eq!(message, "boom");
    }

    #[test]
    fn result_value_specialisation_boxes_payload() {
        let Ok(Some(boxed)) = Result::<String, &str>::Ok("value".to_owned()).into_step_result()
        else {
            panic!("result conversion should succeed");
        };
        let Ok(value) = boxed.downcast::<String>() else {
            panic!("payload should downcast to String");
        };
        assert_eq!(*value, "value");

        let Err(message) = Result::<String, &str>::Err("fail").into_step_result() else {
            panic!("error should bubble as string");
        };
        assert_eq!(message, "fail");
    }
}
