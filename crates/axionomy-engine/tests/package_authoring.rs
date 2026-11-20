use axionomy::{Quantity, path};
use axionomy_engine::source::{self, Diagnostic, SourceFile, SourceFiles};

fn pantry() -> SourceFiles {
    [
        (
            "package.rhai",
            include_str!("../../../examples/packages/pantry/package.rhai"),
        ),
        (
            "definitions.rhai",
            include_str!("../../../examples/packages/pantry/definitions.rhai"),
        ),
        (
            "instances/pantry/morning.rhai",
            include_str!("../../../examples/packages/pantry/instances/pantry/morning.rhai"),
        ),
        (
            "laws/delivery.rhai",
            include_str!("../../../examples/packages/pantry/laws/delivery.rhai"),
        ),
        (
            "ontologies/pantry.rhai",
            include_str!("../../../examples/packages/pantry/ontologies/pantry.rhai"),
        ),
        (
            "scripts/pantry.rhai",
            include_str!("../../../examples/packages/pantry/scripts/pantry.rhai"),
        ),
        (
            "tests/delivery.rhai",
            include_str!("../../../examples/packages/pantry/tests/delivery.rhai"),
        ),
    ]
    .into_iter()
    .map(|(f, s)| (f.parse().unwrap(), s.to_owned()))
    .collect()
}
fn edit(files: &mut SourceFiles, name: &str, program: &str) {
    let file = files.get_mut(&name.parse::<SourceFile>().unwrap()).unwrap();
    *file = format!("let doc = {{ {file} }};\n{program}\ndoc");
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
        "ontologies/pantry.rhai",
        r#"doc = doc.finish(); doc.asset_families = [#{pattern:"meal/{batch}",variables:[#{name:"batch",range:["1","3"]}]}];"#,
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
    edit(
        &mut files,
        "ontologies/pantry.rhai",
        r#"doc = doc.finish(); doc.asset_schema = [["meal",#{Decimal:#{decimals:2}}]];"#,
    );
    assert!(source::build(&files).is_ok());
    edit(
        &mut files,
        "ontologies/pantry.rhai",
        r#"doc.assets.push("decimal/3/meal");"#,
    );
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
    files.insert(f, "#{goals:#{}, goals:#{}}".into());
    let duplicate = rejected(&files);
    assert_eq!(duplicate.line, Some(1));
    assert!(duplicate.reason.to_lowercase().contains("duplicat"));
    let mut files = pantry();
    edit(&mut files, "laws/delivery.rhai", "doc[0][1].consuem = [];");
    let err = rejected(&files);
    assert_eq!(err.file.to_str(), Some("laws/delivery.rhai"));
    assert!(err.field.contains("consuem"), "{err}");
}

#[test]
fn a_listed_candidate_checks_every_binding_before_installation() {
    let mut files = pantry();
    edit(
        &mut files,
        "definitions.rhai",
        r#"doc.candidates["pantry/deliveries"] = #{Listed:[exchange(rate("deliver"), 1).bind(role("supplier"),account("storeroom")).bind(role("recipient"),account("counter"))]};"#,
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
    edit(
        &mut files,
        "definitions.rhai",
        r#"doc.goals["pantry/served"].goal = goal().require(account("counter"), basket([amount(asset("meal"), 3)]));"#,
    );
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
    edit(
        &mut files,
        "package.rhai",
        r#"doc.script_ontologies = #{pantry:["warehouse"]};"#,
    );
    let err = rejected(&files);
    assert!(
        err.reason
            .contains("experiment/pantry/morning uses script/pantry"),
        "{err}"
    );
}

#[test]
fn package_helpers_are_shared_by_build_and_run_and_only_rhai_is_source() {
    assert!("ontologies/pantry.json".parse::<SourceFile>().is_err());
    let mut files = pantry();
    files.insert(
        "lib/pantry/food.rhai".parse().unwrap(),
        r#"fn meal() { asset("meal") }"#.into(),
    );
    let ontology = SourceFile::Ontology(path!("pantry"));
    files.insert(
        ontology,
        r#"
        import "pantry/food" as food;
        let world = ontology();
        world.declare(food::meal());
        for id in ["counter", "stockroom"] { world.declare(account(id)); }
        for id in ["supplier", "recipient"] { world.declare(role(id)); }
        world.declare(rate("deliver"));
        world
    "#
        .into(),
    );
    files.insert(
        SourceFile::Script(path!("pantry")),
        r#"
        import "pantry/food" as food;
        fn served(s) { s.balance(account("counter"), food::meal()) }
    "#
        .into(),
    );
    let file: SourceFile = "definitions.rhai".parse().unwrap();
    let original = &files[&file];
    files.insert(
        file,
        original.replace("seed: 17", "seed: unsigned(\"18446744073709551615\")"),
    );
    let built = source::build(&files).unwrap();
    let experiment = built.engine.experiment(&path!("pantry/morning")).unwrap();
    assert_eq!(experiment.effort.seed, u64::MAX);
    built
        .engine
        .run_experiment(&path!("pantry/morning"), &mut |_, _| {
            std::ops::ControlFlow::Continue(())
        })
        .unwrap();
}
