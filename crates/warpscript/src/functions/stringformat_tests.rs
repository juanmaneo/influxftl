// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! Tests of `STRINGFORMAT` against `String.format` (OpenJDK 27, `Locale.ROOT`).

use super::stringformat::{Locale, printf_format};
use crate::tests::{assert_fails, assert_top};
use crate::value::Value;

fn fmt(format: &str, args: &[Value]) -> String {
    printf_format(Locale::ROOT, format, args).expect("printf_format should succeed")
}

fn err(format: &str, args: &[Value]) -> String {
    printf_format(Locale::ROOT, format, args)
        .expect_err("the value should fail")
        .message()
}

fn longs(values: &[i64]) -> Vec<Value> {
    values.iter().map(|&v| Value::Long(v)).collect()
}

fn doubles(values: &[f64]) -> Vec<Value> {
    values.iter().map(|&v| Value::Double(v)).collect()
}

fn strings(values: &[&str]) -> Vec<Value> {
    values.iter().map(|&v| Value::from(v)).collect()
}

#[test]
fn integers() {
    assert_eq!(fmt("%d", &longs(&[42])), "42");
    assert_eq!(
        fmt("%5d|%-5d|%05d", &longs(&[42, 42, 42])),
        "   42|42   |00042"
    );
    assert_eq!(fmt("%+d % d %(d", &longs(&[42, 42, -42])), "+42  42 (42)");
    assert_eq!(fmt("%,d", &longs(&[1_234_567])), "1,234,567");
    assert_eq!(fmt("%,d", &longs(&[-1_234_567])), "-1,234,567");
    assert_eq!(fmt("%x %X %o", &longs(&[255, 255, 8])), "ff FF 10");
    assert_eq!(fmt("%x", &longs(&[-1])), "ffffffffffffffff");
    assert_eq!(fmt("%#x %#o", &longs(&[255, 8])), "0xff 010");
    assert_eq!(fmt("%(d", &longs(&[42])), "42");
    assert_eq!(fmt("%d", &longs(&[i64::MIN])), "-9223372036854775808");
    assert_eq!(
        fmt("%,d", &longs(&[i64::MIN])),
        "-9,223,372,036,854,775,808"
    );
    assert_eq!(fmt("%o", &longs(&[-1])), "1777777777777777777777");
    assert_eq!(fmt("%06d", &longs(&[-42])), "-00042");
    assert_eq!(fmt("%(06d", &longs(&[-42])), "(0042)");
    assert_eq!(fmt("%#06x", &longs(&[255])), "0x00ff");
    assert_eq!(fmt("%08X", &longs(&[255])), "000000FF");
    assert_eq!(fmt("%,010d", &longs(&[1234])), "000001,234");
    assert_eq!(fmt("%d", &[Value::Null]), "null");
    assert_eq!(fmt("%3d|", &longs(&[12345])), "12345|");
    assert_eq!(fmt("%+(d %+(d % (d", &longs(&[-5, 5, -5])), "(5) +5 (5)");
    assert_eq!(fmt("%,d %,d", &longs(&[123, -12])), "123 -12");
    assert_eq!(fmt("%+,010d", &longs(&[1234])), "+00001,234");
    assert_eq!(fmt("%x %#o %#x", &longs(&[0, 0, 0])), "0 00 0x0");
    assert_eq!(fmt("%05x", &longs(&[-1])), "ffffffffffffffff");
    assert_eq!(fmt("%010o", &longs(&[8])), "0000000010");
    assert_eq!(fmt("%+x", &[Value::Null]), "null");
}

