step-error-missing-fixture = Saknar fixturen "{ $name }" av typen "{ $ty }" för stegfunktionen "{ $step }"
step-error-execution = Fel vid körning av steget "{ $pattern }" via funktionen "{ $function }": { $message }
step-error-panic = Panik i steget "{ $pattern }", funktionen "{ $function }": { $message }
step-keyword-parse-error = ogiltigt stegnyckelord: { $keyword }
unsupported-step-type = stegtypen stöds inte: { $step_type }
placeholder-pattern-mismatch = mönstret matchade inte
placeholder-invalid-placeholder = ogiltig platshållarsyntax: { $details }
placeholder-invalid-pattern = ogiltigt stegmönster: { $pattern }
placeholder-syntax = ogiltig platshållarsyntax: { $details }
placeholder-syntax-detail = { $reason } vid byte { $position } (nollindexerat){ $suffix }
placeholder-syntax-suffix = för platshållaren "{ $placeholder }"
step-context-ambiguous-override = Tvetydig fixturersättning: fler än en fixtur matchar type_id { $type_id }. Ersättningen ignorerades.
panic-message-opaque-payload = <icke-avlusningsbar paniknyttolast av typen { $type }>
assert-step-ok-panic = steget returnerade ett fel: { $error }
assert-step-err-success = steget lyckades oväntat
assert-step-err-missing-substring = felet "{ $display }" innehåller inte "{ $expected }"

assert-skip-not-skipped = förväntade att { $target } skulle registrera ett hoppat resultat
assert-skip-missing-message = förväntade att { $target } skulle ange ett hopputelämningsmeddelande som innehåller '{ $expected }'
assert-skip-missing-substring = hopputelämningsmeddelandet '{ $actual }' innehåller inte '{ $expected }'
assert-skip-unexpected-message = förväntade att { $target } inte skulle ange ett hopputelämningsmeddelande
assert-skip-flag-mismatch = förväntade att { $target }-flaggan '{ $flag }' skulle vara { $expected }, men den var { $actual }

execution-error-skip = Steget hoppades över{ $has_message ->
    *[no] {""}
    [yes] : { $message }
}
execution-error-step-not-found = Steg hittades inte vid index { $index }: { $keyword } { $text } (feature: { $feature_path }, scenario: { $scenario_name })
execution-error-missing-fixtures = Steget "{ $step_pattern }" (definierat vid { $step_location }) kräver fixturer { $required }, men följande saknas: { $missing }. Tillgängliga fixturer från scenariot: { $available } (feature: { $feature_path }, scenario: { $scenario_name })
execution-error-handler-failed = Steget misslyckades vid index { $index }: { $keyword } { $text } - { $error } (feature: { $feature_path }, scenario: { $scenario_name })
