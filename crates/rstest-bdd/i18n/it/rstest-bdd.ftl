step-error-missing-fixture = Fixture « { $name } » di tipo « { $ty } » mancante per la funzione di step « { $step } »
step-error-execution = Errore durante l'esecuzione dello step « { $pattern } » tramite la funzione « { $function } »: { $message }
step-error-panic = Panico nello step « { $pattern } », funzione « { $function } »: { $message }
step-keyword-parse-error = parola chiave dello step non valida: { $keyword }
unsupported-step-type = tipo di step non supportato: { $step_type }
placeholder-pattern-mismatch = il pattern non corrisponde
placeholder-invalid-placeholder = sintassi del segnaposto non valida: { $details }
placeholder-invalid-pattern = pattern dello step non valido: { $pattern }
placeholder-syntax = sintassi del segnaposto non valida: { $details }
placeholder-syntax-detail = { $reason } al byte { $position } (indice da zero){ $suffix }
placeholder-syntax-suffix = per il segnaposto « { $placeholder } »
step-context-ambiguous-override = Override della fixture ambiguo: più di una fixture corrisponde a type_id { $type_id }. Override ignorato.
panic-message-opaque-payload = <payload di panico non visualizzabile di tipo { $type }>
assert-step-ok-panic = lo step ha restituito un errore: { $error }
assert-step-err-success = lo step è riuscito inaspettatamente
assert-step-err-missing-substring = l'errore « { $display } » non contiene « { $expected } »

assert-skip-not-skipped = si è previsto che { $target } registrasse un risultato saltato
assert-skip-missing-message = si è previsto che { $target } fornisse un messaggio di skip contenente '{ $expected }'
assert-skip-missing-substring = il messaggio di skip '{ $actual }' non contiene '{ $expected }'
assert-skip-unexpected-message = si è previsto che { $target } non fornisse un messaggio di skip
assert-skip-flag-mismatch = si è previsto che il flag '{ $flag }' di { $target } fosse { $expected }, ma era { $actual }

execution-error-skip = Step saltato{ $has_message ->
    *[no] {""}
    [yes] : { $message }
}
execution-error-step-not-found = Step non trovato all'indice { $index }: { $keyword } { $text } (feature: { $feature_path }, scenario: { $scenario_name })
execution-error-missing-fixtures = Lo step « { $step_pattern } » (definito in { $step_location }) richiede fixture { $required }, ma mancano i seguenti: { $missing }. Fixture disponibili dallo scenario: { $available } (feature: { $feature_path }, scenario: { $scenario_name })
execution-error-handler-failed = Step fallito all'indice { $index }: { $keyword } { $text } - { $error } (feature: { $feature_path }, scenario: { $scenario_name })