#[test]
fn floats() {
    assert_eq!(fmt("%08.3f", &doubles(&[3.141_593_5])), "0003.142");
    assert_eq!(fmt("%.2f", &doubles(&[0.125])), "0.13");
    assert_eq!(fmt("%.2f", &doubles(&[2.675])), "2.68");
    assert_eq!(fmt("%.1f", &doubles(&[0.25])), "0.3");
    assert_eq!(fmt("%.0f", &doubles(&[2.5])), "3");
    assert_eq!(fmt("%.0f", &doubles(&[3.5])), "4");
    assert_eq!(fmt("%f", &doubles(&[1.0])), "1.000000");
    assert_eq!(fmt("%f", &doubles(&[1e20])), "100000000000000000000.000000");
    assert_eq!(fmt("%.3f", &doubles(&[1e-10])), "0.000");
    assert_eq!(fmt("%.2f", &doubles(&[1.005])), "1.01");
    assert_eq!(fmt("%.2f", &doubles(&[1.015])), "1.02");
    assert_eq!(fmt("%.1f", &doubles(&[0.05])), "0.1");
    assert_eq!(fmt("%.1f", &doubles(&[0.15])), "0.2");
    assert_eq!(fmt("%.10f", &doubles(&[0.1])), "0.1000000000");
    assert_eq!(fmt("%.20f", &doubles(&[0.1])), "0.10000000000000000000");
    assert_eq!(fmt("%#.0f", &doubles(&[3.0])), "3.");
    assert_eq!(fmt("%(.1f", &doubles(&[-2.5])), "(2.5)");
    assert_eq!(fmt("%,.2f", &doubles(&[1_234_567.891])), "1,234,567.89");
    assert_eq!(fmt("%.3f", &doubles(&[f64::NAN])), "NaN");
    assert_eq!(fmt("%5.1f|", &doubles(&[f64::INFINITY])), "Infinity|");
    assert_eq!(fmt("%05f", &doubles(&[f64::NEG_INFINITY])), "-Infinity");
    assert_eq!(fmt("%(f", &doubles(&[f64::NEG_INFINITY])), "(Infinity)");
    assert_eq!(fmt("%+f", &doubles(&[f64::INFINITY])), "+Infinity");
    assert_eq!(fmt("%.1f", &doubles(&[-0.0])), "-0.0");
    assert_eq!(fmt("%03.1f", &doubles(&[1234.5])), "1234.5");
    assert_eq!(fmt("%#02x", &longs(&[255])), "0xff");
    assert_eq!(fmt("%02.1e", &doubles(&[1.5])), "1.5e+00");
    assert_eq!(fmt("%014g", &doubles(&[1e-5])), "0001.00000e-05");
    assert_eq!(fmt("%2g", &doubles(&[1e-5])), "1.00000e-05");
    assert_eq!(fmt("%.0f", &doubles(&[0.4])), "0");
    assert_eq!(fmt("%.2f", &doubles(&[0.0])), "0.00");
    assert_eq!(fmt("%.0f", &doubles(&[1e-300])), "0");
    assert_eq!(fmt("%.1f", &doubles(&[123_456.0])), "123456.0");
    assert_eq!(fmt("%.1f", &doubles(&[99.96])), "100.0");
    assert_eq!(
        fmt("%7.1f|%-7.1f|", &doubles(&[1.5, 1.5])),
        "    1.5|1.5    |"
    );
    assert_eq!(fmt("%f", &[Value::Null]), "null");
    assert_eq!(fmt("%.1f %.1f", &doubles(&[0.95, 9.95])), "1.0 10.0");
    assert_eq!(fmt("%.2f", &doubles(&[9.995])), "10.00");
    assert_eq!(fmt("%.0f %.0f %.0f", &doubles(&[0.5, 1.5, 9.5])), "1 2 10");
    assert_eq!(fmt("%f", &doubles(&[5e-324])), "0.000000");
    assert_eq!(fmt("%#.0f", &doubles(&[0.0])), "0.");
    assert_eq!(fmt("%,.0f", &doubles(&[1_234_567.5])), "1,234,568");
    assert_eq!(fmt("%,015.2f", &doubles(&[-1234.5])), "-0000001,234.50");
    assert_eq!(fmt("%,(015.2f", &doubles(&[-1234.5])), "(000001,234.50)");
    assert_eq!(fmt("%(08.2f", &doubles(&[-1.5])), "(001.50)");
    assert_eq!(
        fmt(
            "%08.3f|%+08.3f|% 08.3f",
            &doubles(&[-1.2345, 1.2345, 1.2345])
        ),
        "-001.235|+001.235| 001.235"
    );
    assert_eq!(
        fmt("%.2f", &doubles(&[1e300])),
        format!("1{}.00", "0".repeat(300))
    );
    // The precision of a general conversion also cuts the `null` of a numeric one.
    assert_eq!(fmt("%.1f|%.2e", &[Value::Null, Value::Null]), "n|nu");
}

