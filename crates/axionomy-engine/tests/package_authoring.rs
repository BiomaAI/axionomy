use axionomy::{Quantity, path};
use axionomy_engine::source::{self, Diagnostic, SourceFile, SourceFiles};
use serde_json::{Value, json};

fn pantry() -> SourceFiles {
    [
        (
            "package.json",
            include_str!("../../../examples/packages/pantry/package.json"),
        ),
        (
            "definitions.json",
            include_str!("../../../examples/packages/pantry/definitions.json"),
        ),
        (
            "instances/pantry/morning.json",
            include_str!("../../../examples/packages/pantry/instances/pantry/morning.json"),
        ),
        (
            "laws/delivery.json",
            include_str!("../../../examples/packages/pantry/laws/delivery.json"),
        ),
        (
            "ontologies/pantry.json",
            include_str!("../../../examples/packages/pantry/ontologies/pantry.json"),
        ),
        (
            "scripts/pantry.rhai",
            include_str!("../../../examples/packages/pantry/scripts/pantry.rhai"),
        ),
        (
            "tests/delivery.json",
            include_str!("../../../examples/packages/pantry/tests/delivery.json"),
        ),
    ]
    .into_iter()
    .map(|(f, s)| (f.parse().unwrap(), s.to_owned()))
    .collect()
}
fn edit(files: &mut SourceFiles, name: &str, change: impl FnOnce(&mut Value)) {
    let file = files.get_mut(&name.parse::<SourceFile>().unwrap()).unwrap();
    let mut value = serde_json::from_str(file).unwrap();
    change(&mut value);
    *file = serde_json::to_string_pretty(&value).unwrap();
}
fn rejected(files: &SourceFiles) -> Diagnostic {
    match source::build(files) {
        Err(e) => e,
        Ok(_) => panic!("invalid source passed"),
    }
}
#[test]
fn a_typo_in_a_literal_balance_reports_the_script_line() {
    let mut files = pantry();
    let script = files.get_mut(&SourceFile::Script(path!("pantry"))).unwrap();
    *script = script.replace("\"meal\"", "\"meals\"");
    let err = rejected(&files);
    assert_eq!(err.file.to_str(), Some("scripts/pantry.rhai"));
    assert_eq!(err.line, Some(2));
    assert!(err.reason.contains("undeclared asset"), "{err}");
}

#[test]
fn bounded_families_admit_a_declared_zero_holding_and_check_their_limits() {
    let mut files = pantry();
    edit(
        &mut files,
        "ontologies/pantry.json",
        |v| {
            v["asset_families"] =
                json!([{"pattern":"meal/{batch}","variables":[{"name":"batch","range":["1","3"]}]}])
        },
    );
    let script = SourceFile::Script(path!("pantry"));
    files.insert(
        script.clone(),
        "fn served(s) { s.balance(\"counter\", \"meal/3\") }".into(),
    );
    assert!(source::build(&files).is_ok());
    files.insert(
        script,
        "fn served(s) { s.balance(\"counter\", \"meal/4\") }".into(),
    );
    assert!(rejected(&files).reason.contains("meal/4"));
}

#[test]
fn denominations_are_checked_even_for_assets_with_no_opening_balance() {
    let mut files = pantry();
    for text in files.values_mut() {
        *text = text.replace("\"meal\"", "\"decimal/2/meal\"");
    }
    edit(&mut files, "ontologies/pantry.json", |v| {
        v["asset_schema"] = json!([["meal",{"Decimal":{"decimals":2}}]])
    });
    assert!(source::build(&files).is_ok());
    edit(&mut files, "ontologies/pantry.json", |v| {
        v["assets"]
            .as_array_mut()
            .unwrap()
            .push(json!("decimal/3/meal"))
    });
    assert!(
        rejected(&files)
            .reason
            .contains("conflicting atomic definitions")
    );
}

