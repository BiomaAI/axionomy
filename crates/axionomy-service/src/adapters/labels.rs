use axionomy_problems::{
    amm, bridge, connect_four, exact_cover, logistics, marketplace, maze, mission, perishables,
    rescue, scheduling, sokoban, work_league, workshop,
};
use std::fmt::Debug;

pub(crate) trait StudioLabel {
    fn studio_label(&self) -> String;
}

fn words(value: &impl Debug) -> String {
    let debug = format!("{value:?}");
    let mut output = String::with_capacity(debug.len() + 4);
    let mut previous_lowercase = false;
    for character in debug.chars() {
        if matches!(character, '(' | ')' | '{' | '}' | '[' | ']') {
            output.push(' ');
            previous_lowercase = false;
        } else if character == ',' {
            output.push_str(", ");
            previous_lowercase = false;
        } else if character == '_' {
            output.push(' ');
            previous_lowercase = false;
        } else {
            if character.is_ascii_uppercase() && previous_lowercase {
                output.push(' ');
            }
            output.push(character);
            previous_lowercase = character.is_ascii_lowercase() || character.is_ascii_digit();
        }
    }
    output.split_whitespace().collect::<Vec<_>>().join(" ")
}

macro_rules! debug_labels {
    ($($type:path),+ $(,)?) => {$(
        impl StudioLabel for $type {
            fn studio_label(&self) -> String { words(self) }
        }
    )+};
}

debug_labels!(
    perishables::cold_chain::Asset,
    perishables::cold_chain::Role,
    rescue::incident::Asset,
    rescue::incident::Role,
    mission::expedition::Asset,
    mission::expedition::Role,
    logistics::fleet::Asset,
    logistics::fleet::Role
);
impl StudioLabel for logistics::fleet::AccountId {
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
impl StudioLabel for logistics::fleet::RateId {
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
                from, to, order, ..
            } => format!("Transfer Order {} from {from:?} to {to:?}", order + 1),
            Self::Depart { van, from, to } => format!("{van:?} van departs {from:?} → {to:?}"),
            Self::Resolve { van, outcome, .. } => {
                format!("{van:?} van's trip resolves: {outcome:?}")
            }
            Self::Arrive { van, to, .. } => format!("{van:?} van arrives at {to:?}"),
            Self::Deliver { van, order, late } => format!(
                "{van:?} delivers Order {} {}",
                order + 1,
                if *late {
                    "late for half payment"
                } else {
                    "within its window"
                }
            ),
            Self::Expire { order, .. } => format!("Order {} expires undelivered", order + 1),
            Self::Refuel(van) => format!("{van:?} occupies the pump and purchases 3 fuel"),
            Self::EndRefuel(van) => format!("{van:?} finishes refueling and releases the pump"),
            Self::Repair { van, damaged } => format!(
                "{van:?} starts {} in the repair bay",
                if *damaged {
                    "breakdown repair"
                } else {
                    "preventive maintenance"
                }
            ),
            Self::EndRepair { van, .. } => {
                format!("{van:?} finishes service and releases the repair bay")
            }
            Self::Tow { van, from } => {
                format!("Dispatch the shared tow truck for {van:?} at {from:?}")
            }
            Self::EndTow(van) => format!("Tow {van:?} to depot and release the tow truck"),
            Self::Tick => "Advance shared fleet time one tick".into(),
            Self::Close => {
                "Close the fleet run with all orders and occupied services settled".into()
            }
        }
    }
}
impl StudioLabel for mission::expedition::AccountId {
    fn studio_label(&self) -> String {
        match self {
            Self::World => "Team clock, weather and supplies".into(),
            Self::Nature => "Hidden mission sites and reports".into(),
            Self::Agent(agent) => format!("{agent:?}"),
            Self::Objective(id) => {
                if mission::expedition::patient(*id) {
                    format!("Casualty {}", id / 2 + 1)
                } else {
                    "Research archive".into()
                }
            }
        }
    }
}
impl StudioLabel for mission::expedition::RateId {
    fn studio_label(&self) -> String {
        match self {
            Self::Depart { agent, from, to } => {
                format!("{agent:?} starts travel {from:?} → {to:?}")
            }
            Self::Arrive {
                agent,
                to,
                hazardous,
            } => format!(
                "{agent:?} arrives at {to:?}{}",
                if *hazardous {
                    " and is injured by the hazard"
                } else {
                    ""
                }
            ),
            Self::Survey(location) => format!("Scout surveys {location:?}"),
            Self::Discover {
                objective,
                location,
            } => format!(
                "Scout discovers {} at {location:?}",
                mission::expedition::AccountId::Objective(*objective).studio_label()
            ),
            Self::Scan => "Scout requests a fallible sensor report".into(),
            Self::Report { location, .. } => format!("Private sensor report suggests {location:?}"),
            Self::Share {
                objective,
                location,
                remote,
            } => format!(
                "Brief Medic on {} at {location:?} {}",
                mission::expedition::AccountId::Objective(*objective).studio_label(),
                if *remote {
                    "by radio"
                } else {
                    "at a rendezvous"
                }
            ),
            Self::Collect { .. } => "Scout starts three-tick archive recovery".into(),
            Self::FinishCollect(_) => "Scout finishes recovery and carries the archive".into(),
            Self::TreatPatient { objective, .. } => format!(
                "Medic stabilizes {}",
                mission::expedition::AccountId::Objective(*objective).studio_label()
            ),
            Self::Extract { .. } => "Scout and Medic begin two-tick joint extraction".into(),
            Self::FinishExtract(_) => "Joint extraction ends; Medic carries the casualty".into(),
            Self::Deliver(objective) => format!(
                "Deliver {} safely to base",
                mission::expedition::AccountId::Objective(*objective).studio_label()
            ),
            Self::TreatAgent { target, .. } => format!("Medic treats {target:?}'s injury"),
            Self::Resupply(agent) => {
                format!(
                    "{agent:?} takes finite base energy{}",
                    if *agent == mission::expedition::Agent::Medic {
                        " and a medical kit"
                    } else {
                        ""
                    }
                )
            }
            Self::Weather { location, opening } => format!(
                "{location:?} {}",
                if *opening {
                    "reopens as the hazard clears"
                } else {
                    "closes as the hazard arrives"
                }
            ),
            Self::Clinical {
                objective, active, ..
            } => format!(
                "{}: {}",
                mission::expedition::AccountId::Objective(*objective).studio_label(),
                if *active {
                    "condition deteriorates"
                } else {
                    "outcome already settled"
                }
            ),
            Self::FinishClinical(_) => "Finish the scheduled casualty assessment".into(),
            Self::Fail {
                objective, dead, ..
            } => format!(
                "{} {}",
                mission::expedition::AccountId::Objective(*objective).studio_label(),
                if *dead {
                    "dies before evacuation"
                } else {
                    "misses the mission deadline"
                }
            ),
            Self::Tick(day) => format!("Advance shared mission time to {}", day + 1),
            Self::Close => "Close the mission with every objective accounted for".into(),
        }
    }
}
impl StudioLabel for rescue::incident::AccountId {
    fn studio_label(&self) -> String {
        match self {
            Self::World => "Incident clock and hospital".into(),
            Self::Responder => "Evacuation crew".into(),
            Self::Nature => "Hidden patient locations and sensor reports".into(),
            Self::Patient(id) => format!("Patient {}", id + 1),
        }
    }
}
impl StudioLabel for rescue::incident::RateId {
    fn studio_label(&self) -> String {
        match self {
            Self::Drive { from, to, fast } => format!(
                "Drive {from:?} → {to:?}{}",
                if *fast {
                    " at emergency speed"
                } else {
                    " conserving fuel"
                }
            ),
            Self::Search(location) => format!("Search {location:?} for casualties"),
            Self::Scan => "Spend a sensor charge on a new location hint".into(),
            Self::Report { location, .. } => {
                format!("Sensor suggests {location:?}; hint may be wrong")
            }
            Self::Discover { patient, location } => {
                format!("Discover Patient {} at {location:?}", patient + 1)
            }
            Self::Deteriorate(patient) => format!("Patient {} loses vitality", patient + 1),
            Self::Stabilize { patient, .. } => format!(
                "Stabilize Patient {} and extend the evacuation window",
                patient + 1
            ),
            Self::Load { patient, .. } => {
                format!("Load Patient {} into the evacuation seat", patient + 1)
            }
            Self::Unload(patient) => format!("Admit Patient {} to the hospital", patient + 1),
            Self::Lose { patient, cause, .. } => {
                format!("Resolve Patient {}: {cause:?}", patient + 1)
            }
            Self::Resupply => "Take the hospital's finite fuel, energy and medical reserve".into(),
            Self::Wait => "Advance incident time by one tick".into(),
            Self::Close => "Close the incident with all patient outcomes recorded".into(),
        }
    }
}
impl StudioLabel for perishables::cold_chain::AccountId {
    fn studio_label(&self) -> String {
        match self {
            Self::World => "Cold-chain resources".into(),
            Self::Buyer => "Funded food deliveries".into(),
            Self::Batch(id) => format!("Cohort {}", id + 1),
        }
    }
}
impl StudioLabel for perishables::cold_chain::RateId {
    fn studio_label(&self) -> String {
        match self {
            Self::Split {
                source,
                destination,
                ..
            } => format!(
                "Split Cohort {} into Cohort {} with the same heat history",
                source + 1,
                destination + 1
            ),
            Self::Move { batch, to, .. } => format!("Move Cohort {} to {to:?}", batch + 1),
            Self::Process(batch) => format!("Preserve fresh food in Cohort {}", batch + 1),
            Self::Deliver {
                batch, preserved, ..
            } => format!(
                "Deliver {} food from Cohort {}",
                if *preserved { "preserved" } else { "fresh" },
                batch + 1
            ),
            Self::Discard { batch, .. } => format!(
                "Discard spoiled food from Cohort {} and release space",
                batch + 1
            ),
            Self::Cool => "Power refrigeration for today".into(),
            Self::Generate => "Burn shared fuel for backup electricity".into(),
            Self::EndDay => "Close today's decisions and settle thermal exposure".into(),
            Self::AgeEmpty(batch) => format!("Cohort {} has no fresh food to age", batch + 1),
            Self::AgeCold(batch) => format!(
                "Cohort {} stays cold without resetting its prior heat",
                batch + 1
            ),
            Self::AgeWarm { batch, spoil, .. } => format!(
                "Cohort {} {}",
                batch + 1,
                if *spoil {
                    "crosses the spoilage threshold"
                } else {
                    "accumulates heat"
                }
            ),
            Self::OpenDay(day) => format!(
                "Open day {} with the encoded grid supply and new obligations",
                day + 1
            ),
            Self::Close => "Close the cold-chain horizon and record unmet demand".into(),
        }
    }
}

