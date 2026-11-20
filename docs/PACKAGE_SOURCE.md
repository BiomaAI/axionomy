# Authoring an Axionomy package

A package starts as a directory of Rhai programs. The compiler resolves names, builds each economy with the Rust kernel, checks the scripts and definitions, and runs the package's economic assertions in a fresh store. A successful archive carries both the executable package and the exact files used to build it. The CLI, MCP server and shipped gallery use the same compiler.

## Build and run the pantry example

The stockroom starts with three meals. Delivering two leaves one in the stockroom and two at the counter. A second delivery of two is one meal short. Reversing the supplier and recipient violates the law's fixed account bindings. The example tests all three cases. Its experiment asks for the shortest sequence of single deliveries that leaves two meals at the counter. [Pantry source](../examples/packages/pantry)

Run these commands from the repository root with the built CLI on your path. Choose a fresh file name for the output.

```sh
cargo build -p axionomy-cli --bin axionomy
export PATH="$PWD/target/debug:$PATH"
axionomy package validate examples/packages/pantry
axionomy package test examples/packages/pantry
axionomy package run examples/packages/pantry pantry/morning
axionomy package build examples/packages/pantry pantry.zip
axionomy package inspect pantry.zip
axionomy package query pantry.zip pantry/morning --source 'state.balance(account("stockroom"), asset("meal"))'
axionomy package assess pantry.zip pantry/morning --source 'exchange("deliver", #{supplier: "stockroom", recipient: "counter"}, 2)'
axionomy package compare examples/packages/pantry pantry.zip pantry/morning --seed 17
```

The experiment reaches its goal in two exchanges and measures two meals served. To see a difference, copy the directory and change the goal's required counter balance from two to three in the copy. Compare the two directories. Their third step differs by one delivery, and the report names the stockroom and counter balances at that step.

## Source files

| Relative name | Contents |
| --- | --- |
| `package.rhai` | `format` set to `axionomy.package/1`, package name and version, optional named branches and script interfaces |
| `ontologies/<name>.rhai` | Account and asset names, roles, rate keys or rate patterns, and exchange parameters. An ontology may also declare bounded families and an asset schema. |
| `laws/<name>.rhai` | Ordered rate declarations using the kernel's rate representation. Each entry is a pair of the rate key and its law, or a finite family to expand. |
| `instances/<ref>.rhai` | An opening economy's ontology, ordered law sets, accounts and balances, optional constants, rate tables and linear invariants |
| `definitions.rhai` | Goals with their ontologies, named candidate sets, strategies, objectives and experiments, and views keyed by ontology |
| `lib/<name>.rhai` | Reusable helpers imported by build programs and runtime scripts |
| `scripts/<name>.rhai` | Named modules for proposals, observations, objectives and rendering. A module can import another shipped module. |
| `tests/<name>.rhai` | A named starting ref and an ordered list of exchanges with independent expected results |

Each authoring program returns its final expression. It can declare functions, iterate over board coordinates, and import a package helper from `lib/<path>.rhai`. Runtime scripts can import the same helper. The compiler accepts Rhai source for every entry including the manifest, ontology, laws and assertions.

The Rust host exposes distinct `account("counter")`, `asset("meal")`, `role("supplier")` and `rate("deliver")` values. The ontology declares its vocabulary with `ontology()` and `declare`. A declared asset can have a zero opening balance. `amount(asset, quantity)` and `basket(amounts)` build checked native values. Use `quantity("…")` for an exact whole-atom amount outside Rhai’s signed integer range and `unsigned("…")` for a full-width trial seed or budget.

```rhai
import "connect-four/board" as board;
let rule = invariant("line-counts");
for color in ["red", "yellow"] {
    rule.weight(asset(`piece/${color}`), -16);
    for counter in board::line_counters(color, 9, 8) { rule.weight(counter, 1); }
}
rule.finish()
```

An ontology can also return a Rhai object with `asset_families`, `account_families` and `asset_schema`. For example, `#{pattern: "meal/{batch}", variables: [#{name: "batch", range: ["1", "12"]}]}` declares an inclusive range. The compiler validates declared and referenced denominations including assets reached through rate patterns.

A rate names the roles that consume, produce or preserve assets. Bind a role to a fixed account when identity is part of the law. Supply outstanding variables or relations in the instance's `tables` under the rate's exact key. This lets several opening economies share the same laws while differing in their map, calendar or population.

A rate key such as `move/{from}/{to}` normally remains a kernel template. Use an entry shaped as `#{expand: "select/ab/{before}", rate: …}` to materialize individual rates in domain order when the published economy exposes each concrete rate. Exact Cover uses this for progress transitions. A branch in `package.rhai` names a starting ref and its ordered `exchanges`. The compiler applies each exchange to establish that branch's history. [Exact Cover source](../crates/axionomy-problems/packages/exact-cover)

At runtime, Rhai scripts propose exchanges, measure outcomes and draw views. The kernel assesses and applies exchanges. The compiler resolves script imports and named callbacks, checks callback arity and checks literal host lookups against their declared vocabulary. Computed names depend on execution. Exercise those paths with economic tests and experiments. For a package with several ontologies, `script_ontologies` maps each script path to the ontologies it supports. A `Listed` candidate set in a multi-ontology package declares its ontology in `candidate_ontologies` in `definitions.rhai`.

## Economic assertions

A test step contains an `exchange` and the expected status, `Applicable`, `Infeasible` or `Invalid`. Optional `shortfalls` assert exact missing baskets per account. A `receipt` asserts the exchange and all deltas, including preserved assets and accounts opened or closed. Use `balances`, `exists` and `supply` to assert holdings, account lifecycle and total quantities after the attempt. Each test runs independently. A refusal must preserve the state and replaying all accepted exchanges must reproduce the final state. [Workshop receipt](../crates/axionomy-problems/packages/workshop/tests/a-chair-consumes-two-wood.rhai) · [Marketplace settlement](../crates/axionomy-problems/packages/marketplace/tests/two-orders-settle-for-190.rhai)

## Edit through MCP or Studio

Call `axionomy_package_source` with an installed package's `name` to retrieve the editable file tree. Call `axionomy_package_build` with a `files` object mapping relative file names to their complete UTF-8 contents. With `base` set to an installed package name, those entries edit its source. Set an entry to JSON null to remove the file. Omitting `base` builds the submitted tree from scratch.

A successful build returns its version, refs, definitions and passed tests. Request `export: true` to receive a base64 ZIP. Set `install: true` to publish the successful build to the connected store. Change the manifest version when installing changed content. A failed build returns a diagnostic identifying the file and the field or script location when available. Validation and tests run before installation.

In Studio, open **Source** to edit a Rhai file or add a module. Source opens with multiline indentation, including nested exchange and binding maps. Both **Source** and the **Rhai console** highlight syntax as you type. Choose **Format** to apply indentation and line breaks to an edit. **Build package** compiles the draft and runs its economic tests. **Download package** saves the resulting ZIP, which you can open in any client. In **From here**, the **Rhai console** runs queries at the selected point, assesses a proposed exchange and lets you apply the assessed exchange. Select an experiment and use **Compare build** to supply a second compiled package. Comparison uses one trial seed per matching pair, requires matching opening holdings and history, and reports the first differing exchange, affected balances and trial outcomes. The same workflow runs in native Studio and in the browser worker.
