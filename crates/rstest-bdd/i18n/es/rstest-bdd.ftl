step-error-missing-fixture = Falta la fixture «{ $name }» de tipo «{ $ty }» para la función de paso «{ $step }»
step-error-execution = Error al ejecutar el paso «{ $pattern }» mediante la función «{ $function }»: { $message }
step-error-panic = Pánico en el paso «{ $pattern }», función «{ $function }»: { $message }
step-keyword-parse-error = palabra clave de paso inválida: { $keyword }
unsupported-step-type = tipo de paso no compatible: { $step_type }
step-pattern-not-compiled = la expresión regular del patrón de paso no se ha compilado; ejecute compile() primero sobre el patrón «{ $pattern }»
placeholder-pattern-mismatch = el patrón no coincide
placeholder-invalid-placeholder = sintaxis de marcador inválida: { $details }
placeholder-invalid-pattern = patrón de paso inválido: { $pattern }
placeholder-not-compiled = el patrón de paso «{ $pattern }» debe compilarse antes de usarlo
placeholder-syntax = sintaxis de marcador inválida: { $details }
placeholder-syntax-detail = { $reason } en el byte { $position } (comenzando en cero){ $suffix }
placeholder-syntax-suffix = para el marcador «{ $placeholder }»
step-context-ambiguous-override = Sobrescritura de fixture ambigua: más de una fixture coincide con type_id { $type_id }. Se ignoró la sobrescritura.
panic-message-opaque-payload = <carga opaca de pánico de tipo { $type }>
assert-step-ok-panic = el paso devolvió un error: { $error }
assert-step-err-success = el paso tuvo éxito de forma inesperada
assert-step-err-missing-substring = el error «{ $display }» no contiene «{ $expected }»

assert-skip-not-skipped = se esperaba que { $target } registrara un resultado omitido
assert-skip-missing-message = se esperaba que { $target } proporcionara un mensaje de omisión que contuviera '{ $expected }'
assert-skip-missing-substring = el mensaje de omisión '{ $actual }' no contiene '{ $expected }'
assert-skip-unexpected-message = se esperaba que { $target } no proporcionara un mensaje de omisión
assert-skip-flag-mismatch = se esperaba que la bandera '{ $flag }' de { $target } fuera { $expected }, pero fue { $actual }

execution-error-skip = Step skipped{ $message ->
    [none] {""}
    *[other] : { $message }
}
execution-error-step-not-found = Step not found at index { $index }: { $keyword } { $text } (feature: { $feature_path }, scenario: { $scenario_name })
execution-error-missing-fixtures = Step '{ $step_pattern }' (defined at { $step_location }) requires fixtures { $required }, but the following are missing: { $missing }. Available fixtures from scenario: { $available } (feature: { $feature_path }, scenario: { $scenario_name })
execution-error-handler-failed = Step failed at index { $index }: { $keyword } { $text } - { $error } (feature: { $feature_path }, scenario: { $scenario_name })