fn maze_node(node: maze::Node) -> String {
    words(&node)
}

impl StudioLabel for maze::Node {
    fn studio_label(&self) -> String {
        maze_node(*self)
    }
}

impl StudioLabel for maze::AccountId {
    fn studio_label(&self) -> String {
        match self {
            Self::Agent => "Agent".into(),
            Self::World => "Vault District".into(),
        }
    }
}

impl StudioLabel for maze::Asset {
    fn studio_label(&self) -> String {
        match *self {
            Self::PowerTransit => "Power routed to the canal crossing".into(),
            Self::PowerVault => "Power routed to the vault shutter".into(),
            Self::Health => "Health reserve".into(),
            Self::Injury => "Injuries slowing each action".into(),
            Self::Shield => "Protective equipment charges".into(),
            Self::UsedShield => "Spent protection".into(),
            Self::Medicine => "Available medicine".into(),
            Self::UsedMedicine => "Medicine used".into(),
            Self::At(node) => format!("Standing in {}", maze_node(node)),
            Self::Edge(from, to) => {
                format!("Passage {} → {}", maze_node(from), maze_node(to))
            }
            Self::Key => "Key".into(),
            Self::Locked => "Vault gate locked".into(),
            Self::Open => "Vault gate open".into(),
            Self::Energy => "Energy left".into(),
            Self::SpentEnergy => "Energy spent".into(),
            Self::Time => "Time left".into(),
            Self::SpentTime => "Time spent".into(),
            Self::Target(node) => format!("Goal: reach {}", maze_node(node)),
            Self::Distance(node) => format!("Distance from {} to the exit", maze_node(node)),
            Self::Active => "Expedition in progress".into(),
            Self::Solved => "Explorer escaped".into(),
        }
    }
}

impl StudioLabel for maze::RateId {
    fn studio_label(&self) -> String {
        match *self {
            Self::DivertPower { at, to_vault } => format!(
                "Divert power at {} to {}",
                maze_node(at),
                if to_vault { "vault" } else { "canal" }
            ),
            Self::TakeProtection { at } => {
                format!("Collect protective equipment at {}", maze_node(at))
            }
            Self::Treat { at } => format!("Treat injuries at {}", maze_node(at)),
            Self::ProtectedMove { from, to, energy } => format!(
                "Protected crossing {} → {} ({energy} energy)",
                maze_node(from),
                maze_node(to)
            ),
            Self::Move {
                from,
                to,
                energy,
                needs_open_door,
            } => format!(
                "Walk {} → {} ({} energy{})",
                maze_node(from),
                maze_node(to),
                energy,
                if needs_open_door {
                    ", needs the vault gate open"
                } else {
                    ""
                }
            ),
            Self::TakeKey { at } => format!("Pick up the key in {}", maze_node(at)),
            Self::UnlockDoor { at } => format!("Unlock the gate from {}", maze_node(at)),
            Self::Finish => "Reach the exit".into(),
        }
    }
}

debug_labels!(maze::Role);

impl StudioLabel for sokoban::AccountId {
    fn studio_label(&self) -> String {
        match self {
            Self::Cell(cell) => format!("Cell {cell}"),
            Self::Success => "Puzzle".into(),
        }
    }
}

