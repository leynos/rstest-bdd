step-error-missing-fixture = Mangler fiksturen "{ $name }" av typen "{ $ty }" for stegfunksjonen "{ $step }"
step-error-execution = Feil ved kjøring av steget "{ $pattern }" via funksjonen "{ $function }": { $message }
step-error-panic = Panikk i steget "{ $pattern }", funksjonen "{ $function }": { $message }
step-keyword-parse-error = ugyldig steg-nøkkelord: { $keyword }
unsupported-step-type = stegtypen støttes ikke: { $step_type }
placeholder-pattern-mismatch = mønsteret samsvarer ikke
placeholder-invalid-placeholder = ugyldig plassholder-syntaks: { $details }
placeholder-invalid-pattern = ugyldig stegmønster: { $pattern }
placeholder-syntax = ugyldig plassholder-syntaks: { $details }
placeholder-syntax-detail = { $reason } ved byte { $position } (nullindeksert){ $suffix }
placeholder-syntax-suffix = for plassholderen "{ $placeholder }"
step-context-ambiguous-override = Tvetydig fikstur-overstyring: mer enn én fikstur samsvarer med type_id { $type_id }. Overstyring ignorert.
panic-message-opaque-payload = <ikke-debuggbar panikk-nyttelast av typen { $type }>
assert-step-ok-panic = steget returnerte en feil: { $error }
assert-step-err-success = steget lyktes uventet
assert-step-err-missing-substring = feilen "{ $display }" inneholder ikke "{ $expected }"

assert-skip-not-skipped = forventet at { $target } skulle registrere et utfall som er hoppet over
assert-skip-missing-message = forventet at { $target } skulle gi en melding for hopp som inneholder '{ $expected }'
assert-skip-missing-substring = hopp-meldingen '{ $actual }' inneholder ikke '{ $expected }'
assert-skip-unexpected-message = forventet at { $target } ikke skulle gi en hopp-melding
assert-skip-flag-mismatch = forventet at flagget '{ $flag }' for { $target } skulle være { $expected }, men det var { $actual }

execution-error-skip = Trinn hoppet over{ $has_message ->
    *[no] {""}
    [yes] : { $message }
}
execution-error-step-not-found = Trinn ikke funnet ved indeks { $index }: { $keyword } { $text } (funksjon: { $feature_path }, scenario: { $scenario_name })
execution-error-missing-fixtures = Trinn "{ $step_pattern }" (definert ved { $step_location }) krever fiksturer { $required }, men følgende mangler: { $missing }. Tilgjengelige fiksturer fra scenario: { $available } (funksjon: { $feature_path }, scenario: { $scenario_name })
execution-error-handler-failed = Trinn feilet ved indeks { $index }: { $keyword } { $text } - { $error } (funksjon: { $feature_path }, scenario: { $scenario_name })
