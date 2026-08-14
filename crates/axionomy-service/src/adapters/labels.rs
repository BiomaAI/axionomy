use axionomy_problems::{
    bridge, connect_four, exact_cover, logistics, marketplace, maze, mission, perishables, rescue,
    scheduling, sokoban, work_league, workshop,
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
            Self::World => "Maze".into(),
        }
    }
}

impl StudioLabel for maze::Asset {
    fn studio_label(&self) -> String {
        match *self {
            Self::At(node) => format!("Standing in {}", maze_node(node)),
            Self::Edge(from, to) => {
                format!("Passage {} → {}", maze_node(from), maze_node(to))
            }
            Self::Key => "Key".into(),
            Self::Locked => "Door locked".into(),
            Self::Open => "Door open".into(),
            Self::Energy => "Energy left".into(),
            Self::SpentEnergy => "Energy spent".into(),
            Self::Time => "Time left".into(),
            Self::SpentTime => "Time spent".into(),
            Self::Target(node) => format!("Goal: reach {}", maze_node(node)),
            Self::Distance(node) => format!("Distance from {} to the exit", maze_node(node)),
            Self::Active => "Maze in progress".into(),
            Self::Solved => "Maze solved".into(),
        }
    }
}

impl StudioLabel for maze::RateId {
    fn studio_label(&self) -> String {
        match *self {
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
                    ", needs the door open"
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
        "Workshop".into()
    }
}

impl StudioLabel for workshop::Asset {
    fn studio_label(&self) -> String {
        match self {
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
            Self::BasicChair => "Build one chair (fast, more scrap)".into(),
            Self::EfficientBatch => "Build two chairs (slower, less scrap)".into(),
            Self::CounterfeitChair => "Counterfeit chair proposal".into(),
            Self::Finish => "Complete the order".into(),
        }
    }
}

debug_labels!(workshop::Role);

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
        }
    }
}

impl StudioLabel for marketplace::RateId {
    fn studio_label(&self) -> String {
        match self {
            Self::SettleOrder(order) => {
                format!("Settle Order {order:?} (buyer, seller, carrier, platform, and tax)")
            }
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
        }
    }
}

impl StudioLabel for work_league::Asset {
    fn studio_label(&self) -> String {
        match self {
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
