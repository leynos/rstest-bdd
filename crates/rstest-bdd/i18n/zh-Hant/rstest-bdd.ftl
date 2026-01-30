step-error-missing-fixture = 缺少步驟函式「{ $step }」所需型別為「{ $ty }」的治具「{ $name }」
step-error-execution = 透過函式「{ $function }」執行步驟「{ $pattern }」時發生錯誤：{ $message }
step-error-panic = 步驟「{ $pattern }」中的函式「{ $function }」發生 panic：{ $message }
step-keyword-parse-error = 無效的步驟關鍵字：{ $keyword }
unsupported-step-type = 不支援的步驟型別：{ $step_type }
step-pattern-not-compiled = 步驟樣式的正規表示式尚未編譯；請先在樣式「{ $pattern }」上呼叫 compile()
placeholder-pattern-mismatch = 樣式不相符
placeholder-invalid-placeholder = 無效的參數語法：{ $details }
placeholder-invalid-pattern = 無效的步驟樣式：{ $pattern }
placeholder-not-compiled = 步驟樣式「{ $pattern }」在使用前必須編譯
placeholder-syntax = 無效的參數語法：{ $details }
placeholder-syntax-detail = { $reason } 位於位元組 { $position }（從零開始）{ $suffix }
placeholder-syntax-suffix = 針對參數「{ $placeholder }」
step-context-ambiguous-override = 治具覆寫有歧義：多個治具符合 type_id { $type_id }。已忽略覆寫。
panic-message-opaque-payload = <型別為 { $type } 的不可除錯 panic 載荷>
assert-step-ok-panic = 步驟回傳錯誤：{ $error }
assert-step-err-success = 步驟意外成功
assert-step-err-missing-substring = 錯誤「{ $display }」不包含「{ $expected }」

assert-skip-not-skipped = 預期 { $target } 記錄為已略過的結果
assert-skip-missing-message = 預期 { $target } 提供包含「{ $expected }」的略過訊息
assert-skip-missing-substring = 略過訊息「{ $actual }」不包含「{ $expected }」
assert-skip-unexpected-message = 預期 { $target } 不會提供略過訊息
assert-skip-flag-mismatch = 預期 { $target } 的旗標「{ $flag }」為 { $expected }，但實際為 { $actual }

execution-error-skip = Step skipped{ $message ->
    [none] {""}
    *[other] : { $message }
}
execution-error-step-not-found = Step not found at index { $index }: { $keyword } { $text } (feature: { $feature_path }, scenario: { $scenario_name })
execution-error-missing-fixtures = Step '{ $step_pattern }' (defined at { $step_location }) requires fixtures { $required }, but the following are missing: { $missing }. Available fixtures from scenario: { $available } (feature: { $feature_path }, scenario: { $scenario_name })
execution-error-handler-failed = Step failed at index { $index }: { $keyword } { $text } - { $error } (feature: { $feature_path }, scenario: { $scenario_name })
