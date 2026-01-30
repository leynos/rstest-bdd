step-error-missing-fixture = Falta a fixture '{ $name }' do tipo '{ $ty }' para a função de passo '{ $step }'
step-error-execution = Erro ao executar o passo '{ $pattern }' através da função '{ $function }': { $message }
step-error-panic = Pânico no passo '{ $pattern }', função '{ $function }': { $message }
step-keyword-parse-error = palavra-chave de passo inválida: { $keyword }
unsupported-step-type = tipo de passo não suportado: { $step_type }
step-pattern-not-compiled = a expressão regular do padrão do passo não foi compilada; execute compile() primeiro no padrão '{ $pattern }'
placeholder-pattern-mismatch = o padrão não corresponde
placeholder-invalid-placeholder = sintaxe de marcador inválida: { $details }
placeholder-invalid-pattern = padrão de passo inválido: { $pattern }
placeholder-not-compiled = o padrão de passo '{ $pattern }' deve ser compilado antes de ser usado
placeholder-syntax = sintaxe de marcador inválida: { $details }
placeholder-syntax-detail = { $reason } no byte { $position } (índice zero){ $suffix }
placeholder-syntax-suffix = para o marcador '{ $placeholder }'
step-context-ambiguous-override = Substituição de fixture ambígua: mais do que uma fixture corresponde a type_id { $type_id }. Substituição ignorada.
panic-message-opaque-payload = <carga opaca de pânico do tipo { $type }>
assert-step-ok-panic = o passo devolveu um erro: { $error }
assert-step-err-success = o passo teve sucesso de forma inesperada
assert-step-err-missing-substring = o erro '{ $display }' não contém '{ $expected }'

assert-skip-not-skipped = esperava-se que { $target } registasse um resultado ignorado
assert-skip-missing-message = esperava-se que { $target } fornecesse uma mensagem de salto que contivesse '{ $expected }'
assert-skip-missing-substring = a mensagem de salto '{ $actual }' não contém '{ $expected }'
assert-skip-unexpected-message = esperava-se que { $target } não fornecesse uma mensagem de salto
assert-skip-flag-mismatch = esperava-se que o sinalizador '{ $flag }' de { $target } fosse { $expected }, mas era { $actual }

execution-error-skip = Step skipped{ $message ->
    [none] {""}
    *[other] : { $message }
}
execution-error-step-not-found = Step not found at index { $index }: { $keyword } { $text } (feature: { $feature_path }, scenario: { $scenario_name })
execution-error-missing-fixtures = Step '{ $step_pattern }' (defined at { $step_location }) requires fixtures { $required }, but the following are missing: { $missing }. Available fixtures from scenario: { $available } (feature: { $feature_path }, scenario: { $scenario_name })
execution-error-handler-failed = Step failed at index { $index }: { $keyword } { $text } - { $error } (feature: { $feature_path }, scenario: { $scenario_name })
