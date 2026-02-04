step-error-missing-fixture = La fixture « { $name } » de type « { $ty } » est introuvable pour la fonction « { $step } »
step-error-execution = Erreur lors de l'exécution de l'étape « { $pattern } » via la fonction « { $function } » : { $message }
step-error-panic = Panique dans l'étape « { $pattern } », fonction « { $function } » : { $message }
step-keyword-parse-error = mot-clé d'étape invalide : { $keyword }
unsupported-step-type = type d'étape non pris en charge : { $step_type }
placeholder-pattern-mismatch = le motif ne correspond pas
placeholder-invalid-placeholder = syntaxe de paramètre invalide : { $details }
placeholder-invalid-pattern = motif d'étape invalide : { $pattern }
placeholder-syntax = syntaxe de paramètre invalide : { $details }
placeholder-syntax-detail = { $reason } à l'octet { $position } (indexé depuis zéro){ $suffix }
placeholder-syntax-suffix = pour le paramètre « { $placeholder } »
step-context-ambiguous-override = Remplacement de fixture ambigu : plusieurs fixtures correspondent au type_id { $type_id }. Remplacement ignoré.
panic-message-opaque-payload = <charge utile de panique non affichable de type { $type }>
assert-step-ok-panic = l'étape a renvoyé une erreur : { $error }
assert-step-err-success = l'étape a réussi de manière inattendue
assert-step-err-missing-substring = l'erreur « { $display } » ne contient pas « { $expected } »

assert-skip-not-skipped = { $target } n'aurait pas dû signaler une étape ignorée
assert-skip-missing-message = { $target } devait fournir un message de saut contenant « { $expected } »
assert-skip-missing-substring = le message de saut « { $actual } » ne contient pas « { $expected } »
assert-skip-unexpected-message = { $target } ne devait pas fournir de message de saut
assert-skip-flag-mismatch = le drapeau « { $flag } » pour { $target } devait valoir { $expected }, mais vaut { $actual }

execution-error-skip = Étape ignorée{ $has_message ->
    *[no] {""}
    [yes] : { $message }
}
execution-error-step-not-found = Étape introuvable à l'index { $index } : { $keyword } { $text } (feature : { $feature_path }, scénario : { $scenario_name })
execution-error-missing-fixtures = L'étape « { $step_pattern } » (définie à { $step_location }) requiert les fixtures { $required }, mais les suivantes sont manquantes : { $missing }. Fixtures disponibles depuis le scénario : { $available } (feature : { $feature_path }, scénario : { $scenario_name })
execution-error-handler-failed = Étape échouée à l'index { $index } : { $keyword } { $text } - { $error } (feature : { $feature_path }, scénario : { $scenario_name })