impl StudioLabel for sokoban::Asset {
    fn studio_label(&self) -> String {
        match self {
            Self::DeadSquare => "A crate cannot reach a goal from here".into(),
            Self::ExitCell => "Warehouse exit".into(),
            Self::AtExit => "Player at the exit".into(),
            Self::AwayFromExit => "Player inside the warehouse".into(),
            Self::Plate(id) => format!("Pressure plate {}", id + 1),
            Self::Door(id) => format!("Door {}", id + 1),
            Self::Panel(id) => format!("Latch panel {}", id + 1),
            Self::Pressed(id) => format!("Plate {} has a crate", id + 1),
            Self::Released(id) => format!("Plate {} is released", id + 1),
            Self::LatchReady(id) => format!("Door {} can be latched", id + 1),
            Self::Latched(id) => format!("Door {} latched open", id + 1),
            Self::CellIdentity(cell) => format!("This is cell {cell}"),
            Self::BoardWidth(width) => format!("Board width is {width}"),
            Self::BoardHeight(height) => format!("Board height is {height}"),
            Self::Floor => "Walkable floor".into(),
            Self::Wall => "Wall".into(),
            Self::Player => "Player".into(),
            Self::Crate(crate_id) => format!("Crate {}", crate_id + 1),
            Self::Empty => "Empty cell".into(),
            Self::GoalCell => "Goal square".into(),
            Self::Active => "Puzzle in progress".into(),
            Self::Solved => "Puzzle solved".into(),
        }
    }
}

impl StudioLabel for sokoban::RateId {
    fn studio_label(&self) -> String {
        match self {
            Self::Latch { circuit, at } => {
                format!("Latch door {} from panel at cell {at}", circuit + 1)
            }
            Self::Move { from, to } => format!("Move from cell {from} to cell {to}"),
            Self::Push {
                behind,
                crate_at,
                to,
                crate_id,
            } => format!(
                "Push crate {} from cell {crate_at} to cell {to} (stand at {behind})",
                crate_id + 1
            ),
            Self::Finish { assignment } => format!(
                "Finish with {} stable crates on their assigned goals",
                assignment.len()
            ),
        }
    }
}

debug_labels!(sokoban::Role);

impl StudioLabel for exact_cover::AccountId {
    fn studio_label(&self) -> String {
        "Exact-cover problem".into()
    }
}

impl StudioLabel for exact_cover::Asset {
    fn studio_label(&self) -> String {
        match self {
            Self::Uncovered(element) => format!("Element {element:?} is uncovered"),
            Self::Covered(element) => format!("Element {element:?} is covered"),
            Self::Available(set) => format!("Subset {set:?} is available"),
            Self::Selected(set) => format!("Subset {set:?} selected"),
            Self::Progress(count) => format!("{count} elements covered"),
            Self::Active => "Cover in progress".into(),
            Self::Solved => "Exact cover found".into(),
        }
    }
}

impl StudioLabel for exact_cover::RateId {
    fn studio_label(&self) -> String {
        match self {
            Self::Select { set, before } => {
                format!("Select subset {set:?} after covering {before} elements")
            }
            Self::Finish => "Accept the exact cover".into(),
        }
    }
}

debug_labels!(exact_cover::Role);

impl StudioLabel for workshop::AccountId {
    fn studio_label(&self) -> String {
        match self {
            Self::Workshop => "Workshop".into(),
            Self::Customer(id) => format!("Customer {}", id + 1),
        }
    }
}

impl StudioLabel for workshop::Asset {
    fn studio_label(&self) -> String {
        match self {
            Self::CustomerIdentity(id) => format!("Customer {} identity", id + 1),
            Self::Credit => "Credit".into(),
            Self::Frame => "Prepared frames".into(),
            Self::Panel => "Prepared panel material".into(),
            Self::Offcut => "Recyclable offcuts".into(),
            Self::Table => "Finished tables".into(),
            Self::Durability => "Tool durability".into(),
            Self::Wear => "Tool wear".into(),
            Self::ToolPotential => "Upgrade capacity".into(),
            Self::RepairKit => "Repair kits".into(),
            Self::UsedRepairKit => "Used repair kits".into(),
            Self::UpgradeKit => "Available upgrade".into(),
            Self::Upgraded => "Precision tool upgrade".into(),
            Self::Idle => "Workbench available".into(),
            Self::Contract(id) => format!("Contract {} available", id + 1),
            Self::Raw(id) => format!("Contract {} awaiting components", id + 1),
            Self::Prepared(id) => format!("Contract {} components prepared", id + 1),
            Self::Assembled(id) => format!("Contract {} ready to deliver", id + 1),
            Self::Fulfilled(id) => format!("Contract {} fulfilled", id + 1),
            Self::Completed => "Contracts delivered".into(),
            Self::Wood => "Wood left".into(),
            Self::Labor => "Labor left".into(),
            Self::SpentLabor => "Labor used".into(),
            Self::Tool => "Reusable tool".into(),
            Self::Chair => "Finished chairs".into(),
            Self::Waste => "Scrap".into(),
            Self::Time => "Time left".into(),
            Self::SpentTime => "Time spent".into(),
            Self::Active => "Order in progress".into(),
            Self::Solved => "Order complete".into(),
        }
    }
}

impl StudioLabel for workshop::RateId {
    fn studio_label(&self) -> String {
        match self {
            Self::Accept(id) => format!("Accept contract {}", id + 1),
            Self::Prepare {
                order,
                precise,
                upgraded,
            } => format!(
                "Prepare components for contract {} ({})",
                order + 1,
                if *upgraded {
                    "upgraded tool"
                } else if *precise {
                    "precision cut"
                } else {
                    "fast cut"
                }
            ),
            Self::Assemble(id) => format!("Assemble contract {}", id + 1),
            Self::Deliver(id) => format!("Deliver contract {} and collect payment", id + 1),
            Self::Repair => "Restore worn tool with a repair kit".into(),
            Self::Upgrade => "Install the precision tool upgrade".into(),
            Self::Recycle => "Recover wood from two offcuts".into(),
            Self::BasicChair => "Build one chair (fast, more scrap)".into(),
            Self::EfficientBatch => "Build two chairs (slower, less scrap)".into(),
            Self::CounterfeitChair => "Counterfeit chair proposal".into(),
            Self::Finish => "Complete the order".into(),
        }
    }
}

debug_labels!(workshop::Role);

impl StudioLabel for amm::Actor {
    fn studio_label(&self) -> String {
        match self {
            Self::Founder => "Founding LP",
            Self::Generator => "Solar Generator",
            Self::Factory => "Factory",
            Self::Household => "Household",
            Self::Speculator => "Informed Speculator",
            Self::AdaptiveLp => "Adaptive LP",
            Self::Arbitrageur => "Internal Arbitrageur",
            Self::Whale => "Demand Whale",
        }
        .into()
    }
}

impl StudioLabel for amm::AccountId {
    fn studio_label(&self) -> String {
        match self {
            Self::Pool => "Energy / Credit AMM".into(),
            Self::Treasury => "Closed-economy Treasury".into(),
            Self::Information => "Scarcity Information".into(),
            Self::Actor(actor) => actor.studio_label(),
            Self::World => "Market calendar and production conditions".into(),
        }
    }
}

