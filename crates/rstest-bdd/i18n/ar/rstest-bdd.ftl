step-error-missing-fixture = التجهيز « { $name } » من النوع « { $ty } » مفقود لدالة الخطوة « { $step } »
step-error-execution = حدث خطأ أثناء تنفيذ الخطوة « { $pattern } » عبر الدالة « { $function } »: { $message }
step-error-panic = ذعر في الخطوة « { $pattern } »، الدالة « { $function } »: { $message }
step-keyword-parse-error = كلمة خطوة مفتاحية غير صالحة: { $keyword }
unsupported-step-type = نوع خطوة غير مدعوم: { $step_type }
step-pattern-not-compiled = لم يتم تجميع تعبير نمط الخطوة؛ استدعِ compile() أولًا للنمط « { $pattern } »
placeholder-pattern-mismatch = عدم تطابق في النمط
placeholder-invalid-placeholder = صياغة عنصر نائب غير صالحة: { $details }
placeholder-invalid-pattern = نمط خطوة غير صالح: { $pattern }
placeholder-not-compiled = يجب تجميع نمط الخطوة « { $pattern } » قبل الاستخدام
placeholder-syntax = صياغة عنصر نائب غير صالحة: { $details }
placeholder-syntax-detail = { $reason } عند البايت { $position } (يبدأ العد من الصفر){ $suffix }
placeholder-syntax-suffix = للعنصر النائب « { $placeholder } »
step-context-ambiguous-override = تجاوز تجهيز ملتبس: أكثر من تجهيز واحد يطابق type_id { $type_id }. تم تجاهل التجاوز.
panic-message-opaque-payload = <حمولة ذعر غير قابلة للتصحيح من النوع { $type }>
assert-step-ok-panic = أعادت الخطوة خطأً: { $error }
assert-step-err-success = نجحت الخطوة على نحو غير متوقع
assert-step-err-missing-substring = الخطأ « { $display } » لا يحتوي على « { $expected } »

assert-skip-not-skipped = كان من المتوقع أن يسجّل { $target } نتيجةً متخطّاة
assert-skip-missing-message = كان من المتوقع أن يوفّر { $target } رسالة تخطي تحتوي على « { $expected } »
assert-skip-missing-substring = رسالة التخطي « { $actual } » لا تحتوي على « { $expected } »
assert-skip-unexpected-message = كان من المتوقع ألا يقدّم { $target } رسالة تخطي
assert-skip-flag-mismatch = كان من المتوقع أن يكون علم { $target } « { $flag } » مساويًا لـ { $expected }، لكنه كان { $actual }

execution-error-skip = Step skipped{ $message ->
    [none] {""}
    *[other] : { $message }
}
execution-error-step-not-found = Step not found at index { $index }: { $keyword } { $text } (feature: { $feature_path }, scenario: { $scenario_name })
execution-error-missing-fixtures = Step '{ $step_pattern }' (defined at { $step_location }) requires fixtures { $required }, but the following are missing: { $missing }. Available fixtures from scenario: { $available } (feature: { $feature_path }, scenario: { $scenario_name })
execution-error-handler-failed = Step failed at index { $index }: { $keyword } { $text } - { $error } (feature: { $feature_path }, scenario: { $scenario_name })
