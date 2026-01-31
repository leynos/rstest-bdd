step-error-missing-fixture = Chybí fixtura „{ $name }“ typu „{ $ty }“ pro funkci kroku „{ $step }“
step-error-execution = Chyba při provádění kroku „{ $pattern }“ pomocí funkce „{ $function }“: { $message }
step-error-panic = Panika v kroku „{ $pattern }“, funkce „{ $function }“: { $message }
step-keyword-parse-error = neplatné klíčové slovo kroku: { $keyword }
unsupported-step-type = typ kroku není podporován: { $step_type }
step-pattern-not-compiled = regulární výraz pro vzor kroku nebyl zkompilován; nejprve zavolejte compile() na vzoru „{ $pattern }“
placeholder-pattern-mismatch = vzor neodpovídá
placeholder-invalid-placeholder = neplatná syntaxe zástupného symbolu: { $details }
placeholder-invalid-pattern = neplatný vzor kroku: { $pattern }
placeholder-not-compiled = vzor kroku „{ $pattern }“ musí být před použitím zkompilován
placeholder-syntax = neplatná syntaxe zástupného symbolu: { $details }
placeholder-syntax-detail = { $reason } na bajtu { $position } (počítáno od nuly){ $suffix }
placeholder-syntax-suffix = pro zástupný symbol „{ $placeholder }“
step-context-ambiguous-override = Nejednoznačné přepsání fixtury: více než jedna fixtura odpovídá type_id { $type_id }. Přepsání ignorováno.
panic-message-opaque-payload = <neprůhledná užitečná data paniky typu { $type }>
assert-step-ok-panic = krok vrátil chybu: { $error }
assert-step-err-success = krok neočekávaně uspěl
assert-step-err-missing-substring = chyba „{ $display }“ neobsahuje „{ $expected }“

assert-skip-not-skipped = očekávalo se, že { $target } zaznamená přeskočený výsledek
assert-skip-missing-message = očekávalo se, že { $target } poskytne zprávu o přeskočení obsahující '{ $expected }'
assert-skip-missing-substring = zpráva o přeskočení '{ $actual }' neobsahuje '{ $expected }'
assert-skip-unexpected-message = očekávalo se, že { $target } neposkytne zprávu o přeskočení
assert-skip-flag-mismatch = očekávalo se, že příznak '{ $flag }' pro { $target } bude { $expected }, ale byl { $actual }

execution-error-skip = Step skipped{ $has_message ->
    *[no] {""}
    [yes] : { $message }
}
execution-error-step-not-found = Step not found at index { $index }: { $keyword } { $text } (feature: { $feature_path }, scenario: { $scenario_name })
execution-error-missing-fixtures = Step '{ $step_pattern }' (defined at { $step_location }) requires fixtures { $required }, but the following are missing: { $missing }. Available fixtures from scenario: { $available } (feature: { $feature_path }, scenario: { $scenario_name })
execution-error-handler-failed = Step failed at index { $index }: { $keyword } { $text } - { $error } (feature: { $feature_path }, scenario: { $scenario_name })
