step-error-missing-fixture = فیکسچر «{ $name }» از نوع «{ $ty }» برای تابع گام «{ $step }» وجود ندارد
step-error-execution = خطا هنگام اجرای گام «{ $pattern }» با تابع «{ $function }»: { $message }
step-error-panic = پانیک در گام «{ $pattern }»، تابع «{ $function }»: { $message }
step-keyword-parse-error = کلیدواژه گام نامعتبر است: { $keyword }
unsupported-step-type = نوع گام پشتیبانی نمی‌شود: { $step_type }
step-pattern-not-compiled = عبارت منظم الگوی گام کامپایل نشده است؛ ابتدا روی الگو «{ $pattern }» تابع compile() را فراخوانی کنید
placeholder-pattern-mismatch = الگو مطابق نیست
placeholder-invalid-placeholder = نحو جای‌نگهدار نامعتبر است: { $details }
placeholder-invalid-pattern = الگوی گام نامعتبر است: { $pattern }
placeholder-not-compiled = الگوی گام «{ $pattern }» باید پیش از استفاده کامپایل شود
placeholder-syntax = نحو جای‌نگهدار نامعتبر است: { $details }
placeholder-syntax-detail = { $reason } در بایت { $position } (شمارش از صفر){ $suffix }
placeholder-syntax-suffix = برای جای‌نگهدار «{ $placeholder }»
step-context-ambiguous-override = بازنویسی فیکسچر مبهم است: بیش از یک فیکسچر با type_id { $type_id } مطابقت دارد. بازنویسی نادیده گرفته شد.
panic-message-opaque-payload = <بار پانیک غیرقابل اشکال‌زدایی از نوع { $type }>
assert-step-ok-panic = گام خطایی برگرداند: { $error }
assert-step-err-success = گام به‌طور غیرمنتظره موفق شد
assert-step-err-missing-substring = خطای «{ $display }» شامل «{ $expected }» نیست

assert-skip-not-skipped = انتظار می‌رفت { $target } یک نتیجهٔ ردشده را ثبت کند
assert-skip-missing-message = انتظار می‌رفت { $target } پیام رد کردنی ارائه کند که شامل «{ $expected }» باشد
assert-skip-missing-substring = پیام رد کردن «{ $actual }» شامل «{ $expected }» نیست
assert-skip-unexpected-message = انتظار می‌رفت { $target } پیام رد کردنی ارائه ندهد
assert-skip-flag-mismatch = انتظار می‌رفت پرچم '{ $flag }' در { $target } برابر با { $expected } باشد، اما { $actual } بود

execution-error-skip = Step skipped{ $message ->
    [none] {""}
    *[other] : { $message }
}
execution-error-step-not-found = Step not found at index { $index }: { $keyword } { $text } (feature: { $feature_path }, scenario: { $scenario_name })
execution-error-missing-fixtures = Step '{ $step_pattern }' (defined at { $step_location }) requires fixtures { $required }, but the following are missing: { $missing }. Available fixtures from scenario: { $available } (feature: { $feature_path }, scenario: { $scenario_name })
execution-error-handler-failed = Step failed at index { $index }: { $keyword } { $text } - { $error } (feature: { $feature_path }, scenario: { $scenario_name })
