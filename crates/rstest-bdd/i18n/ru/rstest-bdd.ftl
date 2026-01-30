step-error-missing-fixture = Отсутствует фикстура «{ $name }» типа «{ $ty }» для функции шага «{ $step }»
step-error-execution = Ошибка при выполнении шага «{ $pattern }» через функцию «{ $function }»: { $message }
step-error-panic = Паника в шаге «{ $pattern }», функция «{ $function }»: { $message }
step-keyword-parse-error = недопустимое ключевое слово шага: { $keyword }
unsupported-step-type = неподдерживаемый тип шага: { $step_type }
step-pattern-not-compiled = регулярное выражение шаблона шага не скомпилировано; сначала вызовите compile() для шаблона «{ $pattern }»
placeholder-pattern-mismatch = шаблон не совпадает
placeholder-invalid-placeholder = недопустимый синтаксис заполнителя: { $details }
placeholder-invalid-pattern = недопустимый шаблон шага: { $pattern }
placeholder-not-compiled = шаблон шага «{ $pattern }» необходимо скомпилировать перед использованием
placeholder-syntax = недопустимый синтаксис заполнителя: { $details }
placeholder-syntax-detail = { $reason } в байте { $position } (нумерация с нуля){ $suffix }
placeholder-syntax-suffix = для заполнителя «{ $placeholder }»
step-context-ambiguous-override = Неоднозначная замена фикстуры: более одной фикстуры соответствует type_id { $type_id }. Замена проигнорирована.
panic-message-opaque-payload = <недоступная для отладки нагрузка паники типа { $type }>
assert-step-ok-panic = шаг вернул ошибку: { $error }
assert-step-err-success = шаг неожиданно завершился успешно
assert-step-err-missing-substring = ошибка «{ $display }» не содержит «{ $expected }»

assert-skip-not-skipped = ожидалось, что { $target } зафиксирует пропущенный результат
assert-skip-missing-message = ожидалось, что { $target } предоставит сообщение о пропуске, содержащее «{ $expected }»
assert-skip-missing-substring = сообщение о пропуске «{ $actual }» не содержит «{ $expected }»
assert-skip-unexpected-message = ожидалось, что { $target } не предоставит сообщение о пропуске
assert-skip-flag-mismatch = ожидалось, что флаг «{ $flag }» для { $target } будет { $expected }, но оказалось { $actual }

execution-error-skip = Step skipped{ $message ->
    [none] {""}
    *[other] : { $message }
}
execution-error-step-not-found = Step not found at index { $index }: { $keyword } { $text } (feature: { $feature_path }, scenario: { $scenario_name })
execution-error-missing-fixtures = Step '{ $step_pattern }' (defined at { $step_location }) requires fixtures { $required }, but the following are missing: { $missing }. Available fixtures from scenario: { $available } (feature: { $feature_path }, scenario: { $scenario_name })
execution-error-handler-failed = Step failed at index { $index }: { $keyword } { $text } - { $error } (feature: { $feature_path }, scenario: { $scenario_name })
