step-error-missing-fixture = Ontbrekende fixture « { $name } » van type « { $ty } » voor stapfunctie « { $step } »
step-error-execution = Fout bij uitvoeren van stap « { $pattern } » via functie « { $function } »: { $message }
step-error-panic = Paniek in stap « { $pattern } », functie « { $function } »: { $message }
step-keyword-parse-error = ongeldig stap-trefwoord: { $keyword }
unsupported-step-type = niet-ondersteund staptype: { $step_type }
step-pattern-not-compiled = regex van stappatroon is niet gecompileerd; roep eerst compile() aan voor patroon « { $pattern } »
placeholder-pattern-mismatch = patroon komt niet overeen
placeholder-invalid-placeholder = ongeldige placeholder-syntaxis: { $details }
placeholder-invalid-pattern = ongeldig stappatroon: { $pattern }
placeholder-not-compiled = stappatroon « { $pattern } » moet vóór gebruik worden gecompileerd
placeholder-syntax = ongeldige placeholder-syntaxis: { $details }
placeholder-syntax-detail = { $reason } bij byte { $position } (nulgebaseerd){ $suffix }
placeholder-syntax-suffix = voor placeholder « { $placeholder } »
step-context-ambiguous-override = Ambigue fixture-override: meer dan één fixture komt overeen met type_id { $type_id }. Override genegeerd.
panic-message-opaque-payload = <niet-debugbare panieklading van type { $type }>
assert-step-ok-panic = stap gaf een fout terug: { $error }
assert-step-err-success = stap is onverwacht geslaagd
assert-step-err-missing-substring = fout « { $display } » bevat « { $expected } » niet

assert-skip-not-skipped = verwachtte dat { $target } een overgeslagen resultaat zou registreren
assert-skip-missing-message = verwachtte dat { $target } een skip-bericht zou geven met '{ $expected }'
assert-skip-missing-substring = skip-bericht '{ $actual }' bevat niet '{ $expected }'
assert-skip-unexpected-message = verwachtte dat { $target } geen skip-bericht zou geven
assert-skip-flag-mismatch = verwachtte dat vlag '{ $flag }' van { $target } { $expected } zou zijn, maar het was { $actual }

execution-error-skip = Stap overgeslagen{ $has_message ->
    *[no] {""}
    [yes] : { $message }
}
execution-error-step-not-found = Stap niet gevonden op index { $index }: { $keyword } { $text } (feature: { $feature_path }, scenario: { $scenario_name })
execution-error-missing-fixtures = Stap « { $step_pattern } » (gedefinieerd in { $step_location }) vereist fixtures { $required }, maar de volgende ontbreken: { $missing }. Beschikbare fixtures van scenario: { $available } (feature: { $feature_path }, scenario: { $scenario_name })
execution-error-handler-failed = Stap gefaald op index { $index }: { $keyword } { $text } - { $error } (feature: { $feature_path }, scenario: { $scenario_name })