impl StudioLabel for amm::Asset {
    fn studio_label(&self) -> String {
        match self {
            Self::ActorIdentity(actor) => format!("Identity: {}", actor.studio_label()),
            Self::PoolIdentity => "Identity: energy pool".into(),
            Self::TreasuryIdentity => "Identity: procurement treasury".into(),
            Self::WorldIdentity => "Identity: market calendar".into(),
            Self::MarketActive => "Market season active".into(),
            Self::MarketEnded => "Market season closed".into(),
            Self::Day => "Completed market days".into(),
            Self::AtDay(day) => format!("Market day {}", day + 1),
            Self::Days => "Days in season".into(),
            Self::Scale => "Production scale".into(),
            Self::MarketOpen => "Market access open".into(),
            Self::Trading => "Still trading today".into(),
            Self::Done => "Trading finished for today".into(),
            Self::Decisions => "Decisions remaining today".into(),
            Self::StorageCapacity => "Energy storage capacity".into(),
            Self::ProductionCapacity => "Daily generating capacity".into(),
            Self::ProductionAllowance => "Remaining production today".into(),
            Self::FutureCapacity => "Available capacity expansion".into(),
            Self::PendingCapacity => "Capacity under construction".into(),
            Self::ConstructionDue => "Construction completes at day index".into(),
            Self::ReferencePrice => "Smoothed prior closing price (milli)".into(),
            Self::Sunlight(day) => format!("Day {} sunlight percent", day + 1),
            Self::CurrentSunlight => "Current sunlight percent".into(),
            Self::LiquidityDecision => "Liquidity adjustment available".into(),
            Self::FutureNeed(actor) => format!("{} future energy needs", actor.studio_label()),
            Self::MissedNeed(actor) => format!("{} unmet energy needs", actor.studio_label()),
            Self::FutureObligation(actor) => format!("{} future obligations", actor.studio_label()),
            Self::DefaultedObligation(actor) => {
                format!("{} missed obligations", actor.studio_label())
            }
            Self::UnmadeGoods => "Remaining production contracts".into(),
            Self::Goods => "Manufactured goods awaiting sale".into(),
            Self::DeliveredGoods => "Goods delivered to procurement".into(),
            Self::IncomeDue => "Household income available".into(),
            Self::FutureIncome => "Future income entitlements".into(),
            Self::ReceivedIncome => "Income payments received".into(),
            Self::MissedIncome => "Unpaid income entitlements".into(),
            Self::Energy => "Usable energy".into(),
            Self::Credit => "Settlement credit".into(),
            Self::SolarPotential => "Unharvested solar potential".into(),
            Self::SpentEnergy => "Energy consumed toward goals".into(),
            Self::LpShare => "AMM liquidity share".into(),
            Self::UnissuedLpShare => "Unissued liquidity share".into(),
            Self::Need(actor) => format!("{} energy need", actor.studio_label()),
            Self::SatisfiedNeed(actor) => format!("{} need satisfied", actor.studio_label()),
            Self::Obligation(actor) => format!("{} credit obligation", actor.studio_label()),
            Self::SettledObligation(actor) => {
                format!("{} obligation settled", actor.studio_label())
            }
            Self::Uninformed(actor) => format!("{} has no shortage signal", actor.studio_label()),
            Self::Informed(actor) => format!("{} knows about the shortage", actor.studio_label()),
            Self::ShortageSignal => "Verified energy shortage signal".into(),
            Self::Utility => "Realized utility".into(),
        }
    }
}

impl StudioLabel for amm::RateId {
    fn studio_label(&self) -> String {
        match self {
            Self::EndTrading(actor) => format!("{} finishes trading today", actor.studio_label()),
            Self::AdvanceDay(day) => format!("Close day {} and open day {}", day + 1, day + 2),
            Self::CloseMarket => "Close the market season and record unmet needs".into(),
            Self::InvestProduction => "Fund new generating capacity".into(),
            Self::CommissionProduction => "Bring completed generating capacity online".into(),
            Self::DeliverGoods => "Factory sells manufactured goods".into(),
            Self::CollectIncome => "Household collects its daily income".into(),
            Self::FundLiquidity => "Add liquidity at current reserves with bounded rounding".into(),
            Self::BuyEnergy => "Buy energy from the AMM".into(),
            Self::SellEnergy => "Sell energy into the AMM".into(),
            Self::AddLiquidity => "Deepen AMM liquidity".into(),
            Self::RemoveLiquidity => "Withdraw AMM liquidity".into(),
            Self::ProduceEnergy => "Harvest solar energy".into(),
            Self::UseEnergy(actor) => format!("{} uses energy", actor.studio_label()),
            Self::SettleObligation(actor) => {
                format!("{} settles a credit obligation", actor.studio_label())
            }
            Self::LearnShortage(actor) => {
                format!("{} receives the shortage signal", actor.studio_label())
            }
        }
    }
}

debug_labels!(amm::Role);

fn job(job: scheduling::Job) -> &'static str {
    match job {
        scheduling::Job::One => "Job One",
        scheduling::Job::Two => "Job Two",
    }
}

fn machine(machine: scheduling::Machine) -> &'static str {
    match machine {
        scheduling::Machine::One => "Machine One",
        scheduling::Machine::Two => "Machine Two",
        scheduling::Machine::Three => "Machine Three",
    }
}

impl StudioLabel for scheduling::AccountId {
    fn studio_label(&self) -> String {
        match self {
            Self::Job(value) => job(*value).into(),
            Self::Slot(value, slot) => format!("{} at slot {slot}", machine(*value)),
            Self::Machine(value) => machine(*value).into(),
            Self::Success => "Schedule".into(),
        }
    }
}

impl StudioLabel for scheduling::Asset {
    fn studio_label(&self) -> String {
        match self {
            Self::JobIdentity(value) => format!("This is {}", job(*value)),
            Self::SlotIdentity(value, slot) => format!("{} slot {slot}", machine(*value)),
            Self::ReadyAt(operation, slot) => format!("{operation:?} ready at {slot}"),
            Self::CompletedAt(operation, slot) => format!("{operation:?} completed at {slot}"),
            Self::Available => "Machine slot available".into(),
            Self::Reserved(operation) => format!("Reserved for {operation:?}"),
            Self::Makespan(slot) => format!("Overall finish at slot {slot}"),
            Self::Active => "Schedule in progress".into(),
            Self::Solved => "Schedule complete".into(),
            Self::MachineIdentity(value) => format!("Identity: {}", machine(*value)),
            Self::Ready(op) => format!("{op:?} ready for routing"),
            Self::Queued(op, value) => format!("{op:?} queued on {}", machine(*value)),
            Self::Running(op, value) => format!("{op:?} running on {}", machine(*value)),
            Self::Paused(op, value) => format!("{op:?} paused on {}", machine(*value)),
            Self::QueueFree => "Queue space".into(),
            Self::QueueTicket(op) => format!("Queue reservation for {op:?}"),
            Self::Idle => "Machine idle".into(),
            Self::Busy => "Machine occupied".into(),
            Self::Online => "Machine online".into(),
            Self::Broken => "Machine broken".into(),
            Self::Repairing => "Repair underway".into(),
            Self::Configured(value) => format!("Configured for {}", job(*value)),
            Self::Labor => "Available crew".into(),
            Self::BusyLabor => "Occupied crew".into(),
            Self::RepairKit => "Repair kit".into(),
            Self::UsedRepairKit => "Repair kit used".into(),
            Self::Arrival => "Job release time".into(),
            Self::Due => "Activity completes at".into(),
            Self::WorkRemaining => "Interrupted work remaining".into(),
            Self::Done => "Job completed".into(),
            Self::CompletionTime => "Actual completion time".into(),
            Self::Now => "World time".into(),
            Self::TimeRemaining => "Workday remaining".into(),
            Self::FaultAt => "Scheduled disruption time".into(),
            Self::FaultPending => "Disruption pending".into(),
            Self::FaultOccurred => "Disruption occurred".into(),
            Self::RepairDuration => "Repair duration".into(),
            Self::NotStarted(op) => format!("{op:?} has not started"),
            Self::Started(op) => format!("{op:?} execution started"),
            Self::StartTime(op) => format!("{op:?} actual start"),
            Self::EndTime(op) => format!("{op:?} actual completion"),
            Self::ExecutedOn(op, value) => format!("{op:?} uses {}", machine(*value)),
            Self::Interrupted(op) => format!("{op:?} was interrupted"),
        }
    }
}

