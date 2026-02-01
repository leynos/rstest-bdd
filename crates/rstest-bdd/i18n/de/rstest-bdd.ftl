step-error-missing-fixture = Fehlende Fixture „{ $name }" vom Typ „{ $ty }" für die Schritt-Funktion „{ $step }"
step-error-execution = Fehler beim Ausführen des Schritts „{ $pattern }" über die Funktion „{ $function }": { $message }
step-error-panic = Panic im Schritt „{ $pattern }", Funktion „{ $function }": { $message }
step-keyword-parse-error = ungültiges Schritt-Stichwort: { $keyword }
unsupported-step-type = nicht unterstützter Schritt-Typ: { $step_type }
step-pattern-not-compiled = Regulärer Ausdruck des Schritt-Musters wurde nicht kompiliert; rufen Sie zunächst compile() für das Muster „{ $pattern }" auf
placeholder-pattern-mismatch = Muster stimmt nicht überein
placeholder-invalid-placeholder = ungültige Platzhalter-Syntax: { $details }
placeholder-invalid-pattern = ungültiges Schritt-Muster: { $pattern }
placeholder-not-compiled = Schritt-Muster „{ $pattern }" muss vor der Verwendung kompiliert werden
placeholder-syntax = ungültige Platzhalter-Syntax: { $details }
placeholder-syntax-detail = { $reason } bei Byte { $position } (Null-basiert){ $suffix }
placeholder-syntax-suffix = für den Platzhalter „{ $placeholder }"
step-context-ambiguous-override = Mehrdeutiges Fixture-Override: Mehr als eine Fixture passt zu type_id { $type_id }. Override ignoriert.
panic-message-opaque-payload = <nicht debuggbarer Panic-Payload des Typs { $type }>
assert-step-ok-panic = Schritt gab einen Fehler zurück: { $error }
assert-step-err-success = Schritt war unerwartet erfolgreich
assert-step-err-missing-substring = Fehler „{ $display }" enthält „{ $expected }" nicht

assert-skip-not-skipped = Es wurde erwartet, dass { $target } ein übersprungenes Ergebnis protokolliert
assert-skip-missing-message = Es wurde erwartet, dass { $target } eine Skip-Nachricht mit '{ $expected }' bereitstellt
assert-skip-missing-substring = Skip-Nachricht '{ $actual }' enthält '{ $expected }' nicht
assert-skip-unexpected-message = Es wurde erwartet, dass { $target } keine Skip-Nachricht bereitstellt
assert-skip-flag-mismatch = Es wurde erwartet, dass das Flag '{ $flag }' von { $target } { $expected } ist, war jedoch { $actual }

execution-error-skip = Schritt übersprungen{ $has_message ->
    *[no] {""}
    [yes] : { $message }
}
execution-error-step-not-found = Schritt nicht gefunden bei Index { $index }: { $keyword } { $text } (Feature: { $feature_path }, Szenario: { $scenario_name })
execution-error-missing-fixtures = Schritt „{ $step_pattern }“ (definiert bei { $step_location }) benötigt Fixtures { $required }, aber folgende fehlen: { $missing }. Verfügbare Fixtures aus dem Szenario: { $available } (Feature: { $feature_path }, Szenario: { $scenario_name })
execution-error-handler-failed = Schritt fehlgeschlagen bei Index { $index }: { $keyword } { $text } - { $error } (Feature: { $feature_path }, Szenario: { $scenario_name })