#[test]
fn scientific_and_general() {
    assert_eq!(fmt("%e", &doubles(&[1.5])), "1.500000e+00");
    assert_eq!(fmt("%E", &doubles(&[1234.5678])), "1.234568E+03");
    assert_eq!(fmt("%.2e", &doubles(&[0.000_123_456])), "1.23e-04");
    assert_eq!(fmt("%+.1e", &doubles(&[-1234.5])), "-1.2e+03");
    assert_eq!(fmt("%e", &doubles(&[0.0])), "0.000000e+00");
    assert_eq!(fmt("%.3e", &doubles(&[9.9995])), "1.000e+01");
    assert_eq!(fmt("%.0e", &doubles(&[12345.0])), "1e+04");
    assert_eq!(fmt("%#.0e", &doubles(&[12345.0])), "1.e+04");
    assert_eq!(fmt("%e", &doubles(&[1e100])), "1.000000e+100");
    assert_eq!(fmt("%e", &doubles(&[1e-100])), "1.000000e-100");
    assert_eq!(fmt("%012.2e", &doubles(&[1.5])), "00001.50e+00");
    assert_eq!(fmt("%3.2e", &doubles(&[1.5])), "1.50e+00");
    assert_eq!(fmt("%g", &doubles(&[1234.5678])), "1234.57");
    assert_eq!(fmt("%g", &doubles(&[0.000_012_34])), "1.23400e-05");
    assert_eq!(fmt("%g", &doubles(&[123_456_789.0])), "1.23457e+08");
    assert_eq!(fmt("%.3g", &doubles(&[0.0001])), "0.000100");
    assert_eq!(fmt("%g", &doubles(&[0.0])), "0.00000");
    assert_eq!(fmt("%.0g", &doubles(&[1234.5])), "1e+03");
    assert_eq!(fmt("%g", &doubles(&[99999.5])), "99999.5");
    assert_eq!(fmt("%g", &doubles(&[999_999.5])), "1.00000e+06");
    assert_eq!(fmt("%.2g", &doubles(&[0.000_099_999])), "0.00010");
    assert_eq!(fmt("%g", &doubles(&[1e-5])), "1.00000e-05");
    assert_eq!(fmt("%g", &doubles(&[1e-4])), "0.000100000");
    assert_eq!(fmt("%g", &doubles(&[1e6])), "1.00000e+06");
    assert_eq!(fmt("%g", &doubles(&[1e5])), "100000");
    assert_eq!(fmt("%,g", &doubles(&[123_456.0])), "123,456");
    assert_eq!(fmt("%G", &doubles(&[1e-5])), "1.00000E-05");
    assert_eq!(fmt("%a", &doubles(&[1.0])), "0x1.0p0");
    assert_eq!(fmt("%a", &doubles(&[0.0])), "0x0.0p0");
    assert_eq!(fmt("%a", &doubles(&[-2.5])), "-0x1.4p1");
    assert_eq!(fmt("%A", &doubles(&[1.5])), "0X1.8P0");
    assert_eq!(fmt("%a", &doubles(&[5e-324])), "0x0.0000000000001p-1022");
    assert_eq!(fmt("%.3e", &doubles(&[0.0])), "0.000e+00");
    assert_eq!(fmt("%#.0e", &doubles(&[0.0])), "0.e+00");
    assert_eq!(fmt("%e", &doubles(&[5e-324])), "4.900000e-324");
    assert_eq!(fmt("%(e", &doubles(&[-1.5])), "(1.500000e+00)");
    assert_eq!(fmt("%(012.2e", &doubles(&[-1.5])), "(001.50e+00)");
    assert_eq!(
        fmt("%10.2e|%-10.2e|", &doubles(&[1.5, 1.5])),
        "  1.50e+00|1.50e+00  |"
    );
    assert_eq!(
        fmt("%e %+e %05e", &doubles(&[f64::NAN; 3])),
        "NaN NaN   NaN"
    );
    assert_eq!(fmt("%(E", &doubles(&[f64::NEG_INFINITY])), "(INFINITY)");
    assert_eq!(fmt("%g", &doubles(&[1e-300])), "1.00000e-300");
    assert_eq!(fmt("%.0g", &doubles(&[0.0])), "0");
    assert_eq!(fmt("%(g", &doubles(&[-1.5])), "(1.50000)");
    assert_eq!(fmt("%.1g %.2g", &doubles(&[0.95, 95.0])), "1 95");
    assert_eq!(fmt("%.2g %.1g", &doubles(&[99.5, 9.5])), "1.0e+02 1e+01");
}