impl StudioLabel for scheduling::RateId {
    fn studio_label(&self) -> String {
        match self {
            Self::Schedule {
                operation,
                ready,
                start,
            } => format!("Run {operation:?} from slot {start} (ready at {ready})"),
            Self::Finish {
                job_one_end,
                job_two_end,
                makespan,
            } => format!(
                "Finish schedule (Job One {job_one_end}, Job Two {job_two_end}, makespan {makespan})"
            ),
            Self::Queue {
                operation,
                machine: value,
            } => format!("Queue {operation:?} on {}", machine(*value)),
            Self::CancelQueue {
                operation,
                machine: value,
            } => format!("Release {operation:?}'s queue on {}", machine(*value)),
            Self::Start {
                operation,
                machine: value,
                configured,
            } => format!(
                "Start {operation:?} on {} (configured for {})",
                machine(*value),
                job(*configured)
            ),
            Self::Complete { operation, .. } => format!("Complete {operation:?}"),
            Self::Resume { operation, .. } => format!("Resume interrupted {operation:?}"),
            Self::Fault { operation } => format!(
                "Machine Two fails{}",
                operation.map_or(String::new(), |op| format!(" during {op:?}"))
            ),
            Self::Repair => "Begin Machine Two repair".into(),
            Self::CompleteRepair => "Complete Machine Two repair".into(),
            Self::Advance { to } => format!("Advance workday to {to}"),
            Self::Close => "Close the completed workday".into(),
        }
    }
}

debug_labels!(scheduling::Role);

impl StudioLabel for rescue::AccountId {
    fn studio_label(&self) -> String {
        match self {
            Self::Agent => "Responder".into(),
            Self::Nature => "Hidden scenario".into(),
        }
    }
}

impl StudioLabel for rescue::Asset {
    fn studio_label(&self) -> String {
        match self {
            Self::At(location) => format!("Responder at {location:?}"),
            Self::Energy => "Energy left".into(),
            Self::SpentEnergy => "Energy spent".into(),
            Self::Sensor => "Sensor available".into(),
            Self::UsedSensor => "Sensor used".into(),
            Self::Unresolved => "Location not yet drawn".into(),
            Self::ScenarioWeight(location, seed) => {
                format!("Weight for {location:?}, sensor state {seed}")
            }
            Self::Truth(location) => format!("Survivor is at {location:?}"),
            Self::Seed(seed) => format!("Sensor state {seed}"),
            Self::Belief(location) => format!("Reading points to {location:?}"),
            Self::Planning => "Choosing an action".into(),
            Self::AwaitingObservation => "Waiting for a sensor reading".into(),
            Self::Committed => "Destination chosen".into(),
            Self::Rescued => "Survivor reached".into(),
            Self::Evacuated => "Survivor evacuated".into(),
            Self::Solved => "Rescue complete".into(),
        }
    }
}

impl StudioLabel for rescue::RateId {
    fn studio_label(&self) -> String {
        match self {
            Self::Instantiate { truth, seed } => {
                format!("Draw scenario: {truth:?}, sensor state {seed}")
            }
            Self::BeginObserve => "Use the sensor".into(),
            Self::ResolveObservation { report, .. } => {
                format!("Receive sensor reading: {report:?}")
            }
            Self::Move { from, to } => format!("Move {from:?} → {to:?}"),
            Self::Rescue { location } => format!("Reach survivor at {location:?}"),
            Self::Evacuate { location } => format!("Evacuate from {location:?} to Base"),
            Self::Finish => "Complete the rescue".into(),
        }
    }
}

debug_labels!(rescue::Role);

impl StudioLabel for bridge::AccountId {
    fn studio_label(&self) -> String {
        match self {
            Self::Agent(agent) => format!("Agent {agent:?}"),
            Self::Bridge => "Single-lane bridge".into(),
        }
    }
}

impl StudioLabel for bridge::Asset {
    fn studio_label(&self) -> String {
        match self {
            Self::AgentIdentity(agent) => format!("Identity: Agent {agent:?}"),
            Self::BridgeIdentity => "Bridge identity".into(),
            Self::At(side) => format!("At the {side:?} bank"),
            Self::Energy => "Crossing energy".into(),
            Self::SpentEnergy => "Energy spent crossing".into(),
            Self::Credit => "Credit available".into(),
            Self::Escrow => "Credit in escrow".into(),
            Self::SpentCredit => "Credit spent".into(),
            Self::Bid(amount) => format!("Bid {amount}"),
            Self::CanBid => "Can bid or claim".into(),
            Self::Submitted => "Bid submitted".into(),
            Self::CrossingRight => "Crossing right".into(),
            Self::FirstTurn => "First crossing open".into(),
            Self::SecondTurn => "Second crossing open".into(),
            Self::PriorityBenefit => "Priority crossings won".into(),
            Self::Waiting => "Waiting for the lane".into(),
            Self::Crossed => "Crossed this round".into(),
            Self::CompletedTrip => "Completed prior rounds".into(),
            Self::RoundsRemaining => "Rounds remaining after this one".into(),
            Self::CapacityFree => "Lane available".into(),
            Self::Active => "Allocation in progress".into(),
            Self::Solved => "Allocation complete".into(),
            Self::Now => "World time".into(),
            Self::TimeRemaining => "Time remaining".into(),
            Self::Arrival => "Arrival time".into(),
            Self::Deadline => "Arrival deadline".into(),
            Self::LeaseUntil => "Reservation valid through".into(),
            Self::ReadyAt => "Travel completes at".into(),
            Self::InTransit => "On the bridge".into(),
            Self::OnDetour => "On the alternate route".into(),
            Self::Detouring => "Detouring".into(),
            Self::OnTime => "On-time trips".into(),
            Self::Late => "Late trips".into(),
            Self::MissedReservation => "Expired reservations".into(),
            Self::Round => "Prior traffic waves".into(),
        }
    }
}

