step-error-missing-fixture = Fixtura „{ $name }” de tip „{ $ty }” lipsește pentru funcția de pas „{ $step }”
step-error-execution = Eroare la executarea pasului „{ $pattern }” prin funcția „{ $function }”: { $message }
step-error-panic = Panicǎ în pasul „{ $pattern }”, funcția „{ $function }”: { $message }
step-keyword-parse-error = cuvânt cheie de pas invalid: { $keyword }
unsupported-step-type = tip de pas neacceptat: { $step_type }
step-pattern-not-compiled = expresia regulată a șablonului de pas nu a fost compilată; apelați mai întâi compile() pe șablonul „{ $pattern }”
placeholder-pattern-mismatch = șablonul nu se potrivește
placeholder-invalid-placeholder = sintaxă invalidă pentru loc rezervat: { $details }
placeholder-invalid-pattern = șablon de pas invalid: { $pattern }
placeholder-not-compiled = șablonul de pas „{ $pattern }” trebuie compilat înainte de utilizare
placeholder-syntax = sintaxă invalidă pentru loc rezervat: { $details }
placeholder-syntax-detail = { $reason } la octetul { $position } (indexare de la zero){ $suffix }
placeholder-syntax-suffix = pentru locul rezervat „{ $placeholder }”
step-context-ambiguous-override = Înlocuire de fixtură ambiguă: mai multe fixturi corespund type_id { $type_id }. Înlocuirea a fost ignorată.
panic-message-opaque-payload = <încărcătură de panică opacă de tip { $type }>
assert-step-ok-panic = pasul a returnat o eroare: { $error }
assert-step-err-success = pasul a reușit neașteptat
assert-step-err-missing-substring = eroarea „{ $display }” nu conține „{ $expected }”

assert-skip-not-skipped = s-a așteptat ca { $target } să înregistreze un rezultat skip
assert-skip-missing-message = s-a așteptat ca { $target } să furnizeze un mesaj de skip care să conțină '{ $expected }'
assert-skip-missing-substring = mesajul de skip '{ $actual }' nu conține '{ $expected }'
assert-skip-unexpected-message = s-a așteptat ca { $target } să nu furnizeze un mesaj de skip
assert-skip-flag-mismatch = s-a așteptat ca flag-ul '{ $flag }' al { $target } să fie { $expected }, dar a fost { $actual }

execution-error-skip = Pas omis{ $has_message ->
    *[no] {""}
    [yes] : { $message }
}
execution-error-step-not-found = Pas negăsit la indexul { $index }: { $keyword } { $text } (feature: { $feature_path }, scenariu: { $scenario_name })
execution-error-missing-fixtures = Pasul „{ $step_pattern }" (definit la { $step_location }) necesită fixturi { $required }, dar următoarele lipsesc: { $missing }. Fixturi disponibile din scenariu: { $available } (feature: { $feature_path }, scenariu: { $scenario_name })
execution-error-handler-failed = Pas eșuat la indexul { $index }: { $keyword } { $text } - { $error } (feature: { $feature_path }, scenariu: { $scenario_name })
