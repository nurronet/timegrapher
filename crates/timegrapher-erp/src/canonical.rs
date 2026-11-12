//! `LN-CANONICAL-JSON-1`: the bytes the ERP checks a signature against.
//!
//! The ERP rebuilds the signed bytes from the request it receives, with
//! Python's `json.dumps(value, ensure_ascii=False, allow_nan=False,
//! sort_keys=True, separators=(",", ":"))`. One different character fails
//! every signature, and nothing says why, so this module reproduces that
//! output exactly, including how Python writes a float: the shortest digits
//! that read back as the same number, always with a decimal point (`20.0`),
//! and in scientific form only below 1e-4 or from 1e16 (`1e-05`, `1e+16`).
//!
//! An integer and a float with the same value are different bytes (`5` and
//! `5.0`); the type a value has in the [`Value`] is the type it is signed as.

use serde_json::Value;

/// Writes `value` as canonical JSON.
pub fn to_bytes(value: &Value) -> Vec<u8> {
    let mut out = String::new();
    write_value(&mut out, value);
    out.into_bytes()
}

fn write_value(out: &mut String, value: &Value) {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                out.push_str(&i.to_string());
            } else if let Some(u) = n.as_u64() {
                out.push_str(&u.to_string());
            } else {
                out.push_str(&float_repr(n.as_f64().unwrap_or(0.0)));
            }
        }
        Value::String(s) => write_string(out, s),
        Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_value(out, item);
            }
            out.push(']');
        }
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            // Python sorts by code point; UTF-8 byte order is the same order.
            // serde_json's Map is already sorted unless a build turns on its
            // `preserve_order` feature; sorting here keeps the signed bytes
            // right either way, so it is not left to the Map.
            keys.sort();
            out.push('{');
            for (i, key) in keys.into_iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_string(out, key);
                out.push(':');
                write_value(out, &map[key]);
            }
            out.push('}');
        }
    }
}

/// Python's escape table with `ensure_ascii=False`: the short escapes, any
/// other control character as `\u00xx`, everything else as it is.
fn write_string(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

/// Python's `repr(float)`. NaN and infinity cannot be signed and are
/// refused earlier (a JSON number cannot hold them); they give `0.0` here
/// rather than invalid JSON.
pub fn float_repr(value: f64) -> String {
    if !value.is_finite() {
        return "0.0".to_string();
    }
    if value == 0.0 {
        return if value.is_sign_negative() {
            "-0.0"
        } else {
            "0.0"
        }
        .to_string();
    }
    // `{:e}` gives the shortest digits that read back exactly, as
    // "1.2345e-6": sign, digits, exponent.
    let sci = format!("{value:e}");
    let (mantissa, exp) = sci.split_once('e').unwrap_or((&sci, "0"));
    let exp: i32 = exp.parse().unwrap_or(0);
    let (sign, mantissa) = match mantissa.strip_prefix('-') {
        Some(rest) => ("-", rest),
        None => ("", mantissa),
    };
    let digits: String = mantissa.chars().filter(|c| *c != '.').collect();
    let n = digits.len() as i32;

    if !(-4..16).contains(&exp) {
        let mut s = format!("{sign}{}", &digits[..1]);
        if n > 1 {
            s.push('.');
            s.push_str(&digits[1..]);
        }
        let exp_sign = if exp < 0 { '-' } else { '+' };
        s.push_str(&format!("e{exp_sign}{:02}", exp.abs()));
        s
    } else if exp >= 0 {
        let int_digits = exp + 1;
        if n <= int_digits {
            format!("{sign}{digits}{}.0", "0".repeat((int_digits - n) as usize))
        } else {
            let (int_part, frac_part) = digits.split_at(int_digits as usize);
            format!("{sign}{int_part}.{frac_part}")
        }
    } else {
        format!("{sign}0.{}{digits}", "0".repeat((-exp - 1) as usize))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // Each pair is Python's `repr(x)` for the value (see the guide's
    // reference list); `float_repr` must agree to the character.
    #[test]
    fn floats_are_written_as_python_writes_them() {
        let cases: [(f64, &str); 22] = [
            (0.1, "0.1"),
            (20.0, "20.0"),
            (1e-5, "1e-05"),
            (1e-4, "0.0001"),
            (1e16, "1e+16"),
            (1e15, "1000000000000000.0"),
            (1.5e16, "1.5e+16"),
            (0.1 + 0.2, "0.30000000000000004"),
            (-0.0, "-0.0"),
            (0.0, "0.0"),
            (12.345679, "12.345679"),
            (100000.0, "100000.0"),
            (359.999999, "359.999999"),
            (28800.0, "28800.0"),
            (245.5, "245.5"),
            (-1.2, "-1.2"),
            (1e-6, "1e-06"),
            (123456789012345678.0, "1.2345678901234568e+17"),
            (5e-324, "5e-324"),
            (f64::MAX, "1.7976931348623157e+308"),
            (-0.00001, "-1e-05"),
            (4.35e-5, "4.35e-05"),
        ];
        for (value, expected) in cases {
            assert_eq!(float_repr(value), expected, "{value:e}");
        }
        assert_eq!(float_repr(0.000123), "0.000123");
        assert_eq!(float_repr(1234567.125), "1234567.125");
    }

    #[test]
    fn an_integer_and_a_float_of_the_same_value_are_different_bytes() {
        assert_eq!(to_bytes(&json!({"a": 5})), br#"{"a":5}"#);
        assert_eq!(to_bytes(&json!({"a": 5.0})), br#"{"a":5.0}"#);
    }

    #[test]
    fn keys_are_sorted_at_every_level_with_no_spaces() {
        let v = json!({"b": {"y": 1, "x": [true, null, "s"]}, "a": false});
        assert_eq!(
            String::from_utf8(to_bytes(&v)).unwrap(),
            r#"{"a":false,"b":{"x":[true,null,"s"],"y":1}}"#
        );
    }

    // Python: json.dumps("tab\there \"q\" back\\slash \u0001 \u007f é ü 日本 \n",
    // ensure_ascii=False)
    #[test]
    fn strings_are_escaped_as_python_escapes_them() {
        let s = "tab\there \"q\" back\\slash \u{1} \u{7f} é ü 日本 \n";
        assert_eq!(
            String::from_utf8(to_bytes(&json!(s))).unwrap(),
            "\"tab\\there \\\"q\\\" back\\\\slash \\u0001 \u{7f} é ü 日本 \\n\""
        );
        assert_eq!(to_bytes(&json!("\u{8}\u{c}")), br#""\b\f""#);
    }
}