impl StudioLabel for bridge::RateId {
    fn studio_label(&self) -> String {
        match self {
            Self::SubmitBid { agent, amount } => format!("Agent {agent:?} bids {amount}"),
            Self::Resolve {
                winner,
                winning_bid,
                losing_bid,
            } => format!("Settle auction: Agent {winner:?} wins {winning_bid} to {losing_bid}"),
            Self::ClaimFirst { agent } => format!("Agent {agent:?} claims first crossing"),
            Self::ClaimSecond { agent } => format!("Agent {agent:?} claims second crossing"),
            Self::YieldToWaiting { agent } => format!("Grant waiting Agent {agent:?} the lane"),
            Self::Cross { agent } => format!("Agent {agent:?} crosses"),
            Self::ResetRound => "Start the next allocation round".into(),
            Self::Finish => "Complete all crossing rounds".into(),
            Self::Advance { to } => format!("Advance world time to {to}"),
            Self::Complete {
                agent,
                detour,
                late,
            } => format!(
                "Agent {agent:?} finishes {} {}",
                if *detour { "detour" } else { "crossing" },
                if *late { "late" } else { "on time" }
            ),
            Self::Detour { agent, .. } => format!("Agent {agent:?} pays for the alternate route"),
            Self::ExpireReservation { agent } => {
                format!("Release Agent {agent:?}'s expired reservation")
            }
            Self::WithdrawBid { agent, amount } => {
                format!("Agent {agent:?} withdraws bid {amount}")
            }
        }
    }
}

debug_labels!(bridge::Role);

impl StudioLabel for marketplace::AccountId {
    fn studio_label(&self) -> String {
        match self {
            Self::Buyer(id) => format!("Buyer {id:?}"),
            Self::Seller(id) => format!("Seller {id:?}"),
            Self::Platform => "Marketplace platform".into(),
            Self::TaxAuthority => "Tax authority".into(),
            Self::Carrier(id) => format!("Carrier {id:?}"),
            Self::Order(id) => format!("Order {id:?}"),
        }
    }
}

impl StudioLabel for marketplace::Asset {
    fn studio_label(&self) -> String {
        match self {
            Self::Money => "Money".into(),
            Self::Item(item) => format!("{item:?} inventory"),
            Self::PurchaseIntent(order) => format!("Wants Order {order:?}"),
            Self::PurchaseReceipt(order) => format!("Bought Order {order:?}"),
            Self::SaleOffer(item) => format!("Offers one {item:?}"),
            Self::CompletedSale(order) => format!("Sold Order {order:?}"),
            Self::MarketplaceLicense => "Marketplace licence".into(),
            Self::TaxPolicy => "Tax policy".into(),
            Self::ShippingCapacity => "Shipping capacity left".into(),
            Self::UsedShippingCapacity => "Shipping capacity used".into(),
            Self::OpenOrder(order) => format!("Order {order:?} is open"),
            Self::SettledOrder(order) => format!("Order {order:?} settled"),
            Self::SettledValue(value) => format!("Settled value {value}"),
            Self::Utility => "Participant benefit".into(),
            Self::BuyerLicense => "Buyer identity".into(),
            Self::SellerLicense => "Seller identity".into(),
            Self::CarrierLicense => "Carrier identity".into(),
            Self::Contract(order) => format!("Order {order:?} reserved"),
            Self::InShipment(order) => format!("Order {order:?} in transit"),
            Self::PurchaseLocked(order) => format!("Buyer committed to {order:?}"),
            Self::SaleReserved(order) => format!("Seller committed to {order:?}"),
            Self::Booking(order) => format!("Carrier booked for {order:?}"),
            Self::DeliveredBy(order) => format!("Delivered Order {order:?}"),
            Self::AgreedPrice(price) => format!("Agreed price {price}"),
            Self::Arrival => "Order release time".into(),
            Self::Deadline => "Delivery deadline".into(),
            Self::LeaseUntil => "Dispatch reservation expires at".into(),
            Self::ReadyAt => "Delivery available at".into(),
            Self::Now => "World time".into(),
            Self::TimeRemaining => "Time remaining".into(),
            Self::TransitTime => "Base shipping duration".into(),
            Self::DelayRisk => "Exposure to route delays".into(),
            Self::RouteDelay => "Route delay".into(),
            Self::OnTime => "On-time deliveries".into(),
            Self::Late => "Late deliveries".into(),
            Self::Refunded => "Expired reservations refunded".into(),
        }
    }
}

impl StudioLabel for marketplace::RateId {
    fn studio_label(&self) -> String {
        match self {
            Self::SettleOrder(order) => {
                format!("Agree Order {order:?} (Micro: settle; larger markets: reserve)")
            }
            Self::DiscountedContract(order) => {
                format!("Agree Order {order:?} with a 10-credit concession")
            }
            Self::Dispatch(order) => format!("Dispatch Order {order:?}"),
            Self::Deliver { order, late, .. } => format!(
                "Deliver Order {order:?} {}",
                if *late { "late with rebate" } else { "on time" }
            ),
            Self::Cancel { order, .. } => format!("Refund expired reservation for {order:?}"),
            Self::Advance { to } => format!("Advance market time to {to}"),
        }
    }
}

debug_labels!(marketplace::Role);

impl StudioLabel for logistics::AccountId {
    fn studio_label(&self) -> String {
        match self {
            Self::Vehicle => "Delivery vehicle".into(),
            Self::Order(order) => format!("Order {order:?}"),
            Self::Nature => "Weather and breakdowns".into(),
            Self::FuelStation => "Fuel station".into(),
        }
    }
}

impl StudioLabel for logistics::Asset {
    fn studio_label(&self) -> String {
        match self {
            Self::At(location) => format!("Vehicle at {location:?}"),
            Self::Traveling(route) => format!("Traveling on {route:?}"),
            Self::Fuel => "Fuel left".into(),
            Self::SpentFuel => "Fuel spent".into(),
            Self::Money => "Money".into(),
            Self::TimeRemaining => "Time left".into(),
            Self::ElapsedTime => "Elapsed time".into(),
            Self::CargoSpace => "Cargo space available".into(),
            Self::CargoOccupied => "Cargo space occupied".into(),
            Self::RepairTool => "Repair tool".into(),
            Self::Waiting => "Waiting on a travel outcome".into(),
            Self::InTransit => "Order in transit".into(),
            Self::Delivered => "Order delivered".into(),
            Self::Package(order) => format!("Package for Order {order:?}"),
            Self::WeatherReady => "Nature ready for travel".into(),
            Self::OutcomeWeight(route, outcome) => {
                format!("Weight: {route:?} is {outcome:?}")
            }
            Self::Outcome(route, outcome) => format!("{route:?} outcome: {outcome:?}"),
        }
    }
}

impl StudioLabel for logistics::RateId {
    fn studio_label(&self) -> String {
        match self {
            Self::Load(order) => format!("Load Order {order:?}"),
            Self::Depart(route) => format!("Depart via {route:?}"),
            Self::Resolve(route, outcome) => format!("Draw {route:?} outcome: {outcome:?}"),
            Self::Arrive(route, outcome) => format!("Arrive via {route:?} after {outcome:?}"),
            Self::Repair(route) => format!("Repair after breakdown on {route:?}"),
            Self::Deliver(order) => format!("Deliver Order {order:?}"),
            Self::Refuel => "Refuel the vehicle".into(),
        }
    }
}

debug_labels!(logistics::Role);

impl StudioLabel for connect_four::AccountId {
    fn studio_label(&self) -> String {
        match self {
            Self::Game => "Connect Four game".into(),
            Self::Column(column) => format!("Column {column}"),
            Self::Cell { column, row } => format!("Cell ({column},{row})"),
            Self::Result => "Game result".into(),
        }
    }
}