#[test]
fn hexadecimal_floats() {
    assert_eq!(fmt("%010a", &doubles(&[-1.0])), "-0x001.0p0");
    assert_eq!(fmt("%010a", &doubles(&[1.0])), "0x0001.0p0");
    assert_eq!(fmt("%+010a", &doubles(&[1.0])), "+0x001.0p0");
    assert_eq!(fmt("% 010a", &doubles(&[1.0])), " 0x001.0p0");
    assert_eq!(fmt("%-10a|", &doubles(&[1.0])), "0x1.0p0   |");
    assert_eq!(fmt("%10A|", &doubles(&[-1.0])), "  -0X1.0P0|");
    assert_eq!(
        fmt("%.1a %.3a", &doubles(&[0.1, 0.1])),
        "0x1.ap-4 0x1.99ap-4"
    );
    assert_eq!(fmt("%.12a", &doubles(&[0.1])), "0x1.99999999999ap-4");
    assert_eq!(fmt("%.13a", &doubles(&[0.1])), "0x1.999999999999ap-4");
    assert_eq!(fmt("%.20a", &doubles(&[1.0])), "0x1.00000000000000000000p0");
    assert_eq!(fmt("%.0a", &doubles(&[0.1])), "0x1.ap-4");
    assert_eq!(fmt("%.1a", &doubles(&[5e-324])), "0x1.0p-1074");
    assert_eq!(fmt("%.2a", &doubles(&[1e-310])), "0x1.27p-1030");
    assert_eq!(
        fmt("%.14a", &doubles(&[1e-310])),
        "0x0.012688b70e62b0p-1022"
    );
    assert_eq!(fmt("%.3A", &doubles(&[1e-320])), "0X1.FA0P-1064");
    assert_eq!(fmt("%.2a", &doubles(&[f64::MIN_POSITIVE])), "0x1.00p-1022");
    // Ties to even: `1.f8` rounds up to the next binade, `1.08` down, `1.18` up.
    assert_eq!(
        fmt(
            "%.1a %.1a %.1a %.1a",
            &doubles(&[1.96875, 1.03125, 1.09375, 1.0625])
        ),
        "0x1.0p1 0x1.0p0 0x1.2p0 0x1.1p0"
    );
    assert_eq!(fmt("%.1a", &doubles(&[f64::MAX])), "0x1.0p1024");
    assert_eq!(fmt("%.12a", &doubles(&[f64::MAX])), "0x1.000000000000p1024");
    assert_eq!(
        fmt("%a %.2a %.2a", &doubles(&[-0.0, 0.0, -0.0])),
        "-0x0.0p0 0x0.00p0 -0x0.00p0"
    );
    assert_eq!(
        fmt(
            "%a %.2a %+a",
            &doubles(&[f64::NAN, f64::INFINITY, f64::NEG_INFINITY])
        ),
        "NaN Infinity -Infinity"
    );
    assert_eq!(fmt("%a", &doubles(&[1e-310])), "0x0.012688b70e62bp-1022");
    assert_eq!(fmt("%a", &doubles(&[f64::MIN_POSITIVE])), "0x1.0p-1022");
    assert_eq!(fmt("%a", &doubles(&[1e-320])), "0x0.00000000007e8p-1022");
}

