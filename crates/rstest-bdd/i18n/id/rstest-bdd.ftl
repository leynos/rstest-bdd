step-error-missing-fixture = Fixture « { $name } » bertipe « { $ty } » hilang untuk fungsi langkah « { $step } »
step-error-execution = Galat saat menjalankan langkah « { $pattern } » melalui fungsi « { $function } »: { $message }
step-error-panic = Panik pada langkah « { $pattern } », fungsi « { $function } »: { $message }
step-keyword-parse-error = kata kunci langkah tidak valid: { $keyword }
unsupported-step-type = jenis langkah tidak didukung: { $step_type }
step-pattern-not-compiled = regex pola langkah belum dikompilasi; panggil compile() terlebih dahulu pada pola « { $pattern } »
placeholder-pattern-mismatch = pola tidak cocok
placeholder-invalid-placeholder = sintaks placeholder tidak valid: { $details }
placeholder-invalid-pattern = pola langkah tidak valid: { $pattern }
placeholder-not-compiled = pola langkah « { $pattern } » harus dikompilasi sebelum digunakan
placeholder-syntax = sintaks placeholder tidak valid: { $details }
placeholder-syntax-detail = { $reason } pada byte { $position } (indeks awal nol){ $suffix }
placeholder-syntax-suffix = untuk placeholder « { $placeholder } »
step-context-ambiguous-override = Override fixture ambigu: lebih dari satu fixture cocok dengan type_id { $type_id }. Override diabaikan.
panic-message-opaque-payload = <payload panik non-debug bertipe { $type }>
assert-step-ok-panic = langkah mengembalikan galat: { $error }
assert-step-err-success = langkah berhasil secara tak terduga
assert-step-err-missing-substring = galat « { $display } » tidak memuat « { $expected } »

assert-skip-not-skipped = diharapkan { $target } merekam hasil yang dilewati
assert-skip-missing-message = diharapkan { $target } menyediakan pesan skip yang berisi '{ $expected }'
assert-skip-missing-substring = pesan skip '{ $actual }' tidak mengandung '{ $expected }'
assert-skip-unexpected-message = diharapkan { $target } tidak menyediakan pesan skip
assert-skip-flag-mismatch = diharapkan flag '{ $flag }' milik { $target } bernilai { $expected }, tetapi nilainya { $actual }

execution-error-skip = Langkah dilewati{ $has_message ->
    *[no] {""}
    [yes] : { $message }
}
execution-error-step-not-found = Langkah tidak ditemukan pada indeks { $index }: { $keyword } { $text } (fitur: { $feature_path }, skenario: { $scenario_name })
execution-error-missing-fixtures = Langkah « { $step_pattern } » (didefinisikan di { $step_location }) memerlukan fixture { $required }, tetapi yang berikut hilang: { $missing }. Fixture tersedia dari skenario: { $available } (fitur: { $feature_path }, skenario: { $scenario_name })
execution-error-handler-failed = Langkah gagal pada indeks { $index }: { $keyword } { $text } - { $error } (fitur: { $feature_path }, skenario: { $scenario_name })