impl StudioLabel for connect_four::Asset {
    fn studio_label(&self) -> String {
        match self {
            Self::SegmentVacancies(segment) => format!("Unoccupied cells in line {segment}"),
            Self::SegmentPieces(player, segment) => format!("{player:?} pieces in line {segment}"),
            Self::GameIdentity => "Game identity".into(),
            Self::ColumnIdentity(column) => format!("This is column {column}"),
            Self::CellIdentity { column, row } => format!("This is cell ({column},{row})"),
            Self::ResultIdentity => "Result identity".into(),
            Self::Empty => "Empty cell".into(),
            Self::Piece(player) => format!("{player:?} piece"),
            Self::NextRow(row) => format!("Next piece lands on row {row}"),
            Self::ColumnFull => "Column full".into(),
            Self::Turn(player) => format!("{player:?} to move"),
            Self::LineCount(player, line, count) => {
                format!("{player:?} has {count} in {line:?}")
            }
            Self::Winner(player) => format!("{player:?} wins"),
            Self::Draw => "Draw".into(),
            Self::BoardSize { width, height } => format!("Board is {width}×{height}"),
            Self::Pending(player) => format!("Check {player:?} move result"),
        }
    }
}

impl StudioLabel for connect_four::RateId {
    fn studio_label(&self) -> String {
        match self {
            Self::Move {
                player,
                column,
                row,
                ..
            }
            | Self::StandardMove {
                player,
                column,
                row,
            } => format!("{player:?} drops into column {column}, lands on row {row}"),
            Self::Draw(player) | Self::StandardDraw(player) => {
                format!("Declare a draw after {player:?}'s move")
            }
            Self::ClaimWin { player, segment } => {
                format!("{player:?} claims winning line {segment}")
            }
            Self::Continue(player) => format!("No win; pass the turn after {player:?}"),
        }
    }
}

debug_labels!(connect_four::Role);

impl StudioLabel for mission::AccountId {
    fn studio_label(&self) -> String {
        match self {
            Self::Agent(agent) => format!("{agent:?}"),
            Self::Nature => "Hidden mission scenario".into(),
            Self::Mission => "Mission clock and state".into(),
            Self::Success => "Mission objective".into(),
        }
    }
}

impl StudioLabel for mission::Asset {
    fn studio_label(&self) -> String {
        match self {
            Self::AgentIdentity(agent) => format!("Identity: {agent:?}"),
            Self::NatureIdentity => "Nature identity".into(),
            Self::MissionIdentity => "Mission identity".into(),
            Self::SuccessIdentity => "Objective identity".into(),
            Self::At(location) => format!("At {location:?}"),
            Self::Energy => "Energy left".into(),
            Self::SpentEnergy => "Energy spent".into(),
            Self::Sensor => "Sensor available".into(),
            Self::UsedSensor => "Sensor used".into(),
            Self::MedicalKit => "Medical kit available".into(),
            Self::UsedMedicalKit => "Medical kit used".into(),
            Self::Intel(location) => format!("Private sighting: {location:?}"),
            Self::SharedIntel(location) => format!("Shared sighting: {location:?}"),
            Self::Injured => "Agent injured".into(),
            Self::Unresolved => "Scenario not yet drawn".into(),
            Self::ScenarioWeight(location, seed, hazard) => {
                format!("Weight: {location:?}, state {seed}, {hazard:?}")
            }
            Self::Truth(location) => format!("Objective at {location:?}"),
            Self::Seed(seed) => format!("Observation state {seed}"),
            Self::Hazard(hazard) => format!("Hazard: {hazard:?}"),
            Self::HazardResolved => "Hazard resolved".into(),
            Self::TimeRemaining => "Time left".into(),
            Self::ElapsedTime => "Elapsed time".into(),
            Self::Planning => "Choosing an action".into(),
            Self::AwaitingScan => "Waiting for a sighting".into(),
            Self::AwaitingEncounter => "Waiting for a hazard outcome".into(),
            Self::NeedsTreatment => "Treatment required".into(),
            Self::AreaSafe => "Area made safe".into(),
            Self::Rescued => "Objective rescued".into(),
            Self::Solved => "Mission complete".into(),
        }
    }
}

impl StudioLabel for mission::RateId {
    fn studio_label(&self) -> String {
        match self {
            Self::Instantiate {
                truth,
                seed,
                hazard,
            } => format!("Draw scenario: {truth:?}, state {seed}, {hazard:?}"),
            Self::BeginScan => "Scout scans the area".into(),
            Self::ResolveScan { report, .. } => format!("Scout sees {report:?}"),
            Self::Share(location) => format!("Scout shares sighting: {location:?}"),
            Self::MoveTogether(location) => format!("Scout and Medic move to {location:?}"),
            Self::MoveDirect(location) => format!("Move directly to {location:?}"),
            Self::Encounter { location, hazard } => {
                format!("Encounter {hazard:?} hazard at {location:?}")
            }
            Self::Treat => "Medic treats the injury".into(),
            Self::Rescue(location) => format!("Rescue objective at {location:?}"),
            Self::Finish => "Complete the mission".into(),
        }
    }
}

debug_labels!(mission::Role);

impl StudioLabel for perishables::AccountId {
    fn studio_label(&self) -> String {
        match self {
            Self::World => "Clock and power".into(),
            Self::Storage(location) => format!("{location:?}"),
            Self::Cohort(cohort) => format!("{cohort:?} batch condition"),
        }
    }
}

impl StudioLabel for perishables::Asset {
    fn studio_label(&self) -> String {
        match self {
            Self::WorldIdentity => "This account is the world clock".into(),
            Self::LocationIdentity(location) => format!("This account is the {location:?}"),
            Self::CohortIdentity(cohort) => format!("This is the {cohort:?} batch"),
            Self::Claim(cohort) => format!("Units in the {cohort:?} batch"),
            Self::Consumed(cohort) => format!("Used units from the {cohort:?} batch"),
            Self::Fresh(cohort, exposure, expiry) => format!(
                "{cohort:?} batch, still fresh under {exposure:?} exposure, expires at {expiry:?}"
            ),
            Self::Rotten(cohort) => format!("Spoiled units from the {cohort:?} batch"),
            Self::Ambient => "Ambient storage".into(),
            Self::Cold => "Cold storage".into(),
            Self::Powered => "Refrigeration running".into(),
            Self::Unpowered => "Refrigeration down".into(),
            Self::CoolingEnergy => "Cooling energy left".into(),
            Self::SpentCoolingEnergy => "Cooling energy spent".into(),
            Self::Now(moment) => format!("Clock is at {moment:?}"),
            Self::Before(moment) => format!("Before {moment:?}"),
            Self::Reached(moment) => format!("{moment:?} passed"),
            Self::Active => "Inventory simulation running".into(),
            Self::Solved => "Inventory simulation complete".into(),
            Self::Planning => "Storage plan open".into(),
            Self::PlanSolved => "Storage plan complete".into(),
        }
    }
}

