step-error-missing-fixture = Fixtuuri "{ $name }" tyyppiä "{ $ty }" puuttuu askelfunktiolle "{ $step }"
step-error-execution = Virhe suoritettaessa askelta "{ $pattern }" funktion "{ $function }" kautta: { $message }
step-error-panic = Paniikki askeleessa "{ $pattern }", funktiossa "{ $function }": { $message }
step-keyword-parse-error = virheellinen askelen avainsana: { $keyword }
unsupported-step-type = askeltyyppiä { $step_type } ei tueta
step-pattern-not-compiled = askelkuvion regexiä ei ole käännetty; kutsu compile() ensin kuviolle "{ $pattern }"
placeholder-pattern-mismatch = kuvio ei täsmää
placeholder-invalid-placeholder = virheellinen paikkamerkkisyntaksi: { $details }
placeholder-invalid-pattern = virheellinen askelkuvio: { $pattern }
placeholder-syntax = virheellinen paikkamerkkisyntaksi: { $details }
placeholder-syntax-detail = { $reason } tavussa { $position } (nollapohjainen){ $suffix }
placeholder-syntax-suffix = paikkamerkille "{ $placeholder }"
step-context-ambiguous-override = Moniselitteinen fixtuurin korvaaminen: useampi fixtuuri vastaa type_id:tä { $type_id }. Korvaus ohitettiin.
panic-message-opaque-payload = <läpinäkymätön paniikin hyötykuorma tyyppiä { $type }>
assert-step-ok-panic = askel palautti virheen: { $error }
assert-step-err-success = askel onnistui odottamatta
assert-step-err-missing-substring = virhe "{ $display }" ei sisällä "{ $expected }"

assert-skip-not-skipped = odotettiin, että { $target } kirjaisi ohitetun lopputuloksen
assert-skip-missing-message = odotettiin, että { $target } antaa ohitusviestin, joka sisältää '{ $expected }'
assert-skip-missing-substring = ohitusviesti '{ $actual }' ei sisällä '{ $expected }'
assert-skip-unexpected-message = odotettiin, että { $target } ei anna ohitusviestiä
assert-skip-flag-mismatch = odotettiin, että { $target } -lipun '{ $flag }' arvo olisi { $expected }, mutta se oli { $actual }

execution-error-skip = Askel ohitettu{ $has_message ->
    *[no] {""}
    [yes] : { $message }
}
execution-error-step-not-found = Askelta ei löytynyt indeksistä { $index }: { $keyword } { $text } (feature: { $feature_path }, skenaario: { $scenario_name })
execution-error-missing-fixtures = Askel '{ $step_pattern }' (määritelty kohdassa { $step_location }) vaatii fixtuurit { $required }, mutta seuraavat puuttuvat: { $missing }. Käytettävissä olevat fixtuurit skenaariosta: { $available } (feature: { $feature_path }, skenaario: { $scenario_name })
execution-error-handler-failed = Askel epäonnistui indeksissä { $index }: { $keyword } { $text } - { $error } (feature: { $feature_path }, skenaario: { $scenario_name })
