//! How the perishable inventory is presented: its labels, units, scene and leaderboards.

use crate::perishables::{self, AccountId, Asset, Cohort, Location, Moment, World};
use axionomy::path;
use axionomy_view::{
    Format, Label, QuantityFormat, Scene, TimelineLaneView, TimelineSpanView, ViewId,
};

pub(super) fn scene(_: u64, world: &World) -> Option<Scene> {
    let lanes = [
        (path!("world"), "World clock"),
        (path!("ambient"), "Ambient cohort"),
        (path!("refrigerated"), "Refrigerated cohort"),
        (path!("warehouse"), "Warehouse claims"),
        (path!("fridge"), "Fridge claims"),
    ]
    .into_iter()
    .map(|(key, label)| TimelineLaneView {
        id: ViewId::new(key, label),
        classes: Vec::new(),
    })
    .collect();
    let moment = [
        Moment::Harvest,
        Moment::AmbientExpiry,
        Moment::WarmedExpiry,
        Moment::ColdExpiry,
    ]
    .into_iter()
    .position(|moment| {
        !world
            .balance(&AccountId::World, &Asset::Now(moment))
            .is_zero()
    })
    .unwrap_or(0) as u64;
    let ambient_rotten = !world
        .balance(
            &AccountId::Cohort(Cohort::Ambient),
            &Asset::Rotten(Cohort::Ambient),
        )
        .is_zero();
    let cold_rotten = !world
        .balance(
            &AccountId::Cohort(Cohort::Refrigerated),
            &Asset::Rotten(Cohort::Refrigerated),
        )
        .is_zero();
    let powered = !world
        .balance(&AccountId::Storage(Location::Fridge), &Asset::Powered)
        .is_zero();
    let warehouse = world.balance(
        &AccountId::Storage(Location::Warehouse),
        &Asset::Claim(Cohort::Ambient),
    );
    let fridge = world.balance(
        &AccountId::Storage(Location::Fridge),
        &Asset::Claim(Cohort::Refrigerated),
    );
    let spans = vec![
        TimelineSpanView {
            id: path!("ambient-condition"),
            lane: path!("ambient"),
            start: 0,
            end: 1,
            label: if ambient_rotten {
                "Rotten".into()
            } else {
                "Fresh until ambient expiry".into()
            },
            classes: if ambient_rotten {
                vec![path!("failed")]
            } else {
                Vec::new()
            },
        },
        TimelineSpanView {
            id: path!("cold-condition"),
            lane: path!("refrigerated"),
            start: 0,
            end: if powered { 3 } else { 2 },
            label: if cold_rotten {
                "Rotten after outage".into()
            } else if powered {
                "Cold + powered".into()
            } else {
                "Warming after outage".into()
            },
            classes: if powered {
                vec![path!("selected")]
            } else {
                vec![path!("uncertain")]
            },
        },
        TimelineSpanView {
            id: path!("warehouse-claims"),
            lane: path!("warehouse"),
            start: 0,
            end: 3,
            label: format!("{warehouse} fungible claims"),
            classes: Vec::new(),
        },
        TimelineSpanView {
            id: path!("fridge-claims"),
            lane: path!("fridge"),
            start: 0,
            end: 3,
            label: format!("{fridge} fungible claims"),
            classes: Vec::new(),
        },
    ];
    Some(Scene::timeline(
        "Batches, storage, and the spoilage clock",
        lanes,
        spans,
        Some(moment),
    ))
}

// How Studio names and writes this problem's values.

fn cohort(cohort: Cohort) -> &'static str {
    match cohort {
        Cohort::Ambient => "ambient",
        Cohort::Refrigerated => "refrigerated",
    }
}

/// Where a batch's stock stays fresh.
fn exposure(exposure: perishables::Exposure) -> &'static str {
    match exposure {
        perishables::Exposure::Ambient => "in ambient storage",
        perishables::Exposure::Cold => "in cold storage",
        perishables::Exposure::WarmedAfterOutage => "after warming in the outage",
    }
}

