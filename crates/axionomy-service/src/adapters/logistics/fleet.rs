use super::*;
use axionomy_problems::logistics::fleet::{
    self as fleet, AccountId, Asset, LOCATIONS, Location, Policy, ROADS, RateId, VANS, Van, World,
};

pub(super) fn build(
    study: &StudyKey,
    descriptor: &ProblemDescriptor,
    profile: InstanceProfile,
    progress: &mut ProgressSink<'_>,
) -> Result<Study, ServiceError> {
    let initial = fleet::initial(matches!(profile, InstanceProfile::Stress), study.seed);
    let mut documents = Vec::new();
    for (strategy, title, description, policy) in [
        (
            "direct",
            "Logistics · fastest-route fleet",
            "Dispatch two different vans through the shortest available road paths. Express links save hops but consume extra fuel and can cause breakdowns; the fleet responds with shared towing and repair.",
            Policy::Fastest,
        ),
        (
            "reliable",
            "Logistics · reliable-route fleet",
            "Avoid the risky express link, respect the single occupied trunk road, and revisit delivery, refueling and maintenance choices as orders arrive. Heavy freight and cold cargo require different vehicle capacity.",
            Policy::Reliable,
        ),
        (
            "mcts",
            "Logistics · adaptive delivery",
            "Use responsive fleet dispatch and hub relays: pack spare cold-van capacity, stage compatible freight in the finite hub, and let the other van collect it. Every shipment, service and payment is a core exchange. This larger-profile policy is not MCTS; the Micro fixture retains receding-horizon MCTS.",
            Policy::Relay,
        ),
    ] {
        let _ = progress.emit(
            path!("fleet-dispatch"),
            documents.len() as u64,
            3,
            format!(
                "Computing fleet policy {} of 3: {title}",
                documents.len() + 1
            ),
        );
        progress.ensure()?;
        let (trace, final_world) =
            simulate("logistics", &initial, |world| fleet::run(world, policy))?;
        let mut view = document(
            progress,
            DocumentSpec {
                problem: "logistics",
                strategy,
                title,
                description,
                source_label: "Heterogeneous delivery fleet",
            },
            &initial,
            &fleet::goal(),
            &trace,
            objectives(&final_world),
            scene,
        )
        .map_err(|error| problem_error("logistics", error))?;
        view.telemetry.push(telemetry(
            "responsive fleet dispatch with public route planning",
            true,
            [(
                TelemetryKindView::Generated,
                trace.exchanges().len() as u64,
                "verified fleet decisions and realized route tickets".into(),
            )],
        ));
        view.proposals.push(proposal(ProposalSpec { id: "wrong-vehicle", label: "Put refrigerated cargo in the freight van", description: "The freight van has more payload space but no refrigerated slot. Cargo compatibility is enforced by the exchange, alongside arrival and physical location." }, &initial, &fleet::action(RateId::Load { van: Van::Freight, order: 0, at: Location::Depot })));
        documents.push(view);
    }
    let _ = progress.emit(
        path!("fleet-dispatch"),
        3,
        3,
        "Three fleet policies completed and replay verified",
    );
    progress.ensure()?;
    for index in 0..documents.len() {
        let front = evaluated_integer_front(
            &documents,
            &documents[index],
            "Fleet policies on the same orders and vehicle-route tickets",
        );
        documents[index].pareto_fronts.push(front);
    }
    assemble(study, descriptor, documents)
}
fn objectives(world: &World) -> Vec<ObjectiveView> {
    let val = |account, asset| fleet::value(world, account, asset);
    [
        (
            path!("delivered"),
            "Orders delivered",
            val(AccountId::World, Asset::DeliveredCount),
            ObjectiveDirectionView::Maximize,
        ),
        (
            path!("on-time"),
            "Orders on time",
            val(AccountId::World, Asset::OnTimeCount),
            ObjectiveDirectionView::Maximize,
        ),
        (
            path!("expired"),
            "Orders expired",
            val(AccountId::World, Asset::ExpiredCount),
            ObjectiveDirectionView::Minimize,
        ),
        (
            path!("cash"),
            "Fleet cash remaining",
            val(AccountId::World, Asset::Money),
            ObjectiveDirectionView::Maximize,
        ),
        (
            path!("fuel"),
            "Fuel used",
            VANS.into_iter()
                .map(|van| val(AccountId::Vehicle(van), Asset::SpentFuel))
                .sum(),
            ObjectiveDirectionView::Minimize,
        ),
        (
            path!("time"),
            "Elapsed fleet time",
            fleet::now(world),
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
    let val = |account, asset| fleet::value(world, account, asset);
    let nodes = [
        (Location::Depot, "Depot and service yard", 120.0, 300.0),
        (Location::Hub, "Shared relay hub", 430.0, 300.0),
        (Location::North, "North customer", 430.0, 50.0),
        (Location::South, "South customer", 430.0, 590.0),
        (Location::East, "East customer", 780.0, 300.0),
    ]
    .into_iter()
    .map(|(location, label, x, y)| GraphNodeView {
        id: ViewId::new(path::write(&location), label),
        classes: vec![path!("location")],
        x: Some(x),
        y: Some(y),
    })
    .collect();
    let edges = ROADS
        .into_iter()
        .map(|(from, to)| GraphEdgeView {
            id: path!("road").then(&from).then(&to),
            source: path::write(&from),
            target: path::write(&to),
            label: Some(
                if fleet::main_road(from, to) {
                    "one vehicle at a time"
                } else if fleet::express(from, to) {
                    "express · 2 fuel · breakdown risk"
                } else {
                    "2 ticks + possible delay"
                }
                .into(),
            ),
            classes: vec![],
        })
        .collect();
    let mut entities = Vec::new();
    for van in VANS {
        let own = AccountId::Vehicle(van);
        let at = fleet::location(world, van);
        let travel = ROADS
            .into_iter()
            .flat_map(|(a, b)| [(a, b), (b, a)])
            .find(|(from, to)| val(own, Asset::Travelling(*from, *to)) > 0);
        let anchor = if let Some(at) = at {
            SceneAnchorView::GraphNode {
                node: path::write(&at),
            }
        } else if let Some((from, to)) = travel {
            let (a, b) = ROADS
                .into_iter()
                .find(|(a, b)| (*a == from && *b == to) || (*a == to && *b == from))
                .unwrap();
            let start = val(own, Asset::DepartedAt);
            let end = val(own, Asset::ReadyAt);
            let progress = if end > start {
                (fleet::now(world).saturating_sub(start) as f64 / (end - start) as f64)
                    .clamp(0.0, 1.0)
            } else {
                0.0
            };
            SceneAnchorView::GraphEdge {
                edge: path!("road").then(&a).then(&b),
                progress: Some(if a == from { progress } else { 1.0 - progress }),
            }
        } else {
            SceneAnchorView::Unanchored
        };
        let status = if let Some((_, to)) = travel {
            if val(own, Asset::AwaitingOutcome) > 0 {
                format!("To {to:?} · awaiting route outcome")
            } else {
                format!("To {to:?} · arrives {}", val(own, Asset::ReadyAt))
            }
        } else if val(own, Asset::Refueling) > 0 {
            format!("Pump occupied until {}", val(own, Asset::ReadyAt))
        } else if val(own, Asset::Repairing) > 0 {
            format!("Repair bay occupied until {}", val(own, Asset::ReadyAt))
        } else if val(own, Asset::Towing) > 0 {
            format!("Being towed until {}", val(own, Asset::ReadyAt))
        } else if val(own, Asset::Damage) > 0 {
            "Broken down · transfer cargo or tow".into()
        } else {
            "Ready for dispatch".into()
        };
        let mut entity = link_account::<fleet::Ontology>(
            visual_entity(
                path!("van").then(&van),
                format!("{van:?} van"),
                SceneGlyphView::Vehicle,
                anchor,
                if at.is_some() || travel.is_some() {
                    SceneEntityRoleView::Occupant
                } else {
                    SceneEntityRoleView::Context
                },
                if val(own, Asset::Damage) > 0 {
                    SceneToneView::Danger
                } else {
                    SceneToneView::Active
                },
                Some(status),
            ),
            &own,
        );
        entity.metrics = vec![
            visual_metric(path!("fuel"), "Fuel", val(own, Asset::Fuel), Some("/ 8")),
            asset_metric(
                path!("space"),
                "Free payload space",
                &Asset::Space,
                val(own, Asset::Space),
            ),
            visual_metric(path!("wear"), "Wear", val(own, Asset::Wear), Some("/ 6")),
        ];
        entities.push(entity);
        for id in 0..val(AccountId::World, Asset::Count) as u8 {
            if val(own, Asset::Cargo(id)) > 0 {
                let mut cargo = link_account::<fleet::Ontology>(
                    visual_entity(
                        path!("cargo").then(&id),
                        format!("Order {} cargo", id + 1),
                        SceneGlyphView::Package,
                        SceneAnchorView::Entity {
                            entity: path!("van").then(&van),
                        },
                        SceneEntityRoleView::Attachment,
                        SceneToneView::Neutral,
                        Some(
                            if fleet::cold(id) {
                                "Refrigerated"
                            } else if fleet::weight(id) == 3 {
                                "Heavy freight"
                            } else {
                                "Standard freight"
                            }
                            .into(),
                        ),
                    ),
                    &AccountId::Order(id),
                );
                cargo.metrics = vec![asset_metric(
                    path!("weight"),
                    "Payload",
                    &Asset::Weight,
                    val(AccountId::Order(id), Asset::Weight),
                )];
                entities.push(cargo);
            }
        }
    }
    for id in 0..val(AccountId::World, Asset::Count) as u8 {
        let own = AccountId::Order(id);
        let delivered = val(own, Asset::Delivered) > 0;
        let expired = val(own, Asset::Expired) > 0;
        let staged = val(own, Asset::DockUsed) > 0;
        let status = if delivered {
            "Delivered and paid"
        } else if expired {
            "Expired undelivered"
        } else if staged {
            "Staged at hub for another van"
        } else if val(own, Asset::InTransit) > 0 {
            "Cargo assigned to vehicle"
        } else if val(own, Asset::Scheduled) > 0 {
            "Customer order arriving later"
        } else {
            "Cargo waiting at depot"
        };
        let waiting_at = if staged {
            Location::Hub
        } else {
            fleet::destination(id)
        };
        let mut entity = link_account::<fleet::Ontology>(
            visual_entity(
                path!("order").then(&id),
                format!(
                    "Order {} · {}",
                    id + 1,
                    if fleet::cold(id) {
                        "cold"
                    } else if fleet::weight(id) == 3 {
                        "heavy"
                    } else {
                        "standard"
                    }
                ),
                SceneGlyphView::Task,
                SceneAnchorView::GraphNode {
                    node: path::write(&waiting_at),
                },
                SceneEntityRoleView::Attachment,
                if delivered {
                    SceneToneView::Success
                } else if expired {
                    SceneToneView::Danger
                } else {
                    SceneToneView::Neutral
                },
                Some(status.into()),
            ),
            &own,
        );
        entity.metrics = vec![
            asset_metric(
                path!("deadline"),
                "Delivery window",
                &Asset::Deadline,
                val(own, Asset::Deadline),
            ),
            asset_metric(
                path!("arrival"),
                "Order release",
                &Asset::Arrival,
                val(own, Asset::Arrival),
            ),
        ];
        entities.push(entity);
    }
    let mut scene = Scene::graph(
        "Fleet cargo, shared roads and occupied service capacity",
        nodes,
        edges,
        None,
    )
    .with_entities(entities)
    .with_metrics([
        asset_metric(path!("time"), "Fleet time", &Asset::Time, fleet::now(world)),
        asset_metric(
            path!("delivered"),
            "Orders delivered",
            &Asset::DeliveredCount,
            val(AccountId::World, Asset::DeliveredCount),
        ),
        asset_metric(
            path!("on-time"),
            "Orders on time",
            &Asset::OnTimeCount,
            val(AccountId::World, Asset::OnTimeCount),
        ),
        asset_metric(
            path!("expired"),
            "Orders expired",
            &Asset::ExpiredCount,
            val(AccountId::World, Asset::ExpiredCount),
        ),
        asset_metric(
            path!("staged"),
            "Hub handoffs staged",
            &Asset::StagedCount,
            val(AccountId::World, Asset::StagedCount),
        ),
        asset_metric(
            path!("cash"),
            "Fleet cash",
            &Asset::Money,
            val(AccountId::World, Asset::Money),
        ),
    ]);
    for location in LOCATIONS {
        if let Some(entity) = scene
            .entities
            .iter_mut()
            .find(|entity| entity.id.key == path::write(&location))
        {
            if location == Location::Depot {
                entity.status = Some("Shared paid services".into());
                entity.metrics = vec![
                    visual_metric(
                        path!("pump"),
                        "Free fuel pump",
                        val(AccountId::Station, Asset::FuelPump),
                        Some("/ 1"),
                    ),
                    visual_metric(
                        path!("bay"),
                        "Free repair bay",
                        val(AccountId::Station, Asset::RepairBay),
                        Some("/ 1"),
                    ),
                    asset_metric(
                        path!("fuel"),
                        "Station fuel",
                        &Asset::Fuel,
                        val(AccountId::Station, Asset::Fuel),
                    ),
                ];
            }
            if location == Location::Hub {
                entity.status = Some("Finite cargo relay".into());
                entity.metrics = vec![
                    visual_metric(
                        path!("dock"),
                        "Free staging space",
                        val(AccountId::World, Asset::DockSpace),
                        Some("/ 2"),
                    ),
                    visual_metric(
                        path!("road"),
                        "Trunk road available",
                        val(AccountId::World, Asset::RoadSpace),
                        Some("/ 1"),
                    ),
                ];
            }
        }
    }
    Some(scene)
}

// How Studio names and writes this problem's values.

impl StudioFormat for fleet::Asset {
    fn studio_format(&self) -> QuantityFormat {
        match self {
            Self::Time
            | Self::TimeRemaining
            | Self::Horizon
            | Self::DepartedAt
            | Self::ReadyAt
            | Self::Arrival
            | Self::Deadline => QuantityFormat::COUNT.unit("ticks"),
            Self::Fuel | Self::TankSpace | Self::SpentFuel => QuantityFormat::COUNT.unit("units"),
            Self::Space | Self::Weight | Self::DockSpace | Self::DockUsed => {
                QuantityFormat::COUNT.unit("units")
            }
            Self::Money | Self::Revenue | Self::Price => QuantityFormat::COUNT.unit("credits"),
            _ => QuantityFormat::COUNT,
        }
    }
}

impl StudioLabel for fleet::Asset {
    fn studio_label(&self) -> String {
        match self {
            Self::WorldIdentity => "Fleet identity".into(),
            Self::StationIdentity => "Station identity".into(),
            Self::NatureIdentity => "Nature identity".into(),
            Self::VehicleIdentity(van) => format!("{van:?} van identity"),
            Self::OrderIdentity(id) => format!("Order {} identity", id + 1),
            Self::Active => "Fleet run in progress".into(),
            Self::Closed => "Fleet run closed".into(),
            Self::Time => "Fleet time".into(),
            Self::TimeRemaining => "Time remaining".into(),
            Self::Horizon => "Fleet horizon".into(),
            Self::Count => "Orders in the run".into(),
            Self::Resolved => "Orders resolved".into(),
            Self::DeliveredCount => "Orders delivered".into(),
            Self::OnTimeCount => "Orders on time".into(),
            Self::ExpiredCount => "Orders expired".into(),
            Self::Permit(van) => format!("{van:?} van may act this tick"),
            Self::UsedPermit(van) => format!("{van:?} van has acted this tick"),
            Self::RoadSpace => "Trunk road free".into(),
            Self::OnMainRoad => "On the trunk road".into(),
            Self::DockSpace => "Free hub staging space".into(),
            Self::DockUsed => "Hub staging space used".into(),
            Self::At(location) => format!("At {location:?}"),
            Self::Travelling(from, to) => format!("Travelling {from:?} → {to:?}"),
            Self::AwaitingOutcome => "Awaiting the trip outcome".into(),
            Self::Ready => "Ready".into(),
            Self::DepartedAt => "Departed at".into(),
            Self::ReadyAt => "Busy until".into(),
            Self::Trip => "Trips taken".into(),
            Self::Integrity => "Roadworthy".into(),
            Self::Damage => "Broken down".into(),
            Self::Wear => "Wear".into(),
            Self::MaintainedWear => "Wear serviced".into(),
            Self::Fuel => "Fuel".into(),
            Self::TankSpace => "Free tank space".into(),
            Self::SpentFuel => "Fuel spent".into(),
            Self::Space => "Free payload space".into(),
            Self::ColdSpace => "Free refrigerated slot".into(),
            Self::Cargo(id) => format!("Carrying Order {}", id + 1),
            Self::Money => "Cash".into(),
            Self::Revenue => "Revenue".into(),
            Self::Price => "Price".into(),
            Self::FuelPump => "Free fuel pump".into(),
            Self::Refueling => "Refueling".into(),
            Self::RepairBay => "Free repair bay".into(),
            Self::Repairing => "In the repair bay".into(),
            Self::Part => "Spare parts".into(),
            Self::UsedPart => "Parts used".into(),
            Self::TowTruck => "Free tow truck".into(),
            Self::Towing => "Being towed".into(),
            Self::Scheduled => "Arriving later".into(),
            Self::Waiting => "Waiting for pickup".into(),
            Self::InTransit => "In transit".into(),
            Self::Delivered => "Delivered".into(),
            Self::Expired => "Expired".into(),
            Self::Arrival => "Released at".into(),
            Self::Deadline => "Delivery deadline".into(),
            Self::Destination(location) => format!("Bound for {location:?}"),
            Self::Weight => "Weight".into(),
            Self::ColdCargo => "Needs refrigeration".into(),
            Self::TicketWord(van, from, to) => {
                format!("Hidden trip outcomes: {van:?} van, {from:?} → {to:?}")
            }
            Self::TicketDivisor => "Trip ticket cursor".into(),
            Self::TransferCount => "Van-to-van transfers".into(),
            Self::StagedCount => "Hub handoffs staged".into(),
            Self::DamageCount => "Breakdowns".into(),
        }
    }
}

impl StudioLabel for fleet::Role {
    fn studio_label(&self) -> String {
        match self {
            Self::World => "Fleet",
            Self::Vehicle => "Van",
            Self::Other => "Other van",
            Self::Order => "Order",
            Self::Station => "Station",
            Self::Nature => "Nature",
        }
        .into()
    }
}

impl StudioLabel for fleet::AccountId {
    fn studio_label(&self) -> String {
        match self {
            Self::World => "Fleet clock, budget and road capacity".into(),
            Self::Station => "Fuel, repair and towing operator".into(),
            Self::Nature => "Seeded vehicle and route tickets".into(),
            Self::Vehicle(van) => format!("{van:?} van"),
            Self::Order(id) => format!("Order {}", id + 1),
        }
    }
}

impl StudioLabel for fleet::RateId {
    fn studio_label(&self) -> String {
        match self {
            Self::Release(id) => format!("Order {} becomes available", id + 1),
            Self::Load { van, order, at } => {
                format!("{van:?} van loads Order {} at {at:?}", order + 1)
            }
            Self::Stage { van, order } => {
                format!("{van:?} van stages Order {} at the shared hub", order + 1)
            }
            Self::Transfer {
                from,
                to,
                order,
                at,
            } => format!(
                "Transfer Order {} from {from:?} van to {to:?} van at {at:?}",
                order + 1
            ),
            Self::Depart { van, from, to } => format!("{van:?} van departs {from:?} → {to:?}"),
            Self::Resolve {
                van,
                from,
                to,
                outcome,
            } => format!(
                "{van:?} van's trip {from:?} → {to:?} {}",
                match outcome {
                    fleet::Outcome::Clear => "is clear",
                    fleet::Outcome::Delayed => "is delayed",
                    fleet::Outcome::Breakdown => "breaks down",
                }
            ),
            Self::Arrive { van, from, to } => {
                format!("{van:?} van arrives at {to:?} from {from:?}")
            }
            Self::Deliver { van, order, late } => format!(
                "{van:?} van delivers Order {} {}",
                order + 1,
                if *late {
                    "late for half payment"
                } else {
                    "within its window"
                }
            ),
            Self::Expire { order, carrier, at } => match carrier {
                Some(van) => format!(
                    "Order {} expires undelivered aboard the {van:?} van",
                    order + 1
                ),
                None => format!("Order {} expires undelivered at {at:?}", order + 1),
            },
            Self::Refuel(van) => format!("{van:?} van occupies the pump and buys 3 fuel"),
            Self::EndRefuel(van) => {
                format!("{van:?} van finishes refueling and releases the pump")
            }
            Self::Repair { van, damaged } => format!(
                "{van:?} van starts {} in the repair bay",
                if *damaged {
                    "breakdown repair"
                } else {
                    "preventive maintenance"
                }
            ),
            Self::EndRepair { van, damaged } => format!(
                "{van:?} van finishes {} and releases the repair bay",
                if *damaged {
                    "breakdown repair"
                } else {
                    "maintenance"
                }
            ),
            Self::Tow { van, from } => {
                format!("Dispatch the shared tow truck for the {van:?} van at {from:?}")
            }
            Self::EndTow(van) => {
                format!("Tow the {van:?} van to the depot and release the tow truck")
            }
            Self::Tick => "Advance shared fleet time one tick".into(),
            Self::Close => {
                "Close the fleet run with all orders and occupied services settled".into()
            }
        }
    }
}
