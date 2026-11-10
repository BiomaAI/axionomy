use super::*;
use axionomy_problems::perishables::cold_chain::{
    self as chain, AccountId, Asset, BATCHES, Place, Policy, RateId, World,
};
use axionomy_view::{GraphEdgeView, GraphNodeView};

pub(super) fn build(
    study: &StudyKey,
    descriptor: &ProblemDescriptor,
    profile: InstanceProfile,
    progress: &mut ProgressSink<'_>,
) -> Result<Study, ServiceError> {
    let initial = chain::initial(matches!(profile, InstanceProfile::Stress), study.seed);
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
            progress,
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
        view.proposals.push(proposal(ProposalSpec { id: "overfilled-cold-store", label: "Move the entire ambient cohort into full cold storage", description: "The refrigerated space is occupied. Goods cannot move until enough physical capacity is released." }, &initial, &chain::action(RateId::Move { batch: 0, from: Place::Warehouse, to: Place::Fridge }, 1)));
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
    assemble(study, descriptor, documents)
}
fn objectives(world: &World) -> Vec<ObjectiveView> {
    let usable = chain::total(world, Asset::Fresh)
        + chain::total(world, Asset::Preserved)
        + chain::total(world, Asset::Delivered);
    [
        (
            path!("saved"),
            "Food saved or delivered",
            usable,
            ObjectiveDirectionView::Maximize,
        ),
        (
            path!("delivered"),
            "Food delivered",
            chain::total(world, Asset::Delivered),
            ObjectiveDirectionView::Maximize,
        ),
        (
            path!("revenue"),
            "Sales revenue",
            chain::value(world, AccountId::World, Asset::Money),
            ObjectiveDirectionView::Maximize,
        ),
        (
            path!("energy"),
            "Energy used",
            chain::total(world, Asset::SpentEnergy),
            ObjectiveDirectionView::Minimize,
        ),
        (
            path!("fuel"),
            "Fuel used",
            chain::total(world, Asset::GeneratorFuel) + chain::total(world, Asset::TransportFuel),
            ObjectiveDirectionView::Minimize,
        ),
        (
            path!("missed"),
            "Missed food obligations",
            chain::total(world, Asset::MissedDemand),
            ObjectiveDirectionView::Minimize,
        ),
    ]
    .into_iter()
    .map(|(key, label, value, direction)| ObjectiveView {
        key,
        label: label.into(),
        value: value.to_string(),
        direction,
    })
    .collect()
}
fn scene(_: u64, world: &World) -> Option<Scene> {
    let locations = [
        (path!("warehouse"), "Ambient warehouse", 120.0, 140.0),
        (path!("fridge"), "Finite cold storage", 460.0, 140.0),
        (path!("processing"), "Preserving line", 120.0, 340.0),
        (path!("buyer"), "Food deliveries", 770.0, 260.0),
        (path!("power"), "Grid and battery", 460.0, 340.0),
        (path!("fuel"), "Shared fuel reserve", 770.0, 70.0),
    ];
    let nodes = locations
        .into_iter()
        .map(|(key, label, x, y)| GraphNodeView {
            id: ViewId::new(key, label),
            classes: vec![path!("resource")],
            x: Some(x),
            y: Some(y),
        })
        .collect();
    let routes = [
        (path!("warehouse"), path!("fridge"), "space + labor"),
        (path!("warehouse"), path!("processing"), "energy + labor"),
        (path!("warehouse"), path!("buyer"), "up to 2 crates/day"),
        (path!("fridge"), path!("buyer"), "delivery frees space"),
        (path!("power"), path!("fridge"), "2 energy/day"),
        (path!("power"), path!("processing"), "1 energy/crate"),
        (path!("fuel"), path!("power"), "1 fuel → 3 energy"),
        (path!("fuel"), path!("buyer"), "1 fuel/trip"),
    ];
    let edges = routes
        .into_iter()
        .map(|(from, to, label)| GraphEdgeView {
            id: path!("cold").join(&from).join(&to),
            source: from,
            target: to,
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
        let mut entity = link_account::<chain::Ontology>(
            visual_entity(
                path!("cohort").then(&id),
                format!("Cohort {}", id + 1),
                SceneGlyphView::Package,
                SceneAnchorView::GraphNode {
                    node: if place == Place::Fridge {
                        path!("fridge")
                    } else {
                        path!("warehouse")
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
            &AccountId::Batch(id),
        );
        entity.metrics = vec![
            visual_metric(
                path!("heat"),
                "Heat exposure",
                value(Asset::Heat),
                Some("/ 4"),
            ),
            visual_metric(path!("origin"), "Thermal lineage", origin, None),
        ];
        entities.push(entity);
    }
    for (node, entity, metric, label, asset, value) in [
        (
            path!("fridge"),
            path!("stock/space"),
            path!("space"),
            "Free cold capacity",
            Asset::ColdSpace,
            chain::value(world, AccountId::World, Asset::ColdSpace),
        ),
        (
            path!("power"),
            path!("stock/energy"),
            path!("energy"),
            "Stored energy",
            Asset::Energy,
            chain::total(world, Asset::Energy),
        ),
        (
            path!("fuel"),
            path!("stock/fuel"),
            path!("fuel"),
            "Fuel available",
            Asset::Fuel,
            chain::total(world, Asset::Fuel),
        ),
        (
            path!("buyer"),
            path!("stock/delivered"),
            path!("delivered"),
            "Delivered",
            Asset::Delivered,
            chain::total(world, Asset::Delivered),
        ),
        (
            path!("processing"),
            path!("stock/labor"),
            path!("labor"),
            "Labor available",
            Asset::Labor,
            chain::total(world, Asset::Labor),
        ),
    ] {
        let buyer = node == path!("buyer");
        let mut entity = visual_entity(
            entity,
            label,
            SceneGlyphView::Material,
            SceneAnchorView::GraphNode { node },
            SceneEntityRoleView::State,
            SceneToneView::Neutral,
            None,
        );
        entity.metrics = vec![asset_metric(metric, label, &asset, value)];
        entities.push(link_account::<chain::Ontology>(
            entity,
            if buyer {
                &AccountId::Buyer
            } else {
                &AccountId::World
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
            visual_metric(path!("day"), "Day", day + 1, None),
            visual_metric(
                path!("grid"),
                "Grid supply",
                if chain::value(world, AccountId::World, Asset::Outage(day as u8)) > 0 {
                    "outage"
                } else {
                    "available"
                },
                None,
            ),
            visual_metric(
                path!("cooling"),
                "Refrigeration",
                if chain::total(world, Asset::CoolingOn) > 0 {
                    "powered"
                } else {
                    "unpowered"
                },
                None,
            ),
            asset_metric(
                path!("fresh"),
                "Fresh stock",
                &Asset::Fresh,
                chain::total(world, Asset::Fresh),
            ),
            asset_metric(
                path!("preserved"),
                "Preserved stock",
                &Asset::Preserved,
                chain::total(world, Asset::Preserved),
            ),
            visual_metric(
                path!("spoiled"),
                "Spoiled or discarded",
                chain::total(world, Asset::Spoiled) + chain::total(world, Asset::Disposed),
                Some("crates"),
            ),
            asset_metric(
                path!("delivered"),
                "Food delivered",
                &Asset::Delivered,
                chain::total(world, Asset::Delivered),
            ),
            asset_metric(
                path!("missed"),
                "Unmet obligations",
                &Asset::MissedDemand,
                chain::total(world, Asset::MissedDemand),
            ),
            asset_metric(
                path!("energy"),
                "Electricity left",
                &Asset::Energy,
                chain::total(world, Asset::Energy),
            ),
            asset_metric(
                path!("fuel"),
                "Fuel left",
                &Asset::Fuel,
                chain::total(world, Asset::Fuel),
            ),
        ]),
    )
}

// How Studio names and writes this problem's values.

fn place(place: Place) -> &'static str {
    match place {
        Place::Warehouse => "the warehouse",
        Place::Fridge => "the fridge",
    }
}

impl StudioFormat for chain::Asset {
    fn studio_format(&self) -> QuantityFormat {
        match self {
            Self::Fresh
            | Self::Preserved
            | Self::Spoiled
            | Self::Disposed
            | Self::Delivered
            | Self::ColdSpace
            | Self::ColdOccupancy
            | Self::Demand
            | Self::FutureDemand
            | Self::SatisfiedDemand
            | Self::MissedDemand => QuantityFormat::COUNT.unit("crates"),
            Self::Energy
            | Self::SpentEnergy
            | Self::GridPotential
            | Self::SpilledEnergy
            | Self::Fuel
            | Self::GeneratorFuel
            | Self::TransportFuel
            | Self::Labor
            | Self::SpentLabor
            | Self::FutureLabor
            | Self::UnusedLabor => QuantityFormat::COUNT.unit("units"),
            Self::Money => QuantityFormat::COUNT.unit("credits"),
            Self::Day | Self::Days => QuantityFormat::COUNT.unit("days"),
            _ => QuantityFormat::COUNT,
        }
    }
}

impl StudioLabel for chain::Asset {
    fn studio_label(&self) -> String {
        match self {
            Self::WorldIdentity => "Cold chain identity".into(),
            Self::BatchIdentity(id) => format!("Cohort {} identity", id + 1),
            Self::BuyerIdentity => "Buyer identity".into(),
            Self::Active => "Cohort in use".into(),
            Self::Dormant => "Cohort not yet in use".into(),
            Self::At(at) => format!("In {}", place(*at)),
            Self::Fresh => "Fresh food".into(),
            Self::Preserved => "Preserved food".into(),
            Self::Spoiled => "Spoiled food".into(),
            Self::Disposed => "Discarded food".into(),
            Self::Delivered => "Food delivered".into(),
            Self::Heat => "Heat exposure".into(),
            Self::Origin(origin) => match origin {
                0 => "Ambient origin".into(),
                1 => "Cold origin".into(),
                _ => format!("Origin {}", origin + 1),
            },
            Self::ColdSpace => "Free cold storage".into(),
            Self::ColdOccupancy => "Cold storage used".into(),
            Self::Energy => "Electricity".into(),
            Self::SpentEnergy => "Electricity used".into(),
            Self::GridPotential => "Grid supply still to come".into(),
            Self::SpilledEnergy => "Grid supply spilled".into(),
            Self::Fuel => "Fuel".into(),
            Self::GeneratorFuel => "Fuel burned for electricity".into(),
            Self::TransportFuel => "Fuel used for deliveries".into(),
            Self::CoolingOn => "Refrigeration on today".into(),
            Self::CoolingOff => "Refrigeration off".into(),
            Self::Labor => "Labor left today".into(),
            Self::SpentLabor => "Labor used".into(),
            Self::FutureLabor => "Labor in later days".into(),
            Self::UnusedLabor => "Labor left unused".into(),
            Self::Freight => "Shipments left today".into(),
            Self::UsedFreight => "Shipments made".into(),
            Self::FutureFreight => "Shipments in later days".into(),
            Self::UnusedFreight => "Shipments left unused".into(),
            Self::Money => "Money".into(),
            Self::Demand => "Food owed today".into(),
            Self::FutureDemand => "Food owed in later days".into(),
            Self::SatisfiedDemand => "Food obligations met".into(),
            Self::MissedDemand => "Food obligations missed".into(),
            Self::Day => "Time elapsed".into(),
            Self::Days => "Run length".into(),
            Self::Outage(day) => format!("Grid outage on day {}", day + 1),
            Self::Decisions => "Taking today's decisions".into(),
            Self::Aging => "Settling today's heat exposure".into(),
            Self::PendingAge(id) => format!("Cohort {} still to age today", id + 1),
            Self::Solved => "Horizon closed".into(),
        }
    }
}

impl StudioLabel for chain::Role {
    fn studio_label(&self) -> String {
        match self {
            Self::World => "Cold chain",
            Self::Batch => "Cohort",
            Self::Source => "Source cohort",
            Self::Destination => "Destination cohort",
            Self::Buyer => "Buyer",
        }
        .into()
    }
}

impl StudioLabel for chain::AccountId {
    fn studio_label(&self) -> String {
        match self {
            Self::World => "Cold-chain resources".into(),
            Self::Buyer => "Funded food deliveries".into(),
            Self::Batch(id) => format!("Cohort {}", id + 1),
        }
    }
}

impl StudioLabel for chain::RateId {
    fn studio_label(&self) -> String {
        match self {
            Self::Split {
                source,
                destination,
                at,
            } => format!(
                "Split Cohort {} into Cohort {} in {}, keeping its heat history",
                source + 1,
                destination + 1,
                place(*at)
            ),
            Self::Move { batch, to, .. } => {
                format!("Move Cohort {} to {}", batch + 1, place(*to))
            }
            Self::Process(batch) => format!("Preserve fresh food in Cohort {}", batch + 1),
            Self::Deliver {
                batch,
                preserved,
                at,
            } => format!(
                "Deliver {} food from Cohort {} in {}",
                if *preserved { "preserved" } else { "fresh" },
                batch + 1,
                place(*at)
            ),
            Self::Discard { batch, at } => match at {
                Place::Fridge => format!(
                    "Discard spoiled food from Cohort {} and free fridge space",
                    batch + 1
                ),
                Place::Warehouse => format!(
                    "Discard spoiled food from Cohort {} in the warehouse",
                    batch + 1
                ),
            },
            Self::Cool => "Power refrigeration for today".into(),
            Self::Generate => "Burn shared fuel for backup electricity".into(),
            Self::EndDay => "Close today's decisions and settle thermal exposure".into(),
            Self::AgeEmpty(batch) => format!("Cohort {} has no fresh food to age", batch + 1),
            Self::AgeCold(batch) => format!(
                "Cohort {} stays cold without resetting its prior heat",
                batch + 1
            ),
            Self::AgeWarm { batch, at, spoil } => format!(
                "Cohort {} {} in {}",
                batch + 1,
                if *spoil {
                    "crosses the spoilage threshold"
                } else {
                    "accumulates heat"
                },
                match at {
                    Place::Warehouse => "the warehouse",
                    Place::Fridge => "the unpowered fridge",
                }
            ),
            Self::OpenDay(day) => format!(
                "Open day {} with its scheduled grid supply and new obligations",
                day + 1
            ),
            Self::Close => "Close the cold-chain horizon and record unmet demand".into(),
        }
    }
}
