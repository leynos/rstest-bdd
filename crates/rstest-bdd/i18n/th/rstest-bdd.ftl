step-error-missing-fixture = ขาดฟิกซ์เจอร์ « { $name } » ชนิด « { $ty } » สำหรับฟังก์ชันขั้นตอน « { $step } »
step-error-execution = เกิดข้อผิดพลาดขณะรันขั้นตอน « { $pattern } » ผ่านฟังก์ชัน « { $function } »: { $message }
step-error-panic = เกิดแพนิคในขั้นตอน « { $pattern } » ฟังก์ชัน « { $function } »: { $message }
step-keyword-parse-error = คีย์เวิร์ดของขั้นตอนไม่ถูกต้อง: { $keyword }
unsupported-step-type = ชนิดของขั้นตอนไม่รองรับ: { $step_type }
placeholder-pattern-mismatch = แพตเทิร์นไม่ตรงกัน
placeholder-invalid-placeholder = ไวยากรณ์ของตัวยึดตำแหน่งไม่ถูกต้อง: { $details }
placeholder-invalid-pattern = แพตเทิร์นของขั้นตอนไม่ถูกต้อง: { $pattern }
placeholder-syntax = ไวยากรณ์ของตัวยึดตำแหน่งไม่ถูกต้อง: { $details }
placeholder-syntax-detail = { $reason } ที่ไบต์ { $position } (เริ่มนับจากศูนย์){ $suffix }
placeholder-syntax-suffix = สำหรับตัวยึดตำแหน่ง « { $placeholder } »
step-context-ambiguous-override = การเขียนทับฟิกซ์เจอร์ไม่ชัดเจน: มีฟิกซ์เจอร์มากกว่าหนึ่งตัวที่ตรงกับ type_id { $type_id } ข้ามการเขียนทับ
panic-message-opaque-payload = <เพย์โหลดแพนิคที่ดีบักไม่ได้ ชนิด { $type }>
assert-step-ok-panic = ขั้นตอนส่งคืนข้อผิดพลาด: { $error }
assert-step-err-success = ขั้นตอนสำเร็จโดยไม่คาดคิด
assert-step-err-missing-substring = ข้อผิดพลาด « { $display } » ไม่มี « { $expected } »

assert-skip-not-skipped = คาดหวังให้ { $target } บันทึกผลลัพธ์ที่ถูกข้าม
assert-skip-missing-message = คาดหวังให้ { $target } ให้ข้อความการข้ามที่มี '{ $expected }'
assert-skip-missing-substring = ข้อความการข้าม '{ $actual }' ไม่มี '{ $expected }'
assert-skip-unexpected-message = คาดหวังให้ { $target } ไม่ให้ข้อความการข้าม
assert-skip-flag-mismatch = คาดหวังให้ธง '{ $flag }' ของ { $target } เป็น { $expected } แต่กลับเป็น { $actual }

execution-error-skip = ขั้นตอนถูกข้าม{ $has_message ->
    *[no] {""}
    [yes] : { $message }
}
execution-error-step-not-found = ไม่พบขั้นตอนที่ดัชนี { $index }: { $keyword } { $text } (ฟีเจอร์: { $feature_path }, สถานการณ์: { $scenario_name })
execution-error-missing-fixtures = ขั้นตอน « { $step_pattern } » (กำหนดที่ { $step_location }) ต้องการฟิกซ์เจอร์ { $required } แต่สิ่งต่อไปนี้หายไป: { $missing } ฟิกซ์เจอร์ที่มีจากสถานการณ์: { $available } (ฟีเจอร์: { $feature_path }, สถานการณ์: { $scenario_name })
execution-error-handler-failed = ขั้นตอนล้มเหลวที่ดัชนี { $index }: { $keyword } { $text } - { $error } (ฟีเจอร์: { $feature_path }, สถานการณ์: { $scenario_name })