#[test]
fn diagnoses_duplicate_definitions_and_unknown_nested_law_fields() {
    let mut files = pantry();
    let f = SourceFile::Definitions;
    files.insert(f, "{\"goals\":{},\"goals\":{}}".into());
    let duplicate = rejected(&files);
    assert_eq!(duplicate.line, Some(1));
    assert!(duplicate.reason.contains("duplicate source field"));
    let mut files = pantry();
    edit(&mut files, "laws/delivery.json", |v| {
        v[0][1]["consuem"] = json!([])
    });
    let err = rejected(&files);
    assert_eq!(err.file.to_str(), Some("laws/delivery.json"));
    assert!(err.field.contains("consuem"), "{err}");
}

#[test]
fn a_listed_candidate_checks_every_binding_before_installation() {
    let mut files = pantry();
    edit(
        &mut files,
        "definitions.json",
        |v| v["candidates"]["pantry/deliveries"] = json!({"Listed":[{"rate":"deliver","units":"1","bindings":{"supplier":"storeroom","recipient":"counter"}}]}),
    );
    assert!(
        rejected(&files)
            .reason
            .contains("undeclared account `storeroom`")
    );
}

#[test]
fn a_changed_target_reports_the_first_extra_exchange_and_its_balances() {
    let mut files = pantry();
    let a = source::build(&files).unwrap();
    edit(&mut files, "definitions.json", |v| {
        v["goals"]["pantry/served"]["goal"]["required"]["counter"][0][1] = json!("3")
    });
    let b = source::build(&files).unwrap();
    let report = source::compare(
        &a.engine,
        &b.engine,
        &path!("pantry/morning"),
        Some(u64::MAX),
    )
    .unwrap();
    assert!(!report.equal);
    let trial = &report.trials[0];
    assert_eq!(trial.left.steps, 2);
    assert_eq!(trial.right.steps, 3);
    let d = trial.first_difference.as_ref().unwrap();
    assert_eq!(d.step, 3);
    assert!(d.left.is_none());
    assert_eq!(d.right.as_ref().unwrap().rate(), &path!("deliver"));
    assert!(d.balances.contains(&(
        path!("counter"),
        path!("meal"),
        Quantity::new(2),
        Quantity::new(3)
    )));
}

#[test]
fn a_literal_binding_map_checks_both_roles_and_accounts() {
    for (bindings, reason) in [
        (
            "#{suppllier: \"stockroom\", recipient: \"counter\"}",
            "undeclared role",
        ),
        (
            "#{supplier: \"stockrom\", recipient: \"counter\"}",
            "undeclared account",
        ),
    ] {
        let mut files = pantry();
        files
            .get_mut(&SourceFile::Script(path!("pantry")))
            .unwrap()
            .push_str(&format!(
                "\nfn delivery() {{ exchange(\"deliver\", {bindings}, 1) }}"
            ));
        let err = rejected(&files);
        assert!(err.reason.contains(reason), "{err}");
    }
}

#[test]
fn missing_imports_and_syntax_errors_report_their_source_position() {
    for bad in ["\nimport \"pantry/missing\" as missing;", "\nfn served( {"] {
        let mut files = pantry();
        files.insert(SourceFile::Script(path!("pantry")), bad.into());
        let err = rejected(&files);
        assert_eq!(err.file.to_str(), Some("scripts/pantry.rhai"));
        assert_eq!(err.line, Some(2), "{err}");
    }
}
#[test]
fn an_experiment_cannot_use_an_objective_script_for_a_different_ontology() {
    let mut files = pantry();
    let other = files[&SourceFile::Ontology(path!("pantry"))].clone();
    files.insert(SourceFile::Ontology(path!("warehouse")), other);
    edit(&mut files, "package.json", |v| {
        v["script_ontologies"] = json!({"pantry":["warehouse"]})
    });
    let err = rejected(&files);
    assert!(
        err.reason
            .contains("experiment/pantry/morning uses script/pantry"),
        "{err}"
    );
}
