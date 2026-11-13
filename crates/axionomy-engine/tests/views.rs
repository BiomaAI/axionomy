//! A view drawn as data: labels and formats, what it carries from frame to
//! frame, and leaderboards the engine ranks.

use axionomy::{OntologyPath, Quantity, Rate, basket, path::Path, paths};
use axionomy_engine::{Engine, EngineError, ScriptCall, View};
use axionomy_store::{Point, Store};

fn path<K>(text: &str) -> Path<K> {
    text.parse().unwrap()
}

fn pay(to: &str, units: u64) -> paths::Exchange {
    paths::Exchange::new(path("pay"), Quantity::new(units))
        .bind(path("from"), path("bank"))
        .bind(path("to"), path(to))
}

/// A bank paying three players: five to alice, five to bob, two to carol.
fn paid() -> (Engine, Point) {
    let engine = Engine::new(Store::memory().unwrap()).unwrap();
    let economy = paths::EconomyBuilder::new()
        .account(path("bank"), basket([(path("cash"), 100)]).into())
        .account(path("player/alice"), basket([]).into())
        .account(path("player/bob"), basket([]).into())
        .account(path("player/carol"), basket([]).into())
        .rate(
            path("pay"),
            Rate::new()
                .consume(path("from"), basket([(path("cash"), 1)]))
                .produce(path("to"), basket([(path("cash"), 1)])),
        )
        .build()
        .unwrap();
    let start = engine.author(economy).unwrap();
    let end = engine
        .apply(
            start,
            &[
                pay("player/alice", 5),
                pay("player/bob", 5),
                pay("player/carol", 2),
            ],
        )
        .unwrap();
    (engine, end)
}

fn call(function: &str) -> Option<ScriptCall> {
    Some(ScriptCall::new(
        path("bank/view"),
        function.parse().unwrap(),
    ))
}

fn keep(engine: &Engine, source: &str, view: View) {
    engine.keep_script(&path("bank/view"), source).unwrap();
    engine
        .keep_view(&OntologyPath::of::<paths::Paths>(), &view)
        .unwrap();
}

const SCRIPT: &str = r#"
    fn label(item) { if item == "asset/cash" { "Cash" } else { () } }
    fn format(asset) { if asset == "cash" { #{ scale: 2, unit: "USD" } } else { () } }
    // How many payments the history has made so far.
    fn carry(carried, before, exchange, after) {
        if carried == () { 1 } else { carried + 1 }
    }
    fn leaderboards(state, frame) {
        let entries = [];
        for player in state.accounts("player/") {
            let held = state.balance(player, "cash");
            entries.push(#{
                participant: #{ key: player, label: player.sub_string(7) },
                score: held.to_int(),
                value: held.to_string(),
                eligible: player != "player/carol",
            });
        }
        [#{
            key: "richest",
            label: `Richest after ${if frame.carried == () { 0 } else { frame.carried }} payments`,
            direction: "maximize",
            entries: entries,
        }]
    }
"#;

#[test]
fn a_view_labels_formats_carries_and_ranks() {
    let (engine, end) = paid();
    keep(
        &engine,
        SCRIPT,
        View {
            title: "Bank".into(),
            labels: call("label"),
            formats: call("format"),
            carry: call("carry"),
            scene: None,
            leaderboards: call("leaderboards"),
        },
    );
    let document = engine.draw(end).unwrap();
    assert_eq!(document.title, "Bank");
    assert_eq!(document.frames.len(), 3);
    let cash = &document.initial.accounts[0].balances[0].asset;
    assert_eq!(
        (cash.key.as_str(), cash.label.as_str()),
        ("paths/asset/cash", "Cash")
    );
    assert_eq!(
        document.formats[&path("paths/asset/cash")].unit.as_deref(),
        Some("USD")
    );
    // Unlabeled identifiers read as their paths.
    assert_eq!(document.initial.accounts[0].account.label, "account/bank");

    let board = &document.frames[2].after.leaderboards[0];
    assert_eq!(board.label, "Richest after 3 payments");
    let ranked: Vec<_> = board
        .entries
        .iter()
        .map(|entry| {
            (
                entry.participant.label.as_str(),
                entry.rank,
                entry.value.as_str(),
            )
        })
        .collect();
    // Alice and Bob tie for first; Carol is shown, unranked.
    assert_eq!(
        ranked,
        [
            ("alice", Some(1), "5"),
            ("bob", Some(1), "5"),
            ("carol", None, "2")
        ]
    );
    assert_eq!(
        document.initial.leaderboards[0].label,
        "Richest after 0 payments"
    );
}

#[test]
fn a_failing_view_fails_the_drawing() {
    let (engine, end) = paid();
    keep(
        &engine,
        r#"fn leaderboards(state, frame) { [#{ key: "broken" }] }"#,
        View {
            title: "Broken".into(),
            labels: None,
            formats: None,
            carry: None,
            scene: None,
            leaderboards: call("leaderboards"),
        },
    );
    assert!(matches!(engine.draw(end), Err(EngineError::Script { .. })));
    // A view whose script is not kept is refused.
    assert!(
        engine
            .keep_view(
                &OntologyPath::of::<paths::Paths>(),
                &View {
                    title: "Missing".into(),
                    labels: Some(ScriptCall::new(path("absent"), "label".parse().unwrap())),
                    formats: None,
                    carry: None,
                    scene: None,
                    leaderboards: None,
                },
            )
            .is_err()
    );
}

#[test]
fn exchanges_are_presented_assessed_in_the_views_words() {
    let (engine, end) = paid();
    keep(
        &engine,
        r#"
        fn label(item) {
            if item == "rate/pay" { "Payment" }
            else if item == "account/player/alice" { "Alice" }
            else { () }
        }
        "#,
        View {
            title: "Bank".into(),
            labels: call("label"),
            formats: None,
            carry: None,
            scene: None,
            leaderboards: None,
        },
    );
    // The bank holds 88: a payment of 80 applies, one of 90 lacks 2.
    let presented = engine
        .present(end, &[pay("player/alice", 80), pay("player/alice", 90)])
        .unwrap();
    assert_eq!(presented.len(), 2);
    assert_eq!(presented[0].id.as_str(), "exchange/0");
    assert_eq!(presented[0].label, "Payment");
    assert_eq!(presented[0].snapshot_index, 3);
    let alice = presented[0]
        .exchange
        .bindings
        .iter()
        .find(|binding| binding.role.key.as_str().ends_with("role/to"))
        .unwrap();
    assert_eq!(alice.account.label, "Alice");
    use axionomy_view::AssessmentStatusView;
    assert_eq!(
        presented[0].assessment.status,
        AssessmentStatusView::Applicable
    );
    assert_eq!(
        presented[1].assessment.status,
        AssessmentStatusView::Infeasible
    );
    assert_eq!(presented[1].assessment.shortfalls.len(), 1);
    // Presenting records nothing: the point is where it was.
    assert_eq!(engine.reach(end).unwrap().receipts.len(), 3);
}
