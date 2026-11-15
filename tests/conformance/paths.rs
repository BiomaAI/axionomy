//! Paths in their natural order, as the executable Lean specification
//! orders them: numbers by value and before words, a path before the paths
//! it starts, `-` within a segment and `/` between segments told apart.

use axionomy::path::AccountPath;
use serde::Deserialize;
use std::cmp::Ordering;

#[derive(Deserialize)]
struct Fixtures {
    #[serde(rename = "pathOrder")]
    path_order: Vec<PathOrder>,
}

#[derive(Deserialize)]
struct PathOrder {
    left: String,
    right: String,
    order: String,
}

#[test]
fn paths_order_as_lean_orders_them() {
    let fixtures: Fixtures = serde_json::from_str(crate::FIXTURES).unwrap();
    assert!(!fixtures.path_order.is_empty());
    for case in fixtures.path_order {
        let left: AccountPath = case.left.parse().unwrap();
        let right: AccountPath = case.right.parse().unwrap();
        let expected = match case.order.as_str() {
            "lt" => Ordering::Less,
            "eq" => Ordering::Equal,
            "gt" => Ordering::Greater,
            other => panic!("no order {other}"),
        };
        assert_eq!(
            left.cmp(&right),
            expected,
            "{} vs {}",
            case.left,
            case.right
        );
    }
}
