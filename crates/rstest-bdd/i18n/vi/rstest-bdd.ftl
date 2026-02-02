step-error-missing-fixture = Thiếu fixture « { $name } » có kiểu « { $ty } » cho hàm bước « { $step } »
step-error-execution = Lỗi khi thực thi bước « { $pattern } » thông qua hàm « { $function } »: { $message }
step-error-panic = Hoảng loạn trong bước « { $pattern } », hàm « { $function } »: { $message }
step-keyword-parse-error = từ khóa bước không hợp lệ: { $keyword }
unsupported-step-type = kiểu bước không được hỗ trợ: { $step_type }
step-pattern-not-compiled = regex của mẫu bước chưa được biên dịch; hãy gọi compile() trước với mẫu « { $pattern } »
placeholder-pattern-mismatch = mẫu không khớp
placeholder-invalid-placeholder = cú pháp placeholder không hợp lệ: { $details }
placeholder-invalid-pattern = mẫu bước không hợp lệ: { $pattern }
placeholder-syntax = cú pháp placeholder không hợp lệ: { $details }
placeholder-syntax-detail = { $reason } tại byte { $position } (đếm từ 0){ $suffix }
placeholder-syntax-suffix = cho placeholder « { $placeholder } »
step-context-ambiguous-override = Ghi đè fixture mơ hồ: nhiều hơn một fixture khớp với type_id { $type_id }. Bỏ qua ghi đè.
panic-message-opaque-payload = <payload panic không thể gỡ lỗi của kiểu { $type }>
assert-step-ok-panic = bước trả về lỗi: { $error }
assert-step-err-success = bước thành công ngoài mong đợi
assert-step-err-missing-substring = lỗi « { $display } » không chứa « { $expected } »

assert-skip-not-skipped = mong đợi { $target } được ghi nhận là bị bỏ qua
assert-skip-missing-message = mong đợi { $target } cung cấp thông báo bỏ qua chứa '{ $expected }'
assert-skip-missing-substring = thông báo bỏ qua '{ $actual }' không chứa '{ $expected }'
assert-skip-unexpected-message = mong đợi { $target } không cung cấp thông báo bỏ qua
assert-skip-flag-mismatch = mong đợi cờ '{ $flag }' của { $target } là { $expected }, nhưng thực tế là { $actual }

execution-error-skip = Bước đã bỏ qua{ $has_message ->
    *[no] {""}
    [yes] : { $message }
}
execution-error-step-not-found = Không tìm thấy bước tại chỉ mục { $index }: { $keyword } { $text } (tính năng: { $feature_path }, kịch bản: { $scenario_name })
execution-error-missing-fixtures = Bước « { $step_pattern } » (định nghĩa tại { $step_location }) yêu cầu fixture { $required }, nhưng các fixture sau bị thiếu: { $missing }. Fixture có sẵn từ kịch bản: { $available } (tính năng: { $feature_path }, kịch bản: { $scenario_name })
execution-error-handler-failed = Bước thất bại tại chỉ mục { $index }: { $keyword } { $text } - { $error } (tính năng: { $feature_path }, kịch bản: { $scenario_name })