fn moment(moment: Moment) -> &'static str {
    match moment {
        Moment::Harvest => "harvest",
        Moment::AmbientExpiry => "ambient expiry",
        Moment::WarmedExpiry => "warmed expiry",
        Moment::ColdExpiry => "cold expiry",
    }
}

fn sentence(text: &str) -> String {
    let mut characters = text.chars();
    characters.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(characters).collect()
    })
}

impl Format for perishables::Asset {
    fn format(&self) -> QuantityFormat {
        match self {
            Self::CoolingEnergy | Self::SpentCoolingEnergy => QuantityFormat::COUNT.unit("units"),
            _ => QuantityFormat::COUNT,
        }
    }
}

impl Label for perishables::AccountId {
    fn label(&self) -> String {
        match self {
            Self::World => "Clock and power".into(),
            Self::Storage(location) => format!("{location:?}"),
            Self::Cohort(batch) => format!("{} batch condition", sentence(cohort(*batch))),
        }
    }
}

impl Label for perishables::Asset {
    fn label(&self) -> String {
        match self {
            Self::WorldIdentity => "Clock identity".into(),
            Self::LocationIdentity(location) => format!("{location:?} identity"),
            Self::CohortIdentity(batch) => {
                format!("{} batch identity", sentence(cohort(*batch)))
            }
            Self::Claim(batch) => format!("Units in the {} batch", cohort(*batch)),
            Self::Consumed(batch) => format!("Used units from the {} batch", cohort(*batch)),
            Self::Fresh(batch, held, expiry) => format!(
                "{} batch fresh {}, until {}",
                sentence(cohort(*batch)),
                exposure(*held),
                moment(*expiry)
            ),
            Self::Rotten(batch) => format!("Spoiled units from the {} batch", cohort(*batch)),
            Self::Ambient => "Ambient storage".into(),
            Self::Cold => "Cold storage".into(),
            Self::Powered => "Refrigeration running".into(),
            Self::Unpowered => "Refrigeration down".into(),
            Self::CoolingEnergy => "Cooling energy left".into(),
            Self::SpentCoolingEnergy => "Cooling energy spent".into(),
            Self::Now(at) => format!("Clock at {}", moment(*at)),
            Self::Before(at) => format!("Before {}", moment(*at)),
            Self::Reached(at) => format!("{} reached", sentence(moment(*at))),
            Self::Active => "Inventory simulation running".into(),
            Self::Solved => "Inventory simulation complete".into(),
            Self::Planning => "Storage plan open".into(),
            Self::PlanSolved => "Storage plan complete".into(),
        }
    }
}

impl Label for perishables::RateId {
    fn label(&self) -> String {
        match self {
            Self::MoveToFridge => "Move ambient batch units into the fridge".into(),
            Self::Advance { from, to } => {
                format!("Advance time: {} → {}", moment(*from), moment(*to))
            }
            Self::LosePower => "Refrigeration loses power".into(),
            Self::Eat {
                cohort: batch,
                exposure: held,
            } => format!("Use fresh {} units {}", cohort(*batch), exposure(*held)),
            Self::Spoil {
                cohort: batch,
                exposure: held,
            } => format!("Spoil the {} batch {}", cohort(*batch), exposure(*held)),
            Self::Finish => "Finish the spoilage simulation".into(),
            Self::SealStoragePlan => "Commit the storage plan".into(),
        }
    }
}

impl Label for perishables::Role {
    fn label(&self) -> String {
        match self {
            Self::World => "World",
            Self::Holder => "Holder",
            Self::Storage => "Storage",
            Self::Cohort => "Batch",
            Self::SourceStorage => "Source storage",
            Self::DestinationStorage => "Destination storage",
            Self::SourceCohort => "Source batch",
            Self::DestinationCohort => "Destination batch",
        }
        .into()
    }
}
