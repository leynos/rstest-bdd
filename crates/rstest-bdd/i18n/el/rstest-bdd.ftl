step-error-missing-fixture = Λείπει το fixture «{ $name }» τύπου «{ $ty }» για τη συνάρτηση βήματος «{ $step }»
step-error-execution = Σφάλμα κατά την εκτέλεση του βήματος «{ $pattern }» μέσω της συνάρτησης «{ $function }»: { $message }
step-error-panic = Πανικός στο βήμα «{ $pattern }», συνάρτηση «{ $function }»: { $message }
step-keyword-parse-error = μη έγκυρη λέξη-κλειδί βήματος: { $keyword }
unsupported-step-type = μη υποστηριζόμενος τύπος βήματος: { $step_type }
step-pattern-not-compiled = η κανονική έκφραση του προτύπου βήματος δεν έχει μεταγλωττιστεί· καλέστε πρώτα τη compile() στο πρότυπο «{ $pattern }»
placeholder-pattern-mismatch = το πρότυπο δεν ταιριάζει
placeholder-invalid-placeholder = μη έγκυρη σύνταξη συμβόλου θέσης: { $details }
placeholder-invalid-pattern = μη έγκυρο πρότυπο βήματος: { $pattern }
placeholder-not-compiled = το πρότυπο βήματος «{ $pattern }» πρέπει να μεταγλωττιστεί πριν τη χρήση
placeholder-syntax = μη έγκυρη σύνταξη συμβόλου θέσης: { $details }
placeholder-syntax-detail = { $reason } στο byte { $position } (αρίθμηση από το μηδέν){ $suffix }
placeholder-syntax-suffix = για το σύμβολο θέσης «{ $placeholder }»
step-context-ambiguous-override = Διφορούμενη αντικατάσταση fixture: περισσότερα από ένα fixtures αντιστοιχούν στο type_id { $type_id }. Η αντικατάσταση αγνοήθηκε.
panic-message-opaque-payload = <μη ορατό φορτίο πανικού τύπου { $type }>
assert-step-ok-panic = το βήμα επέστρεψε σφάλμα: { $error }
assert-step-err-success = το βήμα ολοκληρώθηκε απροσδόκητα με επιτυχία
assert-step-err-missing-substring = το σφάλμα «{ $display }» δεν περιέχει «{ $expected }»

assert-skip-not-skipped = αναμενόταν το { $target } να καταγράψει ένα αποτέλεσμα παράκαμψης
assert-skip-missing-message = αναμενόταν το { $target } να παρέχει μήνυμα παράκαμψης που να περιέχει «{ $expected }»
assert-skip-missing-substring = το μήνυμα παράκαμψης «{ $actual }» δεν περιέχει «{ $expected }»
assert-skip-unexpected-message = αναμενόταν το { $target } να μη δώσει μήνυμα παράκαμψης
assert-skip-flag-mismatch = αναμενόταν η σημαία «{ $flag }» του { $target } να είναι { $expected }, αλλά ήταν { $actual }

execution-error-skip = Step skipped{ $message ->
    [none] {""}
    *[other] : { $message }
}
execution-error-step-not-found = Step not found at index { $index }: { $keyword } { $text } (feature: { $feature_path }, scenario: { $scenario_name })
execution-error-missing-fixtures = Step '{ $step_pattern }' (defined at { $step_location }) requires fixtures { $required }, but the following are missing: { $missing }. Available fixtures from scenario: { $available } (feature: { $feature_path }, scenario: { $scenario_name })
execution-error-handler-failed = Step failed at index { $index }: { $keyword } { $text } - { $error } (feature: { $feature_path }, scenario: { $scenario_name })
