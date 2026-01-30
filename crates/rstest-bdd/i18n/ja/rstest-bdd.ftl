step-error-missing-fixture = ステップ関数「{ $step }」に必要な型「{ $ty }」のフィクスチャー「{ $name }」が見つかりません
step-error-execution = 関数「{ $function }」でステップ「{ $pattern }」を実行中にエラーが発生しました: { $message }
step-error-panic = ステップ「{ $pattern }」、関数「{ $function }」でパニックが発生しました: { $message }
step-keyword-parse-error = 無効なステップキーワードです: { $keyword }
unsupported-step-type = サポートされていないステップタイプです: { $step_type }
step-pattern-not-compiled = ステップパターンの正規表現が未コンパイルです。パターン「{ $pattern }」で先に compile() を呼び出してください
placeholder-pattern-mismatch = パターンが一致しません
placeholder-invalid-placeholder = 無効なプレースホルダー構文です: { $details }
placeholder-invalid-pattern = 無効なステップパターンです: { $pattern }
placeholder-not-compiled = ステップパターン「{ $pattern }」は使用前にコンパイルする必要があります
placeholder-syntax = 無効なプレースホルダー構文です: { $details }
placeholder-syntax-detail = { $reason } (0 起点) のバイト { $position } にあります{ $suffix }
placeholder-syntax-suffix = プレースホルダー「{ $placeholder }」に対して
step-context-ambiguous-override = フィクスチャーの上書きがあいまいです。複数のフィクスチャーが type_id { $type_id } に一致しました。上書きを無視しました。
panic-message-opaque-payload = <型 { $type } のデバッグ不可なパニックペイロード>
assert-step-ok-panic = ステップがエラーを返しました: { $error }
assert-step-err-success = ステップが予期せず成功しました
assert-step-err-missing-substring = エラー「{ $display }」に「{ $expected }」が含まれていません

assert-skip-not-skipped = { $target } がスキップされた結果を記録すると期待されました
assert-skip-missing-message = { $target } が '{ $expected }' を含むスキップメッセージを提供すると期待されました
assert-skip-missing-substring = スキップメッセージ '{ $actual }' には '{ $expected }' が含まれていません
assert-skip-unexpected-message = { $target } がスキップメッセージを提供しないと期待されました
assert-skip-flag-mismatch = { $target } のフラグ '{ $flag }' は { $expected } であると期待されましたが、実際は { $actual } でした

execution-error-skip = Step skipped{ $message ->
    [none] {""}
    *[other] : { $message }
}
execution-error-step-not-found = Step not found at index { $index }: { $keyword } { $text } (feature: { $feature_path }, scenario: { $scenario_name })
execution-error-missing-fixtures = Step '{ $step_pattern }' (defined at { $step_location }) requires fixtures { $required }, but the following are missing: { $missing }. Available fixtures from scenario: { $available } (feature: { $feature_path }, scenario: { $scenario_name })
execution-error-handler-failed = Step failed at index { $index }: { $keyword } { $text } - { $error } (feature: { $feature_path }, scenario: { $scenario_name })
