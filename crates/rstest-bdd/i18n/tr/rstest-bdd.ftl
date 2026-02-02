step-error-missing-fixture = « { $step } » adım işlevi için « { $ty } » türündeki « { $name } » fikstürü eksik
step-error-execution = « { $pattern } » adımı « { $function } » işleviyle çalıştırılırken hata: { $message }
step-error-panic = « { $pattern } » adımında, « { $function } » işlevinde panik: { $message }
step-keyword-parse-error = geçersiz adım anahtar kelimesi: { $keyword }
unsupported-step-type = desteklenmeyen adım türü: { $step_type }
step-pattern-not-compiled = adım deseninin regex'i derlenmedi; önce « { $pattern } » deseni için compile() çağırın
placeholder-pattern-mismatch = desen eşleşmedi
placeholder-invalid-placeholder = geçersiz yer tutucu söz dizimi: { $details }
placeholder-invalid-pattern = geçersiz adım deseni: { $pattern }
placeholder-syntax = geçersiz yer tutucu söz dizimi: { $details }
placeholder-syntax-detail = { $reason } { $position } baytında (sıfır tabanlı){ $suffix }
placeholder-syntax-suffix = « { $placeholder } » yer tutucusu için
step-context-ambiguous-override = Belirsiz fikstür geçersiz kılma: Birden fazla fikstür type_id { $type_id } ile eşleşiyor. Geçersiz kılma yok sayıldı.
panic-message-opaque-payload = <hata ayıklaması yapılamayan { $type } türünde panik yükü>
assert-step-ok-panic = adım hata döndürdü: { $error }
assert-step-err-success = adım beklenmedik şekilde başarılı oldu
assert-step-err-missing-substring = « { $display } » hatası « { $expected } » ifadesini içermiyor

assert-skip-not-skipped = { $target } öğesinin atlanan bir sonucu kaydetmesi bekleniyordu
assert-skip-missing-message = { $target } öğesinin '{ $expected }' içeren bir atlama mesajı sağlaması bekleniyordu
assert-skip-missing-substring = atlama mesajı '{ $actual }' '{ $expected }' içermiyor
assert-skip-unexpected-message = { $target } öğesinin atlama mesajı sağlamaması bekleniyordu
assert-skip-flag-mismatch = { $target } öğesinin '{ $flag }' bayrağının { $expected } olması bekleniyordu, ancak { $actual } idi

execution-error-skip = Adım atlandı{ $has_message ->
    *[no] {""}
    [yes] : { $message }
}
execution-error-step-not-found = { $index } indeksinde adım bulunamadı: { $keyword } { $text } (özellik: { $feature_path }, senaryo: { $scenario_name })
execution-error-missing-fixtures = « { $step_pattern } » adımı ({ $step_location } konumunda tanımlanmış) { $required } fikstürlerini gerektiriyor, ancak şunlar eksik: { $missing }. Senaryodan mevcut fikstürler: { $available } (özellik: { $feature_path }, senaryo: { $scenario_name })
execution-error-handler-failed = { $index } indeksinde adım başarısız oldu: { $keyword } { $text } - { $error } (özellik: { $feature_path }, senaryo: { $scenario_name })
