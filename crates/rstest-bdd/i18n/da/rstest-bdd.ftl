step-error-missing-fixture = Mangler fixturen "{ $name }" af typen "{ $ty }" til trinfunktionen "{ $step }"
step-error-execution = Fejl under udførelse af trinnet "{ $pattern }" via funktionen "{ $function }": { $message }
step-error-panic = Panik i trinnet "{ $pattern }", funktionen "{ $function }": { $message }
step-keyword-parse-error = ugyldigt trinnøgleord: { $keyword }
unsupported-step-type = trintypen understøttes ikke: { $step_type }
step-pattern-not-compiled = trinnets regex-mønster er ikke blevet kompileret; kald compile() først på mønsteret "{ $pattern }"
placeholder-pattern-mismatch = mønsteret matcher ikke
placeholder-invalid-placeholder = ugyldig pladsholdersyntaks: { $details }
placeholder-invalid-pattern = ugyldigt trinmønster: { $pattern }
placeholder-not-compiled = trinmønsteret "{ $pattern }" skal kompileres før brug
placeholder-syntax = ugyldig pladsholdersyntaks: { $details }
placeholder-syntax-detail = { $reason } ved byte { $position } (nulindekseret){ $suffix }
placeholder-syntax-suffix = for pladsholderen "{ $placeholder }"
step-context-ambiguous-override = Tvetydig fixtur-override: mere end én fixtur matcher type_id { $type_id }. Override ignoreret.
panic-message-opaque-payload = <ikke-debugbar panik-nyttelast af typen { $type }>
assert-step-ok-panic = trinnet returnerede en fejl: { $error }
assert-step-err-success = trinnet lykkedes uventet
assert-step-err-missing-substring = fejlen "{ $display }" indeholder ikke "{ $expected }"

assert-skip-not-skipped = det forventedes, at { $target } registrerede et sprunget udfald
assert-skip-missing-message = det forventedes, at { $target } leverede en skip-besked med '{ $expected }'
assert-skip-missing-substring = skip-beskeden '{ $actual }' indeholder ikke '{ $expected }'
assert-skip-unexpected-message = det forventedes, at { $target } ikke gav en skip-besked
assert-skip-flag-mismatch = det forventedes, at { $target }-flaget '{ $flag }' var { $expected }, men det var { $actual }

execution-error-skip = Trin sprunget over{ $has_message ->
    *[no] {""}
    [yes] : { $message }
}
execution-error-step-not-found = Trin ikke fundet ved indeks { $index }: { $keyword } { $text } (feature: { $feature_path }, scenarie: { $scenario_name })
execution-error-missing-fixtures = Trinnet '{ $step_pattern }' (defineret ved { $step_location }) kræver fixtures { $required }, men følgende mangler: { $missing }. Tilgængelige fixtures fra scenariet: { $available } (feature: { $feature_path }, scenarie: { $scenario_name })
execution-error-handler-failed = Trin fejlede ved indeks { $index }: { $keyword } { $text } - { $error } (feature: { $feature_path }, scenarie: { $scenario_name })