#[test]
fn strings_booleans_hashes() {
    assert_eq!(fmt("%s %S", &strings(&["abc", "abc"])), "abc ABC");
    assert_eq!(fmt("%.2s", &strings(&["abcdef"])), "ab");
    assert_eq!(fmt("%5s|%-5s|", &strings(&["ab", "ab"])), "   ab|ab   |");
    assert_eq!(fmt("%10.3s|", &strings(&["abcdef"])), "       abc|");
    assert_eq!(
        fmt(
            "%b %b %b %b",
            &[
                Value::Null,
                Value::Boolean(true),
                Value::Boolean(false),
                Value::from("x")
            ]
        ),
        "false true false true"
    );
    assert_eq!(fmt("%b", &longs(&[0])), "true");
    assert_eq!(fmt("%B", &[Value::Null]), "FALSE");
    assert_eq!(
        fmt(
            "%h %h %h",
            &[Value::from("abc"), Value::Long(42), Value::Double(1.5)]
        ),
        "17862 2a 3ff80000"
    );
    assert_eq!(fmt("%h", &[Value::Null]), "null");
    assert_eq!(
        fmt("%h %h", &[Value::Boolean(true), Value::Boolean(false)]),
        "4cf 4d5"
    );
    assert_eq!(fmt("%h", &doubles(&[f64::NAN])), "7ff80000");
    assert_eq!(fmt("%h", &[Value::list(longs(&[1, 2]))]), "3e2");
    assert_eq!(fmt("%h", &[crate::tests::top("{ 'a' 1 }")]), "60");
    assert_eq!(fmt("%h", &[crate::tests::top("( 1 2 )")]), "3");
    assert_eq!(
        fmt("%h", &[crate::tests::top("COUNTER 42 COUNTERSET")]),
        "2a"
    );
    assert_eq!(fmt("%h", &[crate::tests::top("<% %>")]), "0");
    assert_eq!(fmt("%s", &[Value::Null]), "null");
    assert_eq!(fmt("%s", &doubles(&[1.5])), "1.5");
    assert_eq!(fmt("%s", &doubles(&[1e21])), "1.0E21");
    assert_eq!(
        fmt("%s", &[Value::list(vec![Value::Long(1), Value::from("a")])]),
        "[1, a]"
    );
    assert_eq!(fmt("%S", &strings(&["ß"])), "SS");
    assert_eq!(fmt("%.1s|", &strings(&["😀x"])), "\u{FFFD}|");
    assert_eq!(fmt("%.2s|", &strings(&["😀x"])), "😀|");
    assert_eq!(fmt("%3s|", &strings(&["😀"])), " 😀|");
    assert_eq!(fmt("%c", &[Value::Null]), "null");
    assert_eq!(
        fmt("%S|%-10S|", &[Value::Null, Value::from("a")]),
        "NULL|A         |"
    );
    assert_eq!(fmt("%-3c|", &[Value::Null]), "null|");
    assert_eq!(fmt("%.5s|%.0s|", &strings(&["ab", "abc"])), "ab||");
    // The precision cuts every general conversion.
    assert_eq!(
        fmt(
            "%.2b %.2B %.1h",
            &[
                Value::Boolean(true),
                Value::Boolean(true),
                Value::from("abc")
            ]
        ),
        "tr TR 1"
    );
}

#[test]
fn literals_indexes_and_locales() {
    assert_eq!(fmt("%n", &[]), "\n");
    assert_eq!(fmt("%%", &[]), "%");
    assert_eq!(fmt("%5%|", &[]), "    %|");
    assert_eq!(fmt("%-5%|", &[]), "%    |");
    assert_eq!(fmt("a%%b%nc", &[]), "a%b\nc");
    assert_eq!(fmt("%2$s %1$s", &strings(&["a", "b"])), "b a");
    assert_eq!(fmt("%s %<s", &strings(&["a"])), "a a");
    // `<` wins over an explicit index.
    assert_eq!(fmt("%s %1$<s", &strings(&["a"])), "a a");
    assert_eq!(fmt("%2$d %<d %d", &longs(&[1, 2])), "2 2 1");
    assert_eq!(fmt("%s %s %<s %s", &strings(&["a", "b", "c"])), "a b b c");
    assert_eq!(fmt("%d %d", &longs(&[1, 2, 3])), "1 2");
    assert_eq!(fmt("%1$d %1$d %d", &longs(&[7, 8])), "7 7 7");
    assert_eq!(fmt("%2$5d|%<-5d|", &longs(&[7, 8])), "    8|8    |");
    assert_eq!(fmt("plain text", &[]), "plain text");
    assert_eq!(fmt("", &longs(&[1])), "");
    assert_eq!(
        printf_format(
            Locale::for_tag("fr").expect("for_tag should succeed"),
            "%,d %,.2f %e",
            &[
                Value::Long(1_234_567),
                Value::Double(1234.5),
                Value::Double(1.5)
            ]
        )
        .expect("the value should succeed"),
        "1\u{202F}234\u{202F}567 1\u{202F}234,50 1,500000e+00"
    );
    assert_eq!(
        printf_format(
            Locale::for_tag("de_DE").expect("for_tag should succeed"),
            "%,d %,.2f",
            &[Value::Long(1_234_567), Value::Double(1234.5)]
        )
        .expect("the value should succeed"),
        "1.234.567 1.234,50"
    );
    assert_eq!(
        printf_format(
            Locale::for_tag("ru").expect("for_tag should succeed"),
            "%,d",
            &longs(&[1234])
        )
        .expect("printf_format should succeed"),
        "1\u{00A0}234"
    );
    assert_eq!(Locale::for_tag("en-US"), Some(Locale::ROOT));
    assert_eq!(Locale::for_tag(""), Some(Locale::ROOT));
    assert_eq!(Locale::for_tag("xx"), None);
}

