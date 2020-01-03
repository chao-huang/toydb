///! Evaluates SQL expressions and compares with expectations.
use super::super::types::Value;
use super::super::{Context, Engine, Parser, Plan, Transaction};
use crate::kv;
use crate::Error;

fn eval_expr(expr: &str) -> Result<Value, Error> {
    let engine = super::super::engine::KV::new(kv::MVCC::new(kv::storage::Memory::new()));
    let mut txn = engine.begin()?;
    let ctx = Context { txn: &mut txn };
    let ast = Parser::new(&format!("SELECT {}", expr)).parse()?;
    let mut result = Plan::build(ast)?.optimize()?.execute(ctx)?;
    let value = result.next().unwrap().unwrap().get(0).unwrap().clone();
    txn.rollback()?;
    Ok(value)
}

macro_rules! test_expr {
    ( $( $name:ident: $expr:expr => $expect:expr, )* ) => {
    $(
        #[test]
        fn $name() -> Result<(), Error> {
            let expect: Result<Value, Error> = $expect;
            let actual = eval_expr($expr);
            match expect {
                Ok(Float(e)) if e.is_nan() => match actual {
                    Ok(Float(a)) if a.is_nan() => {},
                    _ => panic!("Expected NaN, got {:?}", actual),
                }
                _ => assert_eq!($expect, actual),
            }
            Ok(())
        }
    )*
    }
}

use Value::*;

