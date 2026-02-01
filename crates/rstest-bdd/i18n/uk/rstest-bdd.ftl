step-error-missing-fixture = Фікстура « { $name } » типу « { $ty } » відсутня для функції кроку « { $step } »
step-error-execution = Помилка виконання кроку « { $pattern } » через функцію « { $function } »: { $message }
step-error-panic = Паніка у кроці « { $pattern } », функція « { $function } »: { $message }
step-keyword-parse-error = некоректне ключове слово кроку: { $keyword }
unsupported-step-type = непідтримуваний тип кроку: { $step_type }
step-pattern-not-compiled = регулярний вираз шаблону кроку не скомпільовано; спочатку викличте compile() для шаблону « { $pattern } »
placeholder-pattern-mismatch = невідповідність шаблону
placeholder-invalid-placeholder = некоректний синтаксис заповнювача: { $details }
placeholder-invalid-pattern = некоректний шаблон кроку: { $pattern }
placeholder-not-compiled = шаблон кроку « { $pattern } » потрібно скомпілювати перед використанням
placeholder-syntax = некоректний синтаксис заповнювача: { $details }
placeholder-syntax-detail = { $reason } на байті { $position } (нумерація від нуля){ $suffix }
placeholder-syntax-suffix = для заповнювача « { $placeholder } »
step-context-ambiguous-override = Неоднозначне перевизначення фікстури: більше ніж одна фікстура відповідає type_id { $type_id }. Перевизначення проігноровано.
panic-message-opaque-payload = <непрозоре навантаження паніки типу { $type }>
assert-step-ok-panic = крок повернув помилку: { $error }
assert-step-err-success = крок неочікувано завершився успіхом
assert-step-err-missing-substring = помилка « { $display } » не містить « { $expected } »

assert-skip-not-skipped = очікувалося, що { $target } зафіксує пропущений результат
assert-skip-missing-message = очікувалося, що { $target } надасть повідомлення пропуску з '{ $expected }'
assert-skip-missing-substring = повідомлення пропуску '{ $actual }' не містить '{ $expected }'
assert-skip-unexpected-message = очікувалося, що { $target } не надасть повідомлення пропуску
assert-skip-flag-mismatch = очікувалося, що прапорець '{ $flag }' для { $target } дорівнюватиме { $expected }, але було { $actual }

execution-error-skip = Крок пропущено{ $has_message ->
    *[no] {""}
    [yes] : { $message }
}
execution-error-step-not-found = Крок не знайдено за індексом { $index }: { $keyword } { $text } (feature: { $feature_path }, сценарій: { $scenario_name })
execution-error-missing-fixtures = Крок « { $step_pattern } » (визначено у { $step_location }) потребує фікстури { $required }, але відсутні: { $missing }. Доступні фікстури зі сценарію: { $available } (feature: { $feature_path }, сценарій: { $scenario_name })
execution-error-handler-failed = Крок завершився помилкою за індексом { $index }: { $keyword } { $text } - { $error } (feature: { $feature_path }, сценарій: { $scenario_name })