#[test]
fn errors_match_java() {
    assert_eq!(err("%s %s", &strings(&["a"])), "Format specifier '%s'");
    assert_eq!(err("%<s", &strings(&["a"])), "Format specifier '%<s'");
    assert_eq!(err("%3$s", &strings(&["a"])), "Format specifier '%3$s'");
    assert_eq!(
        err("%0$s", &strings(&["a"])),
        "Illegal format argument index = 0"
    );
    assert_eq!(err("%q", &longs(&[1])), "Conversion = 'q'");
    assert_eq!(err("abc%", &[]), "Conversion = '%'");
    assert_eq!(err("%.s", &[]), "Conversion = '.'");
    assert_eq!(err("%.", &[]), "Conversion = '.'");
    assert_eq!(err("%5", &[]), "Conversion = '5'");
    assert_eq!(err("%-", &[]), "Conversion = '-'");
    assert_eq!(err("%!", &[]), "Conversion = '!'");
    assert_eq!(err("%d", &doubles(&[1.5])), "d != java.lang.Double");
    assert_eq!(err("%f", &longs(&[1])), "f != java.lang.Long");
    assert_eq!(err("%x", &doubles(&[1.5])), "x != java.lang.Double");
    // Java names the conversion in lower case whatever the case written.
    assert_eq!(err("%X", &strings(&["a"])), "x != java.lang.String");
    assert_eq!(err("%c", &strings(&["ab"])), "c != java.lang.String");
    assert_eq!(err("%c", &longs(&[65])), "c != java.lang.Long");
    assert_eq!(err("%d", &[Value::Boolean(true)]), "d != java.lang.Boolean");
    assert_eq!(
        err("%E", &[Value::list(Vec::new())]),
        "e != java.util.ArrayList"
    );
    assert_eq!(err("%-d", &longs(&[1])), "%-d");
    assert_eq!(err("%0d", &longs(&[1])), "%0d");
    assert_eq!(err("%-s", &strings(&["a"])), "%-s");
    assert_eq!(err("%-c", &[Value::Null]), "%-c");
    assert_eq!(err("%-%", &[]), "%-%");
    assert_eq!(err("%.2d", &longs(&[1])), "2");
    assert_eq!(err("%.2c", &[Value::Null]), "2");
    assert_eq!(err("%.3%", &[]), "3");
    assert_eq!(err("%.3n", &[]), "3");
    assert_eq!(err("%5n", &[]), "5");
    assert_eq!(err("%-n", &[]), "Flags = '-'");
    assert_eq!(err("%+%", &[]), "Flags = '+'");
    assert_eq!(err("%#s", &strings(&["a"])), "Conversion = s, Flags = #");
    assert_eq!(err("%#b", &strings(&["a"])), "Conversion = b, Flags = #");
    assert_eq!(err("%#H", &strings(&["a"])), "Conversion = h, Flags = #");
    assert_eq!(err("%+s", &strings(&["a"])), "Conversion = s, Flags = +");
    assert_eq!(err("% s", &strings(&["a"])), "Conversion = s, Flags =  ");
    assert_eq!(err("%0s", &strings(&["a"])), "Conversion = s, Flags = 0");
    assert_eq!(err("%05s", &strings(&["a"])), "Conversion = s, Flags = 0");
    assert_eq!(err("%,s", &strings(&["a"])), "Conversion = s, Flags = ,");
    assert_eq!(err("%(s", &strings(&["a"])), "Conversion = s, Flags = (");
    assert_eq!(err("%#c", &[Value::Null]), "Conversion = c, Flags = #");
    assert_eq!(err("%#d", &longs(&[1])), "Conversion = d, Flags = #");
    assert_eq!(err("%,x", &longs(&[15])), "Conversion = x, Flags = ,");
    assert_eq!(err("%,o", &longs(&[15])), "Conversion = o, Flags = ,");
    assert_eq!(err("%,e", &doubles(&[1.5])), "Conversion = e, Flags = ,");
    assert_eq!(err("%,a", &doubles(&[1.5])), "Conversion = a, Flags = ,");
    assert_eq!(err("%(a", &doubles(&[1.5])), "Conversion = a, Flags = (");
    assert_eq!(err("%#g", &doubles(&[1.0])), "Conversion = g, Flags = #");
    assert_eq!(err("%-05d", &longs(&[1])), "Flags = '-0'");
    assert_eq!(err("%+ d", &longs(&[1])), "Flags = '+ '");
    assert_eq!(err("%--5d", &longs(&[1])), "Flags = '-'");
    assert_eq!(err("%##x", &longs(&[1])), "Flags = '#'");
    assert_eq!(
        err("%tY", &longs(&[0])),
        "Date/time conversions (%t) are not supported."
    );
    assert_eq!(
        err("%TH", &longs(&[0])),
        "Date/time conversions (%t) are not supported."
    );
    assert_eq!(err("%-f", &doubles(&[1.0])), "%-f");
    assert_eq!(err("%-05f", &doubles(&[1.0])), "Flags = '-0'");
    assert_eq!(err("%+ e", &doubles(&[1.0])), "Flags = '+ '");
    assert_eq!(err("%0-5d", &longs(&[1])), "Flags = '-0'");
    assert_eq!(err("%-0d", &longs(&[1])), "%-0d");
    assert_eq!(
        err("%,15.3E|", &doubles(&[1.5])),
        "Conversion = e, Flags = ,"
    );
    // Sign flags are rejected by the octal and hexadecimal conversions of a `LONG` only.
    assert_eq!(err("%+x", &longs(&[1])), "Conversion = x, Flags = +");
    assert_eq!(err("% o", &longs(&[1])), "Conversion = o, Flags =  ");
    assert_eq!(err("%(x", &longs(&[1])), "Conversion = x, Flags = (");
    // Upper case variants exist for `b h s c x e g a` only.
    for format in ["%F", "%D", "%O", "%N", "%Q"] {
        assert_eq!(
            err(format, &longs(&[1])),
            format!("Conversion = '{}'", &format[1..]),
            "{format}"
        );
    }
    // An unknown alphabetic conversion is named; any other failure names the character
    // after the `%`.
    assert_eq!(err("%5q", &longs(&[1])), "Conversion = 'q'");
    assert_eq!(err("%1$5.2q", &longs(&[1])), "Conversion = 'q'");
    assert_eq!(err("%5-d", &longs(&[1])), "Conversion = '5'");
    assert_eq!(err("%-^s", &strings(&["a"])), "Conversion = '-'");
    assert_eq!(err("%é", &longs(&[1])), "Conversion = 'é'");
    assert_eq!(err("%5é", &longs(&[1])), "Conversion = '5'");
    assert_eq!(err("%1$$s", &strings(&["a"])), "Conversion = '1'");
    assert_eq!(err("%,,d", &longs(&[1])), "Flags = ','");
    assert_eq!(err("%007d", &longs(&[1])), "Flags = '0'");
    assert_eq!(err("%<<s", &strings(&["a"])), "Flags = '<'");
    assert_eq!(
        err("%00$s", &strings(&["a"])),
        "Illegal format argument index = 0"
    );
    // The specifier is printed as Java rebuilds it: flags, index, width, precision.
    assert_eq!(err("%1$<s", &strings(&["a"])), "Format specifier '%<s'");
    assert_eq!(err("%<5d %d", &longs(&[1])), "Format specifier '%<5d'");
    assert_eq!(err("%2$-d", &longs(&[1, 2])), "%-2$d");
    assert_eq!(err("%2$-S", &strings(&["a"])), "%-2$S");
    assert_eq!(err("%1$-.2s", &strings(&["a"])), "%-1$.2s");
    assert_eq!(err("%-.2S", &strings(&["a"])), "%-.2S");
    assert_eq!(err("%-.0s|", &strings(&["a"])), "%-.0s");
    assert_eq!(err("%1$0d", &longs(&[1])), "%01$d");
    // Date/time conversions are validated like Java does before being rejected.
    assert_eq!(err("%t", &[]), "Conversion = 't'");
    assert_eq!(err("%t5", &[]), "Conversion = 't'");
    assert_eq!(err("%T", &[]), "Conversion = 'T'");
    assert_eq!(err("%Tq", &longs(&[0])), "Conversion = 'tq'");
    assert_eq!(err("%.1tY", &longs(&[0])), "1");
    assert_eq!(err("%#tY", &longs(&[0])), "Conversion = Y, Flags = #");
    assert_eq!(err("%-tY", &longs(&[0])), "%-tY");
    assert_eq!(err("%1$-TY", &longs(&[0])), "%-1$TY");
    assert_eq!(err("%tY", &[]), "Format specifier '%tY'");
    assert_eq!(err("%<tY", &[]), "Format specifier '%<tY'");
    assert_eq!(
        err("%-5tY", &longs(&[0])),
        "Date/time conversions (%t) are not supported."
    );
    let classes = [
        ("( )", "java.util.HashSet"),
        ("<% %>", "io.warp10.script.WarpScriptStack$Macro"),
        ("test.object", "io.warp10.script.NamedWarpScriptFunction"),
        ("MARK", "io.warp10.script.WarpScriptStack$Mark"),
        (
            "SAVE",
            "io.warp10.script.MemoryWarpScriptStack$StackContext",
        ),
        ("COUNTER", "java.util.concurrent.atomic.AtomicLong"),
        ("'00' HEX->", "[B"),
        ("'00' HEX-> BYTESTOBITS", "java.util.BitSet"),
    ];
    for (script, class) in classes {
        assert_eq!(
            err("%d", &[crate::tests::top(script)]),
            format!("d != {class}"),
            "{script}"
        );
    }
}

