use super::*;
use axionomy_problems::perishables::cold_chain::{
    self as chain, AccountId, Asset, BATCHES, Place, Policy, RateId, World,
};
use axionomy_view::{GraphEdgeView, GraphNodeView};

pub(super) fn build(
    run: &ArtifactKey,
    descriptor: &ProblemDescriptor,
    profile: InstanceProfile,
) -> Result<RunArtifact, ServiceError> {
    let initial = chain::initial(matches!(profile, InstanceProfile::Stress), run.seed);
    let mut documents = Vec::new();
    for (strategy, title, description, policy) in [
        (
            "outage",
            "Perishables · protect the cold chain",
            "Power finite cold storage first, deliver urgent fresh stock, process the remainder and use backup fuel when electricity runs short. Transport draws from the same fuel reserve.",
            Policy::ProtectCold,
        ),
        (
            "pareto-inventory",
            "Perishables · process before cooling",
            "Preserve ambient stock before powering refrigeration. Processing consumes electricity and labor, then the policy revisits shipment and storage choices each day.",
            Policy::ProcessFirst,
        ),
        (
            "pareto-energy",
            "Perishables · conserve transport fuel",
            "Ship on alternating days and process what the energy budget permits. Fewer trips save fuel but can leave food obligations unmet.",
            Policy::ConserveFuel,
        ),
        (
            "storage-buffer",
            "Perishables · rebalance cold storage",
            "Ship refrigerated stock to free capacity, split a warm cohort to fit that space, and move it into cooling while preserving its prior heat exposure.",
            Policy::RebalanceCold,
        ),
    ] {
        let trace = chain::run(&initial, policy);
        let final_world = initial
            .replayed(&trace)
            .map_err(|error| problem_error("perishables", error))?;
        let mut view = document(
            DocumentSpec {
                problem: "perishables",
                strategy,
                title,
                description,
                source_label: "Cold-chain operations",
            },
            &initial,
            &chain::goal(),
            &trace,
            objectives(&final_world),
            scene,
        )
        .map_err(|error| problem_error("perishables", error))?;
        view.telemetry.push(telemetry(
            "daily resource-responsive policy",
            true,
            [(
                TelemetryKindView::Generated,
                trace.exchanges().len() as u64,
                "verified physical and temporal exchanges".into(),
            )],
        ));
        view.proposals.push(proposal("perishables", ProposalSpec { id: "overfilled-cold-store", label: "Move the entire ambient cohort into full cold storage", description: "The refrigerated space is occupied. Goods cannot move until enough physical capacity is released." }, &initial, &chain::action(RateId::Move { batch: 0, from: Place::Warehouse, to: Place::Fridge }, 1)));
        documents.push(view);
    }
    for index in 0..documents.len() {
        let front = evaluated_integer_front(
            &documents,
            &documents[index],
            "Non-dominated policies on the same outage schedule",
        );
        documents[index].pareto_fronts.push(front);
    }
    artifact(run, descriptor, documents)
}
fn objectives(world: &World) -> Vec<ObjectiveView> {
    let usable = chain::total(world, Asset::Fresh)
        + chain::total(world, Asset::Preserved)
        + chain::total(world, Asset::Delivered);
    [
        (
            "saved",
            "Food saved or delivered",
            usable,
            ObjectiveDirectionView::Maximize,
        ),
        (
            "delivered",
            "Food delivered",
            chain::total(world, Asset::Delivered),
            ObjectiveDirectionView::Maximize,
        ),
        (
            "revenue",
            "Sales revenue",
            chain::value(world, AccountId::World, Asset::Money),
            ObjectiveDirectionView::Maximize,
        ),
        (
            "energy",
            "Energy used",
            chain::total(world, Asset::SpentEnergy),
            ObjectiveDirectionView::Minimize,
        ),
        (
            "fuel",
            "Fuel used",
            chain::total(world, Asset::GeneratorFuel) + chain::total(world, Asset::TransportFuel),
            ObjectiveDirectionView::Minimize,
        ),
        (
            "missed",
            "Missed food obligations",
            chain::total(world, Asset::MissedDemand),
            ObjectiveDirectionView::Minimize,
        ),
    ]
    .into_iter()
    .map(|(key, label, value, direction)| ObjectiveView {
        key: slug(key),
        label: label.into(),
        value: value.to_string(),
        direction,
    })
    .collect()
}
fn scene(_: u64, world: &World) -> Option<Scene> {
    let locations = [
        ("warehouse", "Ambient warehouse", 120.0, 140.0),
        ("fridge", "Finite cold storage", 460.0, 140.0),
        ("processing", "Preserving line", 120.0, 340.0),
        ("buyer", "Food deliveries", 770.0, 260.0),
        ("power", "Grid and battery", 460.0, 340.0),
        ("fuel", "Shared fuel reserve", 770.0, 70.0),
    ];
    let nodes = locations
        .into_iter()
        .map(|(key, label, x, y)| GraphNodeView {
            id: ViewId::new(slug(key), label),
            classes: vec![slug("resource")],
            x: Some(x),
            y: Some(y),
        })
        .collect();
    let routes = [
        ("warehouse", "fridge", "space + labor"),
        ("warehouse", "processing", "energy + labor"),
        ("warehouse", "buyer", "up to 2 crates/day"),
        ("fridge", "buyer", "delivery frees space"),
        ("power", "fridge", "2 energy/day"),
        ("power", "processing", "1 energy/crate"),
        ("fuel", "power", "1 fuel → 3 energy"),
        ("fuel", "buyer", "1 fuel/trip"),
    ];
    let edges = routes
        .into_iter()
        .map(|(from, to, label)| GraphEdgeView {
            id: slug(format!("cold/{from}/{to}")),
            source: slug(from),
            target: slug(to),
            label: Some(label.into()),
            classes: vec![],
        })
        .collect();
    let mut entities = Vec::new();
    for id in BATCHES {
        let Some(place) = chain::place(world, id) else {
            continue;
        };
        let value = |asset| chain::value(world, AccountId::Batch(id), asset);
        let fresh = value(Asset::Fresh);
        let stable = value(Asset::Preserved);
        let spoiled = value(Asset::Spoiled);
        let origin = if value(Asset::Origin(0)) > 0 {
            "ambient origin"
        } else {
            "cold origin"
        };
        let mut entity = link_account(
            visual_entity(
                slug(format!("cohort/{id}")),
                format!("Cohort {}", id + 1),
                SceneGlyphView::Package,
                SceneAnchorView::GraphNode {
                    node: if place == Place::Fridge {
                        slug("fridge")
                    } else {
                        slug("warehouse")
                    },
                },
                SceneEntityRoleView::Attachment,
                if spoiled > 0 {
                    SceneToneView::Danger
                } else if fresh + stable == 0 {
                    SceneToneView::Muted
                } else {
                    SceneToneView::Active
                },
                Some(format!(
                    "{fresh} fresh · {stable} preserved · {spoiled} spoiled"
                )),
            ),
            slug(format!("perishables/account/batch-{id}")),
        );
        entity.metrics = vec![
            visual_metric(
                slug("heat"),
                "Heat exposure",
                value(Asset::Heat),
                Some("/ 4"),
            ),
            visual_metric(slug("origin"), "Thermal lineage", origin, None),
        ];
        entities.push(entity);
    }
    for (node, key, label, value, unit) in [
        (
            "fridge",
            "space",
            "Free cold capacity",
            chain::value(world, AccountId::World, Asset::ColdSpace),
            "crates",
        ),
        (
            "power",
            "energy",
            "Stored energy",
            chain::total(world, Asset::Energy),
            "units",
        ),
        (
            "fuel",
            "fuel",
            "Fuel available",
            chain::total(world, Asset::Fuel),
            "units",
        ),
        (
            "buyer",
            "delivered",
            "Delivered",
            chain::total(world, Asset::Delivered),
            "crates",
        ),
        (
            "processing",
            "labor",
            "Labor available",
            chain::total(world, Asset::Labor),
            "units",
        ),
    ] {
        let mut entity = visual_entity(
            slug(format!("stock/{key}")),
            label,
            SceneGlyphView::Material,
            SceneAnchorView::GraphNode { node: slug(node) },
            SceneEntityRoleView::State,
            SceneToneView::Neutral,
            None,
        );
        entity.metrics = vec![visual_metric(slug(key), label, value, Some(unit))];
        entities.push(link_account(
            entity,
            if node == "buyer" {
                slug("perishables/account/buyer")
            } else {
                slug("perishables/account/world")
            },
        ));
    }
    let day = chain::value(world, AccountId::World, Asset::Day);
    Some(
        Scene::graph(
            "Thermal history, physical capacity and funded deliveries",
            nodes,
            edges,
            None,
        )
        .with_entities(entities)
        .with_metrics([
            visual_metric(slug("day"), "Day", day + 1, None),
            visual_metric(
                slug("grid"),
                "Grid supply",
                if chain::value(world, AccountId::World, Asset::Outage(day as u8)) > 0 {
                    "outage"
                } else {
                    "available"
                },
                None,
            ),
            visual_metric(
                slug("cooling"),
                "Refrigeration",
                if chain::total(world, Asset::CoolingOn) > 0 {
                    "powered"
                } else {
                    "unpowered"
                },
                None,
            ),
            visual_metric(
                slug("fresh"),
                "Fresh stock",
                chain::total(world, Asset::Fresh),
                Some("crates"),
            ),
            visual_metric(
                slug("preserved"),
                "Preserved stock",
                chain::total(world, Asset::Preserved),
                Some("crates"),
            ),
            visual_metric(
                slug("spoiled"),
                "Spoiled or discarded",
                chain::total(world, Asset::Spoiled) + chain::total(world, Asset::Disposed),
                Some("crates"),
            ),
            visual_metric(
                slug("delivered"),
                "Food delivered",
                chain::total(world, Asset::Delivered),
                Some("crates"),
            ),
            visual_metric(
                slug("missed"),
                "Unmet obligations",
                chain::total(world, Asset::MissedDemand),
                Some("crates"),
            ),
            visual_metric(
                slug("energy"),
                "Electricity left",
                chain::total(world, Asset::Energy),
                Some("units"),
            ),
            visual_metric(
                slug("fuel"),
                "Fuel left",
                chain::total(world, Asset::Fuel),
                Some("units"),
            ),
        ]),
    )
}
