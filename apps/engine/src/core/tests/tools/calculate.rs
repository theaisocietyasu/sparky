//! calculate: arithmetic and date math.

use serde_json::json;

use crate::core::tests::support::ctx;
use crate::core::traits::tools::Tool;
use crate::runtime::tools::calculate::{CalculateTool, evaluate, format_number};

fn tool() -> CalculateTool {
    let Some(offset) = chrono::FixedOffset::east_opt(-7 * 3600) else {
        unreachable!("a valid offset")
    };
    CalculateTool::new(offset)
}

#[test]
fn arithmetic_follows_the_usual_order() {
    assert_eq!(evaluate("2 + 3 * 4"), Ok(14.0));
    assert_eq!(evaluate("(2 + 3) * 4"), Ok(20.0));
    assert_eq!(evaluate("-2 ^ 2"), Ok(-4.0));
    assert_eq!(evaluate("2 ^ 3 ^ 2"), Ok(512.0));
    assert_eq!(evaluate("10 % 4"), Ok(2.0));
    assert_eq!(evaluate("round(780 * 1.08, 2)"), Ok(842.4));
    assert_eq!(evaluate("max(3, 9, 4) + min(1, -1)"), Ok(8.0));
    assert_eq!(evaluate("sqrt(16) + abs(-2)"), Ok(6.0));
    assert_eq!(evaluate("1_000 * 3"), Ok(3000.0));
}

#[test]
fn a_bad_expression_says_why() {
    for bad in [
        "1 / 0", "2 +", "(1 + 2", "foo(2)", "3 $ 4", "1 2", "sqrt(-1)",
    ] {
        assert!(evaluate(bad).is_err(), "{bad} should fail");
    }
    let deep = format!("{}1{}", "(".repeat(200), ")".repeat(200));
    assert!(
        evaluate(&deep).is_err(),
        "deep nesting is refused, not a stack overflow"
    );
    assert!(
        evaluate(&"1+".repeat(400)).is_err(),
        "long input is refused"
    );
}

#[test]
fn numbers_read_as_a_person_writes_them() {
    assert_eq!(format_number(14.0), "14");
    assert_eq!(format_number(2.5), "2.5");
    assert_eq!(format_number(1.0 / 3.0), "0.3333333333");
}

#[tokio::test]
async fn the_days_between_two_dates_are_counted() {
    let out = tool()
        .call(&ctx(), json!({"from": "2026-11-05", "to": "2026-12-25"}))
        .await;
    let Ok(out) = out else {
        unreachable!("{out:?}")
    };
    assert!(out.content.contains("50 days"), "{}", out.content);
    assert!(out.content.contains("Friday"), "{}", out.content);
}

#[tokio::test]
async fn a_date_moves_by_days_and_names_its_weekday() {
    let out = tool()
        .call(&ctx(), json!({"from": "2026-11-05", "add_days": 30}))
        .await;
    let Ok(out) = out else {
        unreachable!("{out:?}")
    };
    assert!(
        out.content.contains("2026-12-05 (Saturday)"),
        "{}",
        out.content
    );
    let out = tool().call(&ctx(), json!({"from": "2026-11-05"})).await;
    let Ok(out) = out else {
        unreachable!("{out:?}")
    };
    assert_eq!(out.content, "2026-11-05 is a Thursday.");
}

#[tokio::test]
async fn a_bad_date_is_an_invalid_argument() {
    let out = tool().call(&ctx(), json!({"from": "next tuesday"})).await;
    assert!(out.is_err());
    let out = tool().call(&ctx(), json!({})).await;
    assert!(out.is_err(), "nothing to work out");
}