#[test]
fn stringformat_function() {
    // `%` is the escape character of WarpScript string literals: formats write `%25`.
    assert_top("'%25d-%25s' [ 1 'a' ] STRINGFORMAT", "1-a");
    assert_top(
        "'%25,d' [ 1234567 ] 'fr' STRINGFORMAT",
        "1\u{202F}234\u{202F}567",
    );
    assert_top("'%25,d' [ 1234567 ] 'en_US' STRINGFORMAT", "1,234,567");
    assert_top("'%25.2f' [ 2.675 ] STRINGFORMAT", "2.68");
    assert_top("'%25s' [ COUNTER 3 COUNTERSET ] STRINGFORMAT", "3");
    assert_fails(
        "'%25d' [ 1 ] 'xx' STRINGFORMAT",
        "STRINGFORMAT expects a valid locale.",
    );
    assert_fails(
        "'%25d' 1 'fr' STRINGFORMAT",
        "STRINGFORMAT expects list of Objects under the locale.",
    );
    assert_fails(
        "'%25d' 1 STRINGFORMAT",
        "STRINGFORMAT expects a locale name or a list of Objects as the first parameter.",
    );
    assert_fails(
        "1 [ 1 ] STRINGFORMAT",
        "STRINGFORMAT expects format under the list of Objects.",
    );
    assert_fails(
        "'%25q' [ 1 ] STRINGFORMAT",
        "STRINGFORMAT expects a valid format. (Conversion = 'q')",
    );
    assert_fails(
        "'%25d' [ 1.5 ] STRINGFORMAT",
        "STRINGFORMAT expects a valid format. (d != java.lang.Double)",
    );
    assert_fails("STRINGFORMAT", "Empty stack.");
    assert_fails("[ 1 ] STRINGFORMAT", "Empty stack.");
    assert_fails("'fr' STRINGFORMAT", "Empty stack.");
}

#[test]
fn widths_and_precisions_are_java_ints() {
    // A width or a precision that does not fit an `int` is ignored, as the formatter ignores it.
    assert_eq!(fmt("%2147483648d", &longs(&[1])), "1");
    assert_eq!(fmt("%9999999999d", &longs(&[1])), "1");
    assert_eq!(fmt("%.2147483648f", &doubles(&[1.5])), "1.500000");
    assert_eq!(fmt("%.99999999999999999999f", &doubles(&[1.5])), "1.500000");
    assert_eq!(fmt("%5d", &longs(&[1])), "    1");
    assert_eq!(err("%.f", &doubles(&[1.5])), "Conversion = '.'");
}
