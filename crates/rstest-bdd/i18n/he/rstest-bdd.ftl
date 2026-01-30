step-error-missing-fixture = הקיבוע "{ $name }" מסוג "{ $ty }" חסר עבור פונקציית הצעד "{ $step }"
step-error-execution = שגיאה בעת ביצוע הצעד "{ $pattern }" דרך הפונקציה "{ $function }": { $message }
step-error-panic = פאניקה בצעד "{ $pattern }", פונקציה "{ $function }": { $message }
step-keyword-parse-error = מילת צעד לא תקינה: { $keyword }
unsupported-step-type = סוג צעד לא נתמך: { $step_type }
step-pattern-not-compiled = ביטוי רגולרי של תבנית הצעד לא קוּמפל; יש לקרוא תחילה ל-compile() על התבנית "{ $pattern }"
placeholder-pattern-mismatch = התבנית אינה תואמת
placeholder-invalid-placeholder = תחביר לא תקין של מציין מקום: { $details }
placeholder-invalid-pattern = תבנית צעד לא תקינה: { $pattern }
placeholder-not-compiled = יש לקמפל את תבנית הצעד "{ $pattern }" לפני השימוש
placeholder-syntax = תחביר לא תקין של מציין מקום: { $details }
placeholder-syntax-detail = { $reason } בבייט { $position } (ספירה מאפס){ $suffix }
placeholder-syntax-suffix = עבור מציין המקום "{ $placeholder }"
step-context-ambiguous-override = עקיפת קיבוע דו-משמעית: יותר מקיבוע אחד תואם את type_id { $type_id }. העקיפה התעלמה.
panic-message-opaque-payload = <מטען פאניקה לא ניתן לניפוי מסוג { $type }>
assert-step-ok-panic = הצעד החזיר שגיאה: { $error }
assert-step-err-success = הצעד הצליח באופן בלתי צפוי
assert-step-err-missing-substring = השגיאה "{ $display }" אינה מכילה את "{ $expected }"

assert-skip-not-skipped = ציפינו ש-{ $target } יתעד תוצאה שדלגה על הצעד
assert-skip-missing-message = ציפינו ש-{ $target } יספק הודעת דילוג המכילה את '{ $expected }'
assert-skip-missing-substring = הודעת הדילוג '{ $actual }' אינה מכילה את '{ $expected }'
assert-skip-unexpected-message = ציפינו ש-{ $target } לא יספק הודעת דילוג
assert-skip-flag-mismatch = ציפינו שדגל '{ $flag }' אצל { $target } יהיה { $expected }, אך היה { $actual }

execution-error-skip = Step skipped{ $message ->
    [none] {""}
    *[other] : { $message }
}
execution-error-step-not-found = Step not found at index { $index }: { $keyword } { $text } (feature: { $feature_path }, scenario: { $scenario_name })
execution-error-missing-fixtures = Step '{ $step_pattern }' (defined at { $step_location }) requires fixtures { $required }, but the following are missing: { $missing }. Available fixtures from scenario: { $available } (feature: { $feature_path }, scenario: { $scenario_name })
execution-error-handler-failed = Step failed at index { $index }: { $keyword } { $text } - { $error } (feature: { $feature_path }, scenario: { $scenario_name })
