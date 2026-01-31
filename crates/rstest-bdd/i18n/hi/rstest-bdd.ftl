step-error-missing-fixture = स्टेप फ़ंक्शन « { $step } » के लिए प्रकार « { $ty } » की फ़िक्चर « { $name } » अनुपस्थित है
step-error-execution = फ़ंक्शन « { $function } » के माध्यम से स्टेप « { $pattern } » चलाते समय त्रुटि: { $message }
step-error-panic = स्टेप « { $pattern } » में, फ़ंक्शन « { $function } » में पैनिक: { $message }
step-keyword-parse-error = अमान्य स्टेप कीवर्ड: { $keyword }
unsupported-step-type = असमर्थित स्टेप प्रकार: { $step_type }
step-pattern-not-compiled = स्टेप पैटर्न का रेगेक्स संकलित नहीं हुआ; पहले पैटर्न « { $pattern } » पर compile() कॉल करें
placeholder-pattern-mismatch = पैटर्न मेल नहीं खाया
placeholder-invalid-placeholder = अमान्य प्लेसहोल्डर वाक्य-विन्यास: { $details }
placeholder-invalid-pattern = अमान्य स्टेप पैटर्न: { $pattern }
placeholder-not-compiled = स्टेप पैटर्न « { $pattern } » को उपयोग से पहले संकलित करना आवश्यक है
placeholder-syntax = अमान्य प्लेसहोल्डर वाक्य-विन्यास: { $details }
placeholder-syntax-detail = { $reason } बाइट { $position } (शून्य-आधारित) पर{ $suffix }
placeholder-syntax-suffix = प्लेसहोल्डर « { $placeholder } » के लिए
step-context-ambiguous-override = अस्पष्ट फ़िक्चर ओवरराइड: एक से अधिक फ़िक्चर type_id { $type_id } से मेल खाते हैं। ओवरराइड नज़रअंदाज़ किया गया।
panic-message-opaque-payload = <{ $type } प्रकार का नॉन-डिबग पैनिक पेलोड>
assert-step-ok-panic = स्टेप ने त्रुटि लौटाई: { $error }
assert-step-err-success = स्टेप अप्रत्याशित रूप से सफल रहा
assert-step-err-missing-substring = त्रुटि « { $display } » में « { $expected } » शामिल नहीं है

assert-skip-not-skipped = अपेक्षा थी कि { $target } एक छोड़े गए परिणाम को दर्ज करेगा
assert-skip-missing-message = अपेक्षा थी कि { $target } एक स्किप संदेश दे जिसमें « { $expected } » शामिल हो
assert-skip-missing-substring = स्किप संदेश « { $actual } » में « { $expected } » शामिल नहीं है
assert-skip-unexpected-message = अपेक्षा थी कि { $target } कोई स्किप संदेश नहीं देगा
assert-skip-flag-mismatch = अपेक्षा थी कि { $target } के फ़्लैग « { $flag } » का मान « { $expected } » होगा, परन्तु वह « { $actual } » था

execution-error-skip = Step skipped{ $has_message ->
    *[no] {""}
    [yes] : { $message }
}
execution-error-step-not-found = Step not found at index { $index }: { $keyword } { $text } (feature: { $feature_path }, scenario: { $scenario_name })
execution-error-missing-fixtures = Step '{ $step_pattern }' (defined at { $step_location }) requires fixtures { $required }, but the following are missing: { $missing }. Available fixtures from scenario: { $available } (feature: { $feature_path }, scenario: { $scenario_name })
execution-error-handler-failed = Step failed at index { $index }: { $keyword } { $text } - { $error } (feature: { $feature_path }, scenario: { $scenario_name })
