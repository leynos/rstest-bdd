step-error-missing-fixture = 缺少步骤函数「{ $step }」所需类型为「{ $ty }」的夹具「{ $name }」
step-error-execution = 通过函数「{ $function }」执行步骤「{ $pattern }」时出错：{ $message }
step-error-panic = 步骤「{ $pattern }」中的函数「{ $function }」发生 panic：{ $message }
step-keyword-parse-error = 无效的步骤关键字：{ $keyword }
unsupported-step-type = 不支持的步骤类型：{ $step_type }
placeholder-pattern-mismatch = 模式不匹配
placeholder-invalid-placeholder = 无效的占位符语法：{ $details }
placeholder-invalid-pattern = 无效的步骤模式：{ $pattern }
placeholder-syntax = 无效的占位符语法：{ $details }
placeholder-syntax-detail = { $reason } 位于字节 { $position }（从零开始）{ $suffix }
placeholder-syntax-suffix = 针对占位符「{ $placeholder }」
step-context-ambiguous-override = 夹具覆盖存在歧义：多个夹具匹配 type_id { $type_id }。已忽略覆盖。
panic-message-opaque-payload = <类型为 { $type } 的不可调试 panic 负载>
assert-step-ok-panic = 步骤返回错误：{ $error }
assert-step-err-success = 步骤意外成功
assert-step-err-missing-substring = 错误「{ $display }」不包含「{ $expected }」

assert-skip-not-skipped = 期望 { $target } 记录为已跳过的结果
assert-skip-missing-message = 期望 { $target } 提供包含 '{ $expected }' 的跳过信息
assert-skip-missing-substring = 跳过信息 '{ $actual }' 不包含 '{ $expected }'
assert-skip-unexpected-message = 期望 { $target } 不提供跳过信息
assert-skip-flag-mismatch = 期望 { $target } 标志 '{ $flag }' 为 { $expected }，但实际为 { $actual }

execution-error-skip = 步骤已跳过{ $has_message ->
    *[no] {""}
    [yes] ：{ $message }
}
execution-error-step-not-found = 索引 { $index } 处未找到步骤：{ $keyword } { $text }（功能：{ $feature_path }，场景：{ $scenario_name }）
execution-error-missing-fixtures = 步骤「{ $step_pattern }」（定义于 { $step_location }）需要夹具 { $required }，但以下夹具缺失：{ $missing }。场景中可用的夹具：{ $available }（功能：{ $feature_path }，场景：{ $scenario_name }）
execution-error-handler-failed = 步骤在索引 { $index } 处执行失败：{ $keyword } { $text } - { $error }（功能：{ $feature_path }，场景：{ $scenario_name }）