impl StudioLabel for perishables::RateId {
    fn studio_label(&self) -> String {
        match self {
            Self::MoveToFridge => "Move ambient batch units into the fridge".into(),
            Self::Advance { from, to } => format!("Advance time: {from:?} → {to:?}"),
            Self::LosePower => "Refrigeration loses power".into(),
            Self::Eat { cohort, exposure } => {
                format!("Use fresh {cohort:?} units under {exposure:?} exposure")
            }
            Self::Spoil { cohort, exposure } => {
                format!("Spoil {cohort:?} batch under {exposure:?} exposure")
            }
            Self::Finish => "Finish the spoilage simulation".into(),
            Self::SealStoragePlan => "Commit the storage plan".into(),
        }
    }
}

debug_labels!(perishables::Role);

impl StudioLabel for work_league::AccountId {
    fn studio_label(&self) -> String {
        match self {
            Self::Agent(agent) => format!("Worker {agent:?}"),
            Self::Job(job) => format!("Job {}", job.0),
            Self::Facility(facility) => format!("{facility:?} facility"),
            Self::Nature => "Disruption model".into(),
            Self::World => "Shared work clock and contracts".into(),
        }
    }
}

impl StudioLabel for work_league::Asset {
    fn studio_label(&self) -> String {
        match self {
            Self::WorldIdentity => "Identity: work economy".into(),
            Self::SeasonActive => "Season active".into(),
            Self::SeasonEnded => "Season ended".into(),
            Self::Tick => "World time".into(),
            Self::Horizon => "Season horizon".into(),
            Self::ContractsOpen => "Unresolved contracts".into(),
            Self::ContractsClosed => "Resolved contracts".into(),
            Self::Activities => "Activities in progress".into(),
            Self::Permit(agent) => format!("{agent:?} may decide this tick"),
            Self::Acted(agent) => format!("{agent:?} has decided this tick"),
            Self::Idle => "Ready for work".into(),
            Self::Busy => "Activity in progress".into(),
            Self::FreeContract => "Can accept a contract".into(),
            Self::ReadyAt => "Activity finishes at tick".into(),
            Self::ReleaseAt => "Contract opens at tick".into(),
            Self::Deadline => "Delivery deadline".into(),
            Self::LeaseUntil => "Claim reserved through tick".into(),
            Self::Expired => "Expired without delivery".into(),
            Self::Transferable => "Handoff available".into(),
            Self::HandedOff => "Contract handed off".into(),
            Self::Assists => "Paid handoffs".into(),
            Self::CompletedJob(job) => format!("Dependency completed: job {}", job.0),
            Self::Credit => "Spendable credit".into(),
            Self::Wear => "Equipment wear".into(),
            Self::Repairing => "Under repair".into(),
            Self::FacilityIdentity(facility) => format!("Identity: {facility:?}"),
            Self::Vacant(facility) => format!("{facility:?} service slot free"),
            Self::Occupied(facility, agent) => format!("{facility:?} occupied by {agent:?}"),
            Self::Servicing(facility) => format!("Receiving {facility:?} service"),
            Self::ChargingEnergy => "Energy reserved for charging".into(),
            Self::ReclaimingMaterial => "Material being reclaimed".into(),
            Self::EmbodiedMaterial => "Material incorporated into work".into(),
            Self::RecyclingLoss => "Material lost during recycling".into(),
            Self::AgentIdentity(agent) => format!("Identity: {agent:?}"),
            Self::JobIdentity(job) => format!("Identity: job {}", job.0),
            Self::Policy(policy) => format!("Policy: {policy:?}"),
            Self::At(location) => format!("At {location:?}"),
            Self::Operational => "Operational".into(),
            Self::Energy => "Energy left".into(),
            Self::TimeRemaining => "Time left".into(),
            Self::Material => "Material left".into(),
            Self::Available => "Available for claim".into(),
            Self::Assigned(agent) => format!("Assigned to {agent:?}"),
            Self::Claimed(job) => format!("Claim on job {}", job.0),
            Self::Pending => "Waiting for work".into(),
            Self::InProgress => "Work in progress".into(),
            Self::Awaiting(job, mode) => format!("Job {} awaiting {mode:?} outcome", job.0),
            Self::Resolved(job, mode, outcome) => {
                format!("Job {} {mode:?} outcome: {outcome:?}", job.0)
            }
            Self::Completed => "Jobs completed".into(),
            Self::Value => "Contract value earned".into(),
            Self::Attempts => "Work attempts".into(),
            Self::Successes => "Successful attempts".into(),
            Self::Failures => "Failed attempts".into(),
            Self::SpentEnergy => "Energy spent".into(),
            Self::ElapsedTime => "Elapsed time".into(),
            Self::MaterialSpent => "Material spent".into(),
            Self::Waste => "Residual waste".into(),
            Self::RecycledWaste => "Waste recycled".into(),
            Self::Damage => "Damage requiring repair".into(),
            Self::RepairSupply => "Repair supply".into(),
            Self::SpentRepairSupply => "Repair supply used".into(),
            Self::ChargeSupply => "Charging energy supply".into(),
            Self::RecyclerCapacity => "Recycler available".into(),
            Self::OutcomeWeight(job, mode, outcome) => {
                format!("Job {} {mode:?} {outcome:?} weight", job.0)
            }
        }
    }
}

impl StudioLabel for work_league::RateId {
    fn studio_label(&self) -> String {
        match self {
            Self::Tick => "Advance the shared work clock".into(),
            Self::Wait { agent } => format!("{agent:?} waits this tick"),
            Self::BeginService {
                agent,
                facility,
                maintenance,
            } => format!(
                "{agent:?} starts {} at {facility:?}",
                if *maintenance {
                    "preventive maintenance"
                } else {
                    "service"
                }
            ),
            Self::EndService { agent, facility } => {
                format!("{agent:?} finishes service and frees {facility:?}")
            }
            Self::Handoff {
                agent,
                recipient,
                job,
                ..
            } => format!(
                "{agent:?} hands job {} to {recipient:?} for 2 credits",
                job.0
            ),
            Self::ReleaseClaim {
                agent,
                job,
                expired,
            } => format!(
                "{agent:?} releases job {}{}",
                job.0,
                if *expired {
                    ": claim lease expired"
                } else {
                    " for another worker"
                }
            ),
            Self::Expire { job, .. } => format!("Job {} misses its delivery deadline", job.0),
            Self::EndSeason => "Close the resolved work season".into(),
            Self::Claim { agent, job } => format!("{agent:?} claims job {}", job.0),
            Self::Move { agent, from, to } => format!("{agent:?} moves {from:?} → {to:?}"),
            Self::Begin { agent, job, mode } => {
                format!("{agent:?} begins job {} in {mode:?} mode", job.0)
            }
            Self::Resolve {
                agent,
                job,
                outcome,
                ..
            } => format!("Resolve {agent:?} on job {}: {outcome:?}", job.0),
            Self::Finish {
                agent,
                job,
                outcome,
                ..
            } => format!("{agent:?} records job {} as {outcome:?}", job.0),
            Self::Repair { agent } => format!("Repair {agent:?}"),
            Self::Recharge { agent } => format!("Recharge {agent:?}"),
            Self::Recycle { agent } => format!("{agent:?} recycles one waste unit"),
        }
    }
}

debug_labels!(work_league::Role);
