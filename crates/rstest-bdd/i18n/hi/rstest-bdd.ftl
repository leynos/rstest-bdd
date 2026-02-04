step-error-missing-fixture = स्टेप फ़ंक्शन « { $step } » के लिए प्रकार « { $ty } » की फ़िक्चर « { $name } » अनुपस्थित है
step-error-execution = फ़ंक्शन « { $function } » के माध्यम से स्टेप « { $pattern } » चलाते समय त्रुटि: { $message }
step-error-panic = स्टेप « { $pattern } » में, फ़ंक्शन « { $function } » में पैनिक: { $message }
step-keyword-parse-error = अमान्य स्टेप कीवर्ड: { $keyword }
unsupported-step-type = असमर्थित स्टेप प्रकार: { $step_type }
placeholder-pattern-mismatch = पैटर्न मेल नहीं खाया
placeholder-invalid-placeholder = अमान्य प्लेसहोल्डर वाक्य-विन्यास: { $details }
placeholder-invalid-pattern = अमान्य स्टेप पैटर्न: { $pattern }
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

execution-error-skip = स्टेप छोड़ा गया{ $has_message ->
    *[no] {""}
    [yes] : { $message }
}
execution-error-step-not-found = इंडेक्स { $index } पर स्टेप नहीं मिला: { $keyword } { $text } (फ़ीचर: { $feature_path }, सिनेरियो: { $scenario_name })
execution-error-missing-fixtures = स्टेप « { $step_pattern } » ({ $step_location } पर परिभाषित) को फ़िक्चर { $required } की आवश्यकता है, लेकिन निम्नलिखित अनुपस्थित हैं: { $missing }। सिनेरियो से उपलब्ध फ़िक्चर: { $available } (फ़ीचर: { $feature_path }, सिनेरियो: { $scenario_name })
execution-error-handler-failed = इंडेक्स { $index } पर स्टेप विफल: { $keyword } { $text } - { $error } (फ़ीचर: { $feature_path }, सिनेरियो: { $scenario_name })
