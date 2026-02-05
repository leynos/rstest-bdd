step-error-missing-fixture = Brak fiksury « { $name } » typu « { $ty } » dla funkcji kroku « { $step } »
step-error-execution = Błąd wykonywania kroku « { $pattern } » przez funkcję « { $function } »: { $message }
step-error-panic = Panika w kroku « { $pattern } », funkcja « { $function } »: { $message }
step-keyword-parse-error = nieprawidłowe słowo kluczowe kroku: { $keyword }
unsupported-step-type = nieobsługiwany typ kroku: { $step_type }
placeholder-pattern-mismatch = niedopasowanie wzorca
placeholder-invalid-placeholder = nieprawidłowa składnia zastępnika: { $details }
placeholder-invalid-pattern = nieprawidłowy wzorzec kroku: { $pattern }
placeholder-syntax = nieprawidłowa składnia zastępnika: { $details }
placeholder-syntax-detail = { $reason } przy bajcie { $position } (liczenie od zera){ $suffix }
placeholder-syntax-suffix = dla zastępnika « { $placeholder } »
step-context-ambiguous-override = Niejednoznaczne zastąpienie fiksury: więcej niż jedna fiksura odpowiada type_id { $type_id }. Zastąpienie pominięto.
panic-message-opaque-payload = <ładunek paniki bez możliwości debugowania typu { $type }>
assert-step-ok-panic = krok zwrócił błąd: { $error }
assert-step-err-success = krok zakończył się powodzeniem niespodziewanie
assert-step-err-missing-substring = błąd « { $display } » nie zawiera « { $expected } »

assert-skip-not-skipped = oczekiwano, że { $target } zarejestruje pominięty wynik
assert-skip-missing-message = oczekiwano, że { $target } dostarczy komunikat pominięcia zawierający « { $expected } »
assert-skip-missing-substring = komunikat pominięcia « { $actual } » nie zawiera « { $expected } »
assert-skip-unexpected-message = oczekiwano, że { $target } nie dostarczy komunikatu pominięcia
assert-skip-flag-mismatch = oczekiwano, że flaga « { $flag } » dla { $target } będzie równa { $expected }, lecz była { $actual }

execution-error-skip = Krok pominięty{ $has_message ->
    *[no] {""}
    [yes] : { $message }
}
execution-error-step-not-found = Nie znaleziono kroku o indeksie { $index }: { $keyword } { $text } (feature: { $feature_path }, scenariusz: { $scenario_name })
execution-error-missing-fixtures = Krok « { $step_pattern } » (zdefiniowany w { $step_location }) wymaga fiksur { $required }, ale brakuje: { $missing }. Dostępne fiksury ze scenariusza: { $available } (feature: { $feature_path }, scenariusz: { $scenario_name })
execution-error-handler-failed = Krok zakończony błędem o indeksie { $index }: { $keyword } { $text } - { $error } (feature: { $feature_path }, scenariusz: { $scenario_name })