test_expr! {
    // Constants and literals
    const_case: "TrUe" => Ok(Boolean(true)),
    const_false: "FALSE" => Ok(Boolean(false)),
    const_infinity: "INFINITY" => Ok(Float(std::f64::INFINITY)),
    const_nan: "NAN" => Ok(Float(std::f64::NAN)),
    const_null: "NULL" => Ok(Null),
    const_true: "TRUE" => Ok(Boolean(true)),

    lit_float: "3.72" => Ok(Float(3.72)),
    lit_float_exp: "3.14e3" => Ok(Float(3140.0)),
    lit_float_exp_neg: "2.718E-2" => Ok(Float(0.02718)),
    lit_float_no_decimal: "3." => Ok(Float(3.0)),
    lit_float_zero_decimal: "3.0" => Ok(Float(3.0)),
    lit_float_max: "1.23456789012345e308" => Ok(Float(1.234_567_890_123_45e308)),
    lit_float_max_neg: "-1.23456789012345e308" => Ok(Float(-1.234_567_890_123_45e308)),
    lit_float_min: "1.23456789012345e-307" => Ok(Float(1.234_567_890_123_45e-307)),
    lit_float_min_neg: "-1.23456789012345e-307" => Ok(Float(-1.234_567_890_123_45e-307)),
    lit_float_min_round: "1.23456789012345e-323" => Ok(Float(1e-323)),
    lit_float_round_53bit: "0.12345678901234567890" => Ok(Float(0.123_456_789_012_345_68)),
    lit_float_overflow: "1e309" => Ok(Float(std::f64::INFINITY)),
    lit_float_underflow: "1e-325" => Ok(Float(0.0)),

    lit_integer: "3" => Ok(Integer(3)),
    lit_integer_multidigit: "314" => Ok(Integer(314)),
    lit_integer_zeroprefix: "03" => Ok(Integer(3)),
    lit_integer_max: "9223372036854775807" => Ok(Integer(9_223_372_036_854_775_807)),
    lit_integer_min: "-9223372036854775807" => Ok(Integer(-9_223_372_036_854_775_807)),
    lit_integer_overflow: "9223372036854775808" => Err(Error::Parse("number too large to fit in target type".into())),
    lit_integer_underflow: "-9223372036854775808" => Err(Error::Parse("number too large to fit in target type".into())),

    lit_string: "'Hi! 👋'" => Ok(String("Hi! 👋".into())),
    lit_string_escape: r#"'Try \n newlines and \t tabs'"# => Ok(String(r#"Try \n newlines and \t tabs"#.into())),
    lit_string_quotes: r#"'Has ''single'' and "double" quotes'"# => Ok(String(r#"Has 'single' and "double" quotes"#.into())),
    lit_string_whitespace: "' Has \n newlines and \t tabs  '" => Ok(String(" Has \n newlines and \t tabs  ".into())),
    lit_string_long: &format!("'{}'", "a".repeat(4096)) => Ok("a".repeat(4096).into()),

    // Logical operators
    op_and_true_true: "TRUE AND TRUE" => Ok(Boolean(true)),
    op_and_true_false: "TRUE AND FALSE" => Ok(Boolean(false)),
    op_and_false_true: "FALSE AND TRUE" => Ok(Boolean(false)),
    op_and_false_false: "FALSE AND FALSE" => Ok(Boolean(false)),
    op_and_true_null: "TRUE AND NULL" => Ok(Null),
    op_and_false_null: "FALSE AND NULL" => Ok(Boolean(false)),
    op_and_null_true: "NULL AND TRUE" => Ok(Null),
    op_and_null_false: "NULL AND FALSE" => Ok(Boolean(false)),
    op_and_null_null: "NULL AND NULL" => Ok(Null),
    op_and_error_float: "3.14 AND 3.14" => Err(Error::Value("Can't and 3.14 and 3.14".into())),
    op_and_error_integer: "3 AND 3" => Err(Error::Value("Can't and 3 and 3".into())),
    op_and_error_string: "'a' AND 'b'" => Err(Error::Value("Can't and a and b".into())),

    op_not_true: "NOT TRUE" => Ok(Boolean(false)),
    op_not_false: "NOT FALSE" => Ok(Boolean(true)),
    op_not_null: "NOT NULL" => Ok(Null),
    op_not_error_float: "NOT 3.14" => Err(Error::Value("Can't negate 3.14".into())),
    op_not_error_integer: "NOT 3" => Err(Error::Value("Can't negate 3".into())),
    op_not_error_string: "NOT 'abc'" => Err(Error::Value("Can't negate abc".into())),

    op_or_true_true: "TRUE OR TRUE" => Ok(Boolean(true)),
    op_or_true_false: "TRUE OR FALSE" => Ok(Boolean(true)),
    op_or_false_true: "FALSE OR TRUE" => Ok(Boolean(true)),
    op_or_false_false: "FALSE OR FALSE" => Ok(Boolean(false)),
    op_or_true_null: "TRUE OR NULL" => Ok(Boolean(true)),
    op_or_false_null: "FALSE OR NULL" => Ok(Null),
    op_or_null_true: "NULL OR TRUE" => Ok(Boolean(true)),
    op_or_null_false: "NULL OR FALSE" => Ok(Null),
    op_or_null_null: "NULL OR NULL" => Ok(Null),
    op_or_error_float: "3.14 OR 3.14" => Err(Error::Value("Can't or 3.14 and 3.14".into())),
    op_or_error_integer: "3 OR 3" => Err(Error::Value("Can't or 3 and 3".into())),
    op_or_error_string: "'a' OR 'b'" => Err(Error::Value("Can't or a and b".into())),

    // Comparison operators
    op_eq_bool: "TRUE = TRUE" => Ok(Boolean(true)),
    op_eq_bool_not: "TRUE = FALSE" => Ok(Boolean(false)),
    op_eq_float: "3.14 = 3.14" => Ok(Boolean(true)),
    op_eq_float_not: "3.14 = 2.718" => Ok(Boolean(false)),
    op_eq_float_infinity: "INFINITY = INFINITY" => Ok(Boolean(true)),
    op_eq_float_nan: "NAN = NAN" => Ok(Boolean(false)),
    op_eq_float_int: "3.0 = 3" => Ok(Boolean(true)),
    op_eq_float_int_not: "3.01 = 3" => Ok(Boolean(false)),
    op_eq_int: "1 = 1" => Ok(Boolean(true)),
    op_eq_int_not: "1 = 2" => Ok(Boolean(false)),
    op_eq_int_float: "3 = 3.0" => Ok(Boolean(true)),
    op_eq_int_float_not: "3 = 3.01" => Ok(Boolean(false)),
    op_eq_null: "NULL = NULL" => Ok(Null),
    op_eq_null_int: "NULL = 1" => Ok(Null),
    op_eq_int_null: "1 = NULL" => Ok(Null),
    op_eq_string: "'abc' = 'abc'" => Ok(Boolean(true)),
    op_eq_string_not: "'abc' = 'xyz'" => Ok(Boolean(false)),
    op_eq_string_case: "'abc' = 'ABC'" => Ok(Boolean(false)),
    op_eq_string_unicode: "'😀' = '😀'" => Ok(Boolean(true)),
    op_eq_string_unicode_not: "'😀' = '🙁'" => Ok(Boolean(false)),
    op_eq_conflict: "1 = 'a'" => Err(Error::Value("Can't compare 1 and a".into())),

    op_neq_bool: "TRUE != FALSE" => Ok(Boolean(true)),
    op_neq_bool_not: "TRUE != TRUE" => Ok(Boolean(false)),
    op_neq_float: "3.14 != 2.718" => Ok(Boolean(true)),
    op_neq_float_not: "3.14 != 3.14" => Ok(Boolean(false)),
    op_neq_float_infinity: "INFINITY != INFINITY" => Ok(Boolean(false)),
    op_neq_float_nan: "NAN != NAN" => Ok(Boolean(true)),
    op_neq_float_int: "3.0 != 4" => Ok(Boolean(true)),
    op_neq_float_int_not: "3.0 != 3" => Ok(Boolean(false)),
    op_neq_int: "1 != 2" => Ok(Boolean(true)),
    op_neq_int_not: "1 != 1" => Ok(Boolean(false)),
    op_neq_int_float: "3 != 3.01" => Ok(Boolean(true)),
    op_neq_int_float_not: "3 != 3.0" => Ok(Boolean(false)),
    op_neq_null: "NULL != NULL" => Ok(Null),
    op_neq_null_int: "NULL != 1" => Ok(Null),
    op_neq_int_null: "1 != NULL" => Ok(Null),
    op_neq_string: "'abc' != 'xyz'" => Ok(Boolean(true)),
    op_neq_string_not: "'abc' != 'abc'" => Ok(Boolean(false)),
    op_neq_string_case: "'abc' != 'ABC'" => Ok(Boolean(true)),
    op_neq_string_unicode: "'😀' != '🙁'" => Ok(Boolean(true)),
    op_neq_string_unicode_not: "'😀' != '😀'" => Ok(Boolean(false)),
    op_neq_conflict: "1 != 'a'" => Err(Error::Value("Can't compare 1 and a".into())),

    op_gt_bool: "TRUE > FALSE" => Ok(Boolean(true)),
    op_gt_bool_eq: "TRUE > TRUE" => Ok(Boolean(false)),
    op_gt_bool_not: "FALSE > TRUE" => Ok(Boolean(false)),
    op_gt_float: "3.14 > 3.13" => Ok(Boolean(true)),
    op_gt_float_eq: "3.14 > 3.14" => Ok(Boolean(false)),
    op_gt_float_not: "3.14 > 3.15" => Ok(Boolean(false)),
    op_gt_float_infinity: "INFINITY > INFINITY" => Ok(Boolean(false)),
    op_gt_float_nan: "NAN > NAN" => Ok(Boolean(false)),
    op_gt_float_int: "3.01 > 3" => Ok(Boolean(true)),
    op_gt_float_int_eq: "3.0 > 3" => Ok(Boolean(false)),
    op_gt_float_int_not: "2.99 > 3" => Ok(Boolean(false)),
    op_gt_int: "2 > 1" => Ok(Boolean(true)),
    op_gt_int_eq: "1 > 1" => Ok(Boolean(false)),
    op_gt_int_not: "1 > 2" => Ok(Boolean(false)),
    op_gt_int_float: "3 > 2.99" => Ok(Boolean(true)),
    op_gt_int_float_eq: "3 > 3.00" => Ok(Boolean(false)),
    op_gt_int_float_not: "3 > 3.01" => Ok(Boolean(false)),
    op_gt_null: "NULL > NULL" => Ok(Null),
    op_gt_null_int: "NULL > 1" => Ok(Null),
    op_gt_int_null: "1 > NULL" => Ok(Null),
    op_gt_string: "'xyz' > 'abc'" => Ok(Boolean(true)),
    op_gt_string_eq: "'abc' > 'abc'" => Ok(Boolean(false)),
    op_gt_string_not: "'abc' > 'xyz'" => Ok(Boolean(false)),
    op_gt_string_case: "'b' > 'A'" => Ok(Boolean(true)),
    op_gt_string_case_eq: "'A' > 'a'" => Ok(Boolean(false)),
    op_gt_string_case_not: "'B' > 'a'" => Ok(Boolean(false)),
    op_gt_string_prefix: "'abcde' > 'abc'" => Ok(Boolean(true)),
    op_gt_string_prefix_not: "'abc' > 'abcde'" => Ok(Boolean(false)),
    op_gt_string_unicode: "'🙁' > '😀'" => Ok(Boolean(true)),
    op_gt_string_unicode_eq: "'😀' > '😀'" => Ok(Boolean(false)),
    op_gt_string_unicode_not: "'😀' > '🙁'" => Ok(Boolean(false)),
    op_gt_conflict: "1 > 'a'" => Err(Error::Value("Can't compare 1 and a".into())),

    op_gte_bool: "TRUE >= TRUE" => Ok(Boolean(true)),
    op_gte_bool_gt: "TRUE >= FALSE" => Ok(Boolean(true)),
    op_gte_bool_not: "FALSE >= TRUE" => Ok(Boolean(false)),
    op_gte_float: "3.14 >= 3.14" => Ok(Boolean(true)),
    op_gte_float_gt: "3.15 >= 3.14" => Ok(Boolean(true)),
    op_gte_float_not: "3.14 >= 3.15" => Ok(Boolean(false)),
    op_gte_float_infinity: "INFINITY >= INFINITY" => Ok(Boolean(true)),
    op_gte_float_nan: "NAN >= NAN" => Ok(Boolean(false)),
    op_gte_float_int: "3.0 >= 3" => Ok(Boolean(true)),
    op_gte_float_int_gt: "3.01 >= 3" => Ok(Boolean(true)),
    op_gte_float_int_not: "2.99 >= 3" => Ok(Boolean(false)),
    op_gte_int: "1 >= 1" => Ok(Boolean(true)),
    op_gte_int_gt: "2 >= 1" => Ok(Boolean(true)),
    op_gte_int_not: "1 >= 2" => Ok(Boolean(false)),
    op_gte_int_float: "3 >= 3.0" => Ok(Boolean(true)),
    op_gte_int_float_gt: "3 >= 2.99" => Ok(Boolean(true)),
    op_gte_int_float_not: "3 >= 3.01" => Ok(Boolean(false)),
    op_gte_null: "NULL >= NULL" => Ok(Null),
    op_gte_null_int: "NULL >= 1" => Ok(Null),
    op_gte_int_null: "1 >= NULL" => Ok(Null),
    op_gte_string: "'abc' >= 'abc'" => Ok(Boolean(true)),
    op_gte_string_gt: "'b' >= 'abc'" => Ok(Boolean(true)),
    op_gte_string_not: "'abc' >= 'xyz'" => Ok(Boolean(false)),
    op_gte_string_case: "'ABC' >= 'abc'" => Ok(Boolean(false)),
    op_gte_string_case_not: "'B' >= 'a'" => Ok(Boolean(false)),
    op_gte_string_prefix: "'abcde' >= 'abc'" => Ok(Boolean(true)),
    op_gte_string_prefix_not: "'abc' >= 'abcde'" => Ok(Boolean(false)),
    op_gte_string_unicode: "'😀' >= '😀'" => Ok(Boolean(true)),
    op_gte_string_unicode_gt: "'🙁' >= '😀'" => Ok(Boolean(true)),
    op_gte_string_unicode_not: "'😀' >= '🙁'" => Ok(Boolean(false)),
    op_gte_conflict: "1 >= 'a'" => Err(Error::Value("Can't compare 1 and a".into())),

    op_lt_bool: "FALSE < TRUE" => Ok(Boolean(true)),
    op_lt_bool_eq: "TRUE < TRUE" => Ok(Boolean(false)),
    op_lt_bool_not: "TRUE < FALSE" => Ok(Boolean(false)),
    op_lt_float: "3.13 < 3.14" => Ok(Boolean(true)),
    op_lt_float_eq: "3.14 < 3.14" => Ok(Boolean(false)),
    op_lt_float_not: "3.15 < 3.14" => Ok(Boolean(false)),
    op_lt_float_infinity: "INFINITY < INFINITY" => Ok(Boolean(false)),
    op_lt_float_nan: "NAN < NAN" => Ok(Boolean(false)),
    op_lt_float_int: "2.99 < 3" => Ok(Boolean(true)),
    op_lt_float_int_eq: "3.0 < 3" => Ok(Boolean(false)),
    op_lt_float_int_not: "3.01 < 3" => Ok(Boolean(false)),
    op_lt_int: "1 < 2" => Ok(Boolean(true)),
    op_lt_int_eq: "1 < 1" => Ok(Boolean(false)),
    op_lt_int_not: "2 < 1" => Ok(Boolean(false)),
    op_lt_int_float: "3 < 3.1" => Ok(Boolean(true)),
    op_lt_int_float_eq: "3 < 3.00" => Ok(Boolean(false)),
    op_lt_int_float_not: "3 < 2.99" => Ok(Boolean(false)),
    op_lt_null: "NULL < NULL" => Ok(Null),
    op_lt_null_int: "NULL < 1" => Ok(Null),
    op_lt_int_null: "1 < NULL" => Ok(Null),
    op_lt_string: "'abc' < 'xyz'" => Ok(Boolean(true)),
    op_lt_string_eq: "'abc' < 'abc'" => Ok(Boolean(false)),
    op_lt_string_not: "'xyz' < 'abc'" => Ok(Boolean(false)),
    op_lt_string_case: "'A' < 'b'" => Ok(Boolean(true)),
    op_lt_string_case_eq: "'a' < 'A'" => Ok(Boolean(false)),
    op_lt_string_case_not: "'a' < 'B'" => Ok(Boolean(false)),
    op_lt_string_prefix: "'abc' < 'abcde'" => Ok(Boolean(true)),
    op_lt_string_prefix_not: "'abcde' < 'abc'" => Ok(Boolean(false)),
    op_lt_string_unicode: "'😀' < '🙁'" => Ok(Boolean(true)),
    op_lt_string_unicode_eq: "'😀' < '😀'" => Ok(Boolean(false)),
    op_lt_string_unicode_not: "'🙁' < '😀'" => Ok(Boolean(false)),
    op_lt_conflict: "1 < 'a'" => Err(Error::Value("Can't compare 1 and a".into())),

    op_lte_bool: "TRUE <= TRUE" => Ok(Boolean(true)),
    op_lte_bool_lt: "FALSE <= TRUE" => Ok(Boolean(true)),
    op_lte_bool_not: "TRUE <= FALSE" => Ok(Boolean(false)),
    op_lte_float: "3.14 <= 3.14" => Ok(Boolean(true)),
    op_lte_float_lt: "3.14 <= 3.15" => Ok(Boolean(true)),
    op_lte_float_not: "3.15 <= 3.14" => Ok(Boolean(false)),
    op_lte_float_infinity: "INFINITY <= INFINITY" => Ok(Boolean(true)),
    op_lte_float_nan: "NAN <= NAN" => Ok(Boolean(false)),
    op_lte_float_int: "3.0 <= 3" => Ok(Boolean(true)),
    op_lte_float_int_lt: "3.01 <= 4" => Ok(Boolean(true)),
    op_lte_float_int_not: "3.01 <= 3" => Ok(Boolean(false)),
    op_lte_int: "1 <= 1" => Ok(Boolean(true)),
    op_lte_int_lt: "1 <= 2" => Ok(Boolean(true)),
    op_lte_int_not: "2 <= 1" => Ok(Boolean(false)),
    op_lte_int_float: "3 <= 3.0" => Ok(Boolean(true)),
    op_lte_int_float_lt: "3 <= 3.01" => Ok(Boolean(true)),
    op_lte_int_float_not: "3 <= 2.99" => Ok(Boolean(false)),
    op_lte_null: "NULL <= NULL" => Ok(Null),
    op_lte_null_int: "NULL <= 1" => Ok(Null),
    op_lte_int_null: "1 <= NULL" => Ok(Null),
    op_lte_string: "'abc' <= 'abc'" => Ok(Boolean(true)),
    op_lte_string_lt: "'a' <= 'abc'" => Ok(Boolean(true)),
    op_lte_string_not: "'xyz' <= 'abc'" => Ok(Boolean(false)),
    op_lte_string_case: "'abc' <= 'ABC'" => Ok(Boolean(false)),
    op_lte_string_case_not: "'a' <= 'B'" => Ok(Boolean(false)),
    op_lte_string_prefix: "'abc' <= 'abcde'" => Ok(Boolean(true)),
    op_lte_string_prefix_not: "'abcde' <= 'abc'" => Ok(Boolean(false)),
    op_lte_string_unicode: "'😀' <= '😀'" => Ok(Boolean(true)),
    op_lte_string_unicode_lt: "'😀' <= '🙁'" => Ok(Boolean(true)),
    op_lte_string_unicode_not: "'🙁' <= '😀'" => Ok(Boolean(false)),
    op_lte_conflict: "1 <= 'a'" => Err(Error::Value("Can't compare 1 and a".into())),

    op_like_percent: "'abcde' LIKE 'a%e'" => Ok(Boolean(true)),
    op_like_percent_escape: "'ab%de' LIKE 'ab%%de'" => Ok(Boolean(true)),
    op_like_percent_escape_not: "'ab%de' LIKE 'a%%e'" => Ok(Boolean(false)),
    op_like_percent_none: "'abcde' LIKE 'abc%de'" => Ok(Boolean(true)),
    op_like_percent_prefix: "'abcde' LIKE 'abc%'" => Ok(Boolean(true)),
    op_like_percent_suffix: "'abcde' LIKE '%cde'" => Ok(Boolean(true)),
    op_like_percent_not: "'abcdef' LIKE 'a%e'" => Ok(Boolean(false)),
    op_like_underscore: "'abc' LIKE 'a_c'" => Ok(Boolean(true)),
    op_like_underscore_not: "'abb' LIKE 'a_c'" => Ok(Boolean(false)),
    op_like_underscore_escape: "'ab_de' LIKE 'ab__de'" => Ok(Boolean(true)),
    op_like_underscore_escape_not: "'abcde' LIKE 'ab__de'" => Ok(Boolean(false)),
    op_like_star: "'abcde' LIKE 'a*bcde'" => Ok(Boolean(false)),
    op_like_question: "'abc' LIKE 'a?bc'" => Ok(Boolean(false)),
    op_like_multi: "'abcdefghijklmno' LIKE 'a_c%f%i_kl%mno'" => Ok(Boolean(true)),
    op_like_case: "'abcde' LIKE 'A%E'" => Ok(Boolean(false)),
    op_like_eq: "'abc' LIKE 'abc'" => Ok(Boolean(true)),
    op_like_neq: "'xyz' LIKE 'abc'" => Ok(Boolean(false)),
    op_like_null: "'abc' LIKE NULL" => Ok(Null),
    op_like_null_lhs: "NULL LIKE 'abc'" => Ok(Null),

    op_null: "NULL IS NULL" => Ok(Boolean(true)),
    op_null_not: "NULL IS NOT NULL" => Ok(Boolean(false)),
    op_null_bool: "TRUE IS NULL" => Ok(Boolean(false)),
    op_null_bool_not: "TRUE IS NOT NULL" => Ok(Boolean(true)),
    op_null_rhs_bool: "NULL IS TRUE" => Err(Error::Parse("Expected token NULL, found TRUE".into())),

    // Math operators
    op_add_float_float: "3.1 + 2.71" => Ok(Float(3.1 + 2.71)),
    op_add_float_int: "3.72 + 1" => Ok(Float(3.72 + 1.0)),
    op_add_float_null: "3.14 + NULL" => Ok(Null),
    op_add_int_float: "1 + 3.72" => Ok(Float(1.0 + 3.72)),
    op_add_int_int: "1 + 2" => Ok(Integer(3)),
    op_add_int_null: "1 + NULL" => Ok(Null),
    op_add_null_float: "NULL + 3.14" => Ok(Null),
    op_add_null_int: "NULL + 1" => Ok(Null),
    op_add_null_null: "NULL + NULL" => Ok(Null),
    op_add_negative: "1 + -3" => Ok(Integer(-2)),
    op_add_infinity: "1 + INFINITY" => Ok(Float(std::f64::INFINITY)),
    op_add_nan: "1 + NAN" => Ok(Float(std::f64::NAN)),
    op_add_overflow_int: "9223372036854775807 + 1" => Err(Error::Value("Integer overflow".into())),
    op_add_underflow_int: "-9223372036854775807 + -2" => Err(Error::Value("Integer overflow".into())),
    op_add_overflow_float: "2e308 + 2e308" => Ok(Float(std::f64::INFINITY)),
    op_add_round_int_float: "9223372036854775807 + 10.0" => Ok(Float(9_223_372_036_854_776_000.0)),
    op_add_error_bool: "TRUE + FALSE" => Err(Error::Value("Can't add TRUE and FALSE".into())),
    op_add_error_strings: "'a' + 'b'" => Err(Error::Value("Can't add a and b".into())),

    op_assert_float: "+3.72" => Ok(Float(3.72)),
    op_assert_int: "+1" => Ok(Integer(1)),
    op_assert_null: "+NULL" => Ok(Null),
    op_assert_infinity: "+INFINITY" => Ok(Float(std::f64::INFINITY)),
    op_assert_nan: "+NAN" => Ok(Float(std::f64::NAN)),
    op_assert_multi: "+++1" => Ok(Integer(1)),
    op_assert_error_bool: "+TRUE" => Err(Error::Value("Can't take the positive of TRUE".into())),
    op_assert_error_string: "+'abc'" => Err(Error::Value("Can't take the positive of abc".into())),

    op_divide_float_float: "4.16 / 3.2" => Ok(Float(1.3)),
    op_divide_float_float_zero: "4.16 / 0.0" => Ok(Float(std::f64::INFINITY)),
    op_divide_float_float_zero_zero: "0.0 / 0.0" => Ok(Float(std::f64::NAN)),
    op_divide_float_integer: "1.5 / 3" => Ok(Float(0.5)),
    op_divide_float_integer_zero: "4.16 / 0" => Ok(Float(std::f64::INFINITY)),
    op_divide_float_null: "4.16 / NULL" => Ok(Null),
    op_divide_integer_float: "3 / 1.2" => Ok(Float(2.5)),
    op_divide_integer_float_zero: "3 / 0.0" => Ok(Float(std::f64::INFINITY)),
    op_divide_integer_integer: "8 / 3" => Ok(Integer(2)),
    op_divide_integer_integer_negative: "8 / -3" => Ok(Integer(-2)),
    op_divide_integer_integer_zero: "1 / 0" => Err(Error::Value("Can't divide by zero".into())),
    op_divide_integer_null: "1 / NULL" => Ok(Null),
    op_divide_infinity: "1 / INFINITY" => Ok(Float(0.0)),
    op_divide_infinity_divisor: "INFINITY / 10" => Ok(Float(std::f64::INFINITY)),
    op_divide_infinity_infinity: "INFINITY / INFINITY" => Ok(Float(std::f64::NAN)),
    op_divide_nan: "1 / NAN" => Ok(Float(std::f64::NAN)),
    op_divide_null_float: "NULL / 3.14" => Ok(Null),
    op_divide_null_integer: "NULL / 1" => Ok(Null),
    op_divide_null_null: "NULL / NULL" => Ok(Null),
    op_divide_error_bool: "TRUE / FALSE" => Err(Error::Value("Can't divide TRUE and FALSE".into())),
    op_divide_error_strings: "'a' / 'b'" => Err(Error::Value("Can't divide a and b".into())),

    op_exp_float_float: "6.25 ^ 0.5" => Ok(Float(2.5)),
    op_exp_float_int: "6.25 ^ 2" => Ok(Float(39.0625)),
    op_exp_float_null: "3.14 ^ NULL" => Ok(Null),
    op_exp_int_float: "9 ^ 0.5" => Ok(Float(3.0)),
    op_exp_int_int: "2 ^ 3" => Ok(Integer(8)),
    op_exp_int_int_large: "2 ^ 10000000000" => Err(Error::Value("Integer overflow".into())),
    op_exp_int_null: "1 ^ NULL" => Ok(Null),
    op_exp_null_float: "NULL ^ 3.14" => Ok(Null),
    op_exp_null_int: "NULL ^ 1" => Ok(Null),
    op_exp_null_null: "NULL ^ NULL" => Ok(Null),
    op_exp_infinity: "INFINITY ^ 2" => Ok(Float(std::f64::INFINITY)),
    op_exp_infinity_exp: "2 ^ INFINITY" => Ok(Float(std::f64::INFINITY)),
    op_exp_infinity_infinity: "INFINITY ^ INFINITY" => Ok(Float(std::f64::INFINITY)),
    op_exp_nan: "NAN ^ 2" => Ok(Float(std::f64::NAN)),
    op_exp_nan_exp: "2 ^ NAN" => Ok(Float(std::f64::NAN)),
    op_exp_overflow_float: "10e200 ^ 2" => Ok(Float(std::f64::INFINITY)),
    op_exp_overflow_int: "9223372036854775807 ^ 2" => Err(Error::Value("Integer overflow".into())),
    op_exp_negative: "2 ^ -3" => Ok(Float(0.125)),
    op_exp_error_bool: "TRUE ^ FALSE" => Err(Error::Value("Can't exponentiate TRUE and FALSE".into())),
    op_exp_error_strings: "'a' ^ 'b'" => Err(Error::Value("Can't exponentiate a and b".into())),

    op_factorial: "3!" => Ok(Integer(6)),
    op_factorial_zero: "0!" => Ok(Integer(1)),
    op_factorial_null: "NULL!" => Ok(Null),
    op_factorial_error_bool: "TRUE!" => Err(Error::Value("Can't take factorial of TRUE".into())),
    op_factorial_error_float: "3.14!" => Err(Error::Value("Can't take factorial of 3.14".into())),
    op_factorial_error_negative: "-3!" => Err(Error::Value("Can't take factorial of negative number".into())),
    op_factorial_error_string: "'abc'!" => Err(Error::Value("Can't take factorial of abc".into())),

    op_modulo_float_float: "6.28 % 2.2" => Ok(Float(1.88)),
    op_modulo_float_float_zero: "6.28 % 0.0" => Ok(Float(std::f64::NAN)),
    op_modulo_float_int: "3.15 % 2" => Ok(Float(1.15)),
    op_modulo_float_null: "3.14 % NULL" => Ok(Null),
    op_modulo_int_float: "6 % 3.15" => Ok(Float(2.85)),
    op_modulo_int_int: "5 % 3" => Ok(Integer(2)),
    op_modulo_int_int_zero: "7 % 0" => Err(Error::Value("Can't divide by zero".into())),
    op_modulo_int_null: "1 % NULL" => Ok(Null),
    op_modulo_null_float: "NULL % 3.14" => Ok(Null),
    op_modulo_null_int: "NULL % 1" => Ok(Null),
    op_modulo_null_null: "NULL % NULL" => Ok(Null),
    op_modulo_negative: "-5 % 3" => Ok(Integer(-2)),
    op_modulo_negative_rhs: "5 % -3" => Ok(Integer(2)),
    op_modulo_infinity: "INFINITY % 7" => Ok(Float(std::f64::NAN)),
    op_modulo_infinity_divisor: "7 % INFINITY" => Ok(Float(7.0)),
    op_modulo_nan: "7 % NAN" => Ok(Float(std::f64::NAN)),
    op_modulo_error_bool: "TRUE % FALSE" => Err(Error::Value("Can't take modulo of TRUE and FALSE".into())),
    op_modulo_error_strings: "'a' % 'b'" => Err(Error::Value("Can't take modulo of a and b".into())),

    op_multiply_float_float: "3.1 * 2.71" => Ok(Float(3.1 * 2.71)),
    op_multiply_float_int: "3.72 * 1" => Ok(Float(3.72 * 1.0)),
    op_multiply_float_null: "3.14 * NULL" => Ok(Null),
    op_multiply_int_float: "1 * 3.72" => Ok(Float(1.0 * 3.72)),
    op_multiply_int_int: "2 * 3" => Ok(Integer(6)),
    op_multiply_int_null: "1 * NULL" => Ok(Null),
    op_multiply_null_float: "NULL * 3.14" => Ok(Null),
    op_multiply_null_int: "NULL * 1" => Ok(Null),
    op_multiply_null_null: "NULL * NULL" => Ok(Null),
    op_multiply_negative: "2 * -3" => Ok(Integer(-6)),
    op_multiply_infinity: "2 * INFINITY" => Ok(Float(std::f64::INFINITY)),
    op_multiply_nan: "2 * NAN" => Ok(Float(std::f64::NAN)),
    op_multiply_overflow_int: "9223372036854775807 * 2" => Err(Error::Value("Integer overflow".into())),
    op_multiply_underflow_int: "9223372036854775807 * -2" => Err(Error::Value("Integer overflow".into())),
    op_multiply_overflow_float: "2e308 * 2" => Ok(Float(std::f64::INFINITY)),
    op_multiply_round_int_float: "9223372036854775807 * 2.0" => Ok(Float(18_446_744_073_709_552_000.0)),
    op_multiply_error_bool: "TRUE * FALSE" => Err(Error::Value("Can't multiply TRUE and FALSE".into())),
    op_multiply_error_strings: "'a' * 'b'" => Err(Error::Value("Can't multiply a and b".into())),

    op_negate: "-1" => Ok(Integer(-1)),
    op_negate_double: "--1" => Ok(Integer(1)),
    op_negate_float: "-3.72" => Ok(Float(-3.72)),
    op_negate_mixed: "-+-+-1" => Ok(Integer(-1)),
    op_negate_multi: "---1" => Ok(Integer(-1)),
    op_negate_null: "-NULL" => Ok(Null),
    op_negate_infinity: "-INFINITY" => Ok(Float(-std::f64::INFINITY)),
    op_negate_nan: "-NAN" => Ok(Float(std::f64::NAN)),
    op_negate_error_bool: "-TRUE" => Err(Error::Value("Can't negate TRUE".into())),
    op_negate_error_string: "-'abc'" => Err(Error::Value("Can't negate abc".into())),

    op_subtract_float_float: "3.1 - 2.71" => Ok(Float(3.1 - 2.71)),
    op_subtract_float_int: "3.72 - 1" => Ok(Float(3.72 - 1.0)),
    op_subtract_float_null: "3.14 - NULL" => Ok(Null),
    op_subtract_int_float: "1 - 3.72" => Ok(Float(1.0 - 3.72)),
    op_subtract_int_int: "1 - 2" => Ok(Integer(-1)),
    op_subtract_int_null: "1 - NULL" => Ok(Null),
    op_subtract_null_float: "NULL - 3.14" => Ok(Null),
    op_subtract_null_int: "NULL - 1" => Ok(Null),
    op_subtract_null_null: "NULL - NULL" => Ok(Null),
    op_subtract_negative: "1 - -3" => Ok(Integer(4)),
    op_subtract_infinity: "1 - INFINITY" => Ok(Float(-std::f64::INFINITY)),
    op_subtract_nan: "1 - NAN" => Ok(Float(std::f64::NAN)),
    op_subtract_overflow_int: "9223372036854775807 - -1" => Err(Error::Value("Integer overflow".into())),
    op_subtract_underflow_int: "-9223372036854775807 - 2" => Err(Error::Value("Integer overflow".into())),
    op_subtract_overflow_float: "2e308 - -2e308" => Ok(Float(std::f64::INFINITY)),
    op_subtract_round_int_float: "9223372036854775807 - -10.0" => Ok(Float(9_223_372_036_854_776_000.0)),
    op_subtract_error_bool: "TRUE - FALSE" => Err(Error::Value("Can't subtract TRUE and FALSE".into())),
    op_subtract_error_strings: "'a' - 'b'" => Err(Error::Value("Can't subtract a and b".into())),

    // Operator precedence, testing each operator against the ones at the same level and immediately
    // below it in order.
    op_prec_negate_factorial: "-3!" => Err(Error::Value("Can't take factorial of negative number".into())),
    op_prec_negate_factorial_paren: "-(3!)" => Ok(Integer(-6)),
    op_prec_negate_is: "-NULL IS NULL" => Ok(Boolean(true)),
    op_prec_negate_is_paren: "-(NULL IS NULL)" => Err(Error::Value("Can't negate TRUE".into())),

    op_prec_not_factorial: "NOT NULL IS NULL" => Ok(Boolean(true)),
    op_prec_not_factorial_paren: "NOT (NULL IS NULL)" => Ok(Boolean(false)),
    op_prec_not_is: "NOT NULL IS NULL" => Ok(Boolean(true)),
    op_prec_not_is_paren: "NOT (NULL IS NULL)" => Ok(Boolean(false)),

    op_prec_factorial_exp: "2 ^ 3!" => Ok(Integer(64)),
    op_prec_factorial_exp_paren: "(2 ^ 3)!" => Ok(Integer(40320)),

    op_prec_is_exp: "2^NULL IS NULL" => Err(Error::Value("Can't exponentiate 2 and TRUE".into())),
    op_prec_is_exp_paren: "(2^NULL) IS NULL" => Ok(Boolean(true)),

    op_assoc_exp: "2^3^2" => Ok(Integer(512)),
    op_assoc_exp_paren: "(2^3)^2" => Ok(Integer(64)),
    op_prec_exp_multiply: "2^3*4" => Ok(Integer(32)),
    op_prec_exp_multiply_paren: "2^(3*4)" => Ok(Integer(4096)),
    op_prec_exp_divide: "2^4/2" => Ok(Integer(8)),
    op_prec_exp_divide_paren: "2^(4/2)" => Ok(Integer(4)),
    op_prec_exp_modulo: "2^5%2" => Ok(Integer(0)),
    op_prec_exp_modulo_paren: "2^(5%2)" => Ok(Integer(2)),

    op_prec_multiply_divide: "3 * 4 / 2" => Ok(Integer(6)),
    op_prec_multiply_modulo: "3 * 4 % 3" => Ok(Integer(0)),
    op_prec_multiply_add: "1 + 2 * 3" => Ok(Integer(7)),
    op_prec_multiply_add_paren: "(1 + 2) * 3" => Ok(Integer(9)),
    op_prec_multiply_subtract: "1 - 2 * 3" => Ok(Integer(-5)),
    op_prec_multiply_subtract_paren: "(1 - 2) * 3" => Ok(Integer(-3)),

    op_prec_divide_multiply: "4 / 2 * 3" => Ok(Integer(6)),
    op_prec_divide_modulo: "8 / 4 % 3" => Ok(Integer(2)),
    op_prec_divide_add: "2 + 4 / 2" => Ok(Integer(4)),
    op_prec_divide_add_paren: "(2 + 4) / 2" => Ok(Integer(3)),
    op_prec_divide_subtract: "4 - 2 / 2" => Ok(Integer(3)),
    op_prec_divide_subtract_paren: "(4 - 2) / 2" => Ok(Integer(1)),

    op_prec_modulo_multiply: "4 % 3 * 3" => Ok(Integer(3)),
    op_prec_modulo_divide: "8 % 3 / 2" => Ok(Integer(1)),
    op_prec_modulo_add: "2 + 4 % 3" => Ok(Integer(3)),
    op_prec_modulo_add_paren: "(2 + 4) % 3" => Ok(Integer(0)),
    op_prec_modulo_subtract: "8 - 5 % 3" => Ok(Integer(6)),
    op_prec_modulo_subtract_paren: "(8 - 5) % 3" => Ok(Integer(0)),

    op_prec_add_subtract: "1 + 2 - 3" => Ok(Integer(0)),
    op_prec_add_gt: "1 + 2 > 2" => Ok(Boolean(true)),
    op_prec_add_gt_paren: "1 + (2 > 2)" => Err(Error::Value("Can't add 1 and FALSE".into())),
    op_prec_add_gte: "1 + 2 >= 2" => Ok(Boolean(true)),
    op_prec_add_gte_paren: "1 + (2 >= 2)" => Err(Error::Value("Can't add 1 and TRUE".into())),
    op_prec_add_lt: "1 + 2 < 2" => Ok(Boolean(false)),
    op_prec_add_lt_paren: "1 + (2 < 2)" => Err(Error::Value("Can't add 1 and FALSE".into())),
    op_prec_add_lte: "1 + 2 <= 2" => Ok(Boolean(false)),
    op_prec_add_lte_paren: "1 + (2 <= 2)" => Err(Error::Value("Can't add 1 and TRUE".into())),

    op_prec_subtract_add: "3 - 2 + 1" => Ok(Integer(2)),
    op_prec_subtract_gt: "5 - 2 > 2" => Ok(Boolean(true)),
    op_prec_subtract_gt_paren: "5 - (2 > 2)" => Err(Error::Value("Can't subtract 5 and FALSE".into())),
    op_prec_subtract_gte: "5 - 2 >= 2" => Ok(Boolean(true)),
    op_prec_subtract_gte_paren: "5 - (2 >= 2)" => Err(Error::Value("Can't subtract 5 and TRUE".into())),
    op_prec_subtract_lt: "5 - 2 < 2" => Ok(Boolean(false)),
    op_prec_subtract_lt_paren: "5 - (2 < 2)" => Err(Error::Value("Can't subtract 5 and FALSE".into())),
    op_prec_subtract_lte: "5 - 2 <= 2" => Ok(Boolean(false)),
    op_prec_subtract_lte_paren: "5 - (2 <= 2)" => Err(Error::Value("Can't subtract 5 and TRUE".into())),

    op_prec_gt_gte: "5 > 3 >= TRUE" => Ok(Boolean(true)),
    op_prec_gt_lt: "5 > 3 < TRUE" => Ok(Boolean(false)),
    op_prec_gt_lte: "5 > 3 <= TRUE" => Ok(Boolean(true)),
    op_prec_gt_eq: "5 > 3 = TRUE" => Ok(Boolean(true)),
    op_prec_gt_eq_paren: "5 > (3 = TRUE)" => Err(Error::Value("Can't compare 3 and TRUE".into())),
    op_prec_gt_neq: "5 > 3 != TRUE" => Ok(Boolean(false)),
    op_prec_gt_neq_paren: "5 > (3 != TRUE)" => Err(Error::Value("Can't compare 3 and TRUE".into())),
    op_prec_gt_like: "5 > 3 LIKE 'abc'" => Err(Error::Value("Can't LIKE TRUE and abc".into())),
    op_prec_gt_like_paren: "5 > (3 LIKE 'abc')" => Err(Error::Value("Can't LIKE 3 and abc".into())),

    op_prec_gte_gt: "5 >= 3 > TRUE" => Ok(Boolean(false)),
    op_prec_gte_lt: "5 >= 3 < TRUE" => Ok(Boolean(false)),
    op_prec_gte_lte: "5 >= 3 <= TRUE" => Ok(Boolean(true)),
    op_prec_gte_eq: "5 >= 3 = TRUE" => Ok(Boolean(true)),
    op_prec_gte_eq_paren: "5 >= (3 = TRUE)" => Err(Error::Value("Can't compare 3 and TRUE".into())),
    op_prec_gte_neq: "5 >= 3 != TRUE" => Ok(Boolean(false)),
    op_prec_gte_neq_paren: "5 >= (3 != TRUE)" => Err(Error::Value("Can't compare 3 and TRUE".into())),
    op_prec_gte_like: "5 >= 3 LIKE 'abc'" => Err(Error::Value("Can't LIKE TRUE and abc".into())),
    op_prec_gte_like_paren: "5 >= (3 LIKE 'abc')" => Err(Error::Value("Can't LIKE 3 and abc".into())),

    op_prec_lt_gt: "3 < 5 > TRUE" => Ok(Boolean(false)),
    op_prec_lt_gte: "3 < 5 >= TRUE" => Ok(Boolean(true)),
    op_prec_lt_lte: "3 < 5 <= TRUE" => Ok(Boolean(true)),
    op_prec_lt_eq: "3 < 5 = TRUE" => Ok(Boolean(true)),
    op_prec_lt_eq_paren: "3 < (5 = TRUE)" => Err(Error::Value("Can't compare 5 and TRUE".into())),
    op_prec_lt_neq: "3 < 5 != TRUE" => Ok(Boolean(false)),
    op_prec_lt_neq_paren: "3 < (5 != TRUE)" => Err(Error::Value("Can't compare 5 and TRUE".into())),
    op_prec_lt_like: "3 < 5 LIKE 'abc'" => Err(Error::Value("Can't LIKE TRUE and abc".into())),
    op_prec_lt_like_paren: "3 < (5 LIKE 'abc')" => Err(Error::Value("Can't LIKE 5 and abc".into())),

    op_prec_lte_gt: "3 <= 5 > TRUE" => Ok(Boolean(false)),
    op_prec_lte_gte: "3 <= 5 >= TRUE" => Ok(Boolean(true)),
    op_prec_lte_lte: "3 <= 5 <= TRUE" => Ok(Boolean(true)),
    op_prec_lte_eq: "3 <= 5 = TRUE" => Ok(Boolean(true)),
    op_prec_lte_eq_paren: "3 <= (5 = TRUE)" => Err(Error::Value("Can't compare 5 and TRUE".into())),
    op_prec_lte_neq: "3 <= 5 != TRUE" => Ok(Boolean(false)),
    op_prec_lte_neq_paren: "3 <= (5 != TRUE)" => Err(Error::Value("Can't compare 5 and TRUE".into())),
    op_prec_lte_like: "3 <= 5 LIKE 'abc'" => Err(Error::Value("Can't LIKE TRUE and abc".into())),
    op_prec_lte_like_paren: "3 <= (5 LIKE 'abc')" => Err(Error::Value("Can't LIKE 5 and abc".into())),

    op_prec_eq_neq: "1 = 1 != FALSE" => Ok(Boolean(true)),
    op_prec_eq_like: "1 = 1 LIKE 'abc'" => Err(Error::Value("Can't LIKE TRUE and abc".into())),
    op_prec_eq_and: "1 = 1 AND TRUE" => Ok(Boolean(true)),
    op_prec_eq_and_paren: "1 = (1 AND TRUE)" => Err(Error::Value("Can't and 1 and TRUE".into())),

    op_prec_neq_eq: "1 != 2 = TRUE" => Ok(Boolean(true)),
    op_prec_neq_like: "1 != 2 LIKE 'abc'" => Err(Error::Value("Can't LIKE TRUE and abc".into())),
    op_prec_neq_and: "2 != 1 AND TRUE" => Ok(Boolean(true)),
    op_prec_neq_and_paren: "2 != (1 AND TRUE)" => Err(Error::Value("Can't and 1 and TRUE".into())),

    op_prec_like_eq: "'abc' LIKE 'abc' = TRUE" => Ok(Boolean(true)),
    op_prec_like_neq: "'abc' LIKE 'abc' != FALSE" => Ok(Boolean(true)),
    op_prec_like_and: "'abc' LIKE 'abc' AND TRUE" => Ok(Boolean(true)),
    op_prec_like_and_paren: "'abc' LIKE ('abc' AND TRUE)" => Err(Error::Value("Can't and abc and TRUE".into())),

    op_prec_and_or: "FALSE AND TRUE OR TRUE" => Ok(Boolean(true)),
    op_prec_and_or_paren: "FALSE AND (TRUE OR TRUE)" => Ok(Boolean(false)),
}
