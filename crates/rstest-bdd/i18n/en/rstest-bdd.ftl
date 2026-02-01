step-error-missing-fixture = Missing fixture '{ $name }' of type '{ $ty }' for step function '{ $step }'
step-error-execution = Error executing step '{ $pattern }' via function '{ $function }': { $message }
step-error-panic = Panic in step '{ $pattern }', function '{ $function }': { $message }
step-keyword-parse-error = invalid step keyword: { $keyword }
unsupported-step-type = unsupported step type: { $step_type }
step-pattern-not-compiled = step pattern regex has not been compiled; call compile() first on pattern '{ $pattern }'
placeholder-pattern-mismatch = pattern mismatch
placeholder-invalid-placeholder = invalid placeholder syntax: { $details }
placeholder-invalid-pattern = invalid step pattern: { $pattern }
placeholder-not-compiled = step pattern '{ $pattern }' must be compiled before use
placeholder-syntax = invalid placeholder syntax: { $details }
placeholder-syntax-detail = { $reason } at byte { $position } (zero-based){ $suffix }
placeholder-syntax-suffix = for placeholder '{ $placeholder }'
step-context-ambiguous-override = Ambiguous fixture override: more than one fixture matches type_id { $type_id }. Override ignored.
panic-message-opaque-payload = <non-debug panic payload of type { $type }>
assert-step-ok-panic = step returned error: { $error }
assert-step-err-success = step succeeded unexpectedly
assert-step-err-missing-substring = error '{ $display }' does not contain '{ $expected }'

assert-skip-not-skipped = expected { $target } to record a skipped outcome
assert-skip-missing-message = expected { $target } to provide a skip message containing '{ $expected }'
assert-skip-missing-substring = skip message '{ $actual }' does not contain '{ $expected }'
assert-skip-unexpected-message = expected { $target } not to provide a skip message
assert-skip-flag-mismatch = expected { $target } flag '{ $flag }' to be { $expected }, but it was { $actual }

execution-error-skip = Step skipped{ $has_message ->
    [yes] : { $message }
    *[no] {""}
}
execution-error-step-not-found = Step not found at index { $index }: { $keyword } { $text } (feature: { $feature_path }, scenario: { $scenario_name })
execution-error-missing-fixtures = Step '{ $step_pattern }' (defined at { $step_location }) requires fixtures { $required }, but the following are missing: { $missing }. Available fixtures from scenario: { $available } (feature: { $feature_path }, scenario: { $scenario_name })
execution-error-handler-failed = Step failed at index { $index }: { $keyword } { $text } - { $error } (feature: { $feature_path }, scenario: { $scenario_name })
