step-error-missing-fixture = הקיבוע "{ $name }" מסוג "{ $ty }" חסר עבור פונקציית הצעד "{ $step }"
step-error-execution = שגיאה בעת ביצוע הצעד "{ $pattern }" דרך הפונקציה "{ $function }": { $message }
step-error-panic = פאניקה בצעד "{ $pattern }", פונקציה "{ $function }": { $message }
step-keyword-parse-error = מילת צעד לא תקינה: { $keyword }
unsupported-step-type = סוג צעד לא נתמך: { $step_type }
step-pattern-not-compiled = ביטוי רגולרי של תבנית הצעד לא קוּמפל; יש לקרוא תחילה ל-compile() על התבנית "{ $pattern }"
placeholder-pattern-mismatch = התבנית אינה תואמת
placeholder-invalid-placeholder = תחביר לא תקין של מציין מקום: { $details }
placeholder-invalid-pattern = תבנית צעד לא תקינה: { $pattern }
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

execution-error-skip = הצעד דולג{ $has_message ->
    *[no] {""}
    [yes] : { $message }
}
execution-error-step-not-found = הצעד לא נמצא באינדקס { $index }: { $keyword } { $text } (תכונה: { $feature_path }, תרחיש: { $scenario_name })
execution-error-missing-fixtures = הצעד "{ $step_pattern }" (מוגדר ב-{ $step_location }) דורש קיבועים { $required }, אך הבאים חסרים: { $missing }. קיבועים זמינים מהתרחיש: { $available } (תכונה: { $feature_path }, תרחיש: { $scenario_name })
execution-error-handler-failed = הצעד נכשל באינדקס { $index }: { $keyword } { $text } - { $error } (תכונה: { $feature_path }, תרחיש: { $scenario_name })
