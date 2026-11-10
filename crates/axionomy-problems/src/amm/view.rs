//! How the Living Market is presented: its labels, units, scene and leaderboards.

use crate::{
    amm::{self, ACTORS, AccountId, Actor, Asset, World},
    support::gcd,
};
use axionomy::path;
use axionomy_study::{ProblemPath, StudyError, problem_error};
use axionomy_view::{
    ExactQuantity, Format, Keyed, Label, LeaderboardEntryView, LeaderboardView, MarketActorView,
    MarketPoolView, ObjectiveDirectionView, QuantityFormat, Scene, SceneAnchorView,
    SceneEntityRoleView, SceneEntityView, SceneEvidenceRefView, SceneGlyphView, SceneSurfaceView,
    SceneToneView, ViewDocument, ViewId, account_key, asset_key, asset_metric, visual_metric,
};
use std::collections::BTreeMap;

fn exact(value: impl ToString) -> ExactQuantity {
    ExactQuantity(value.to_string())
}

fn actor_glyph(actor: Actor) -> SceneGlyphView {
    match actor {
        Actor::Founder | Actor::AdaptiveLp => SceneGlyphView::Organization,
        Actor::Generator => SceneGlyphView::Energy,
        Actor::Factory => SceneGlyphView::Machine,
        Actor::Household => SceneGlyphView::Person,
        Actor::Speculator => SceneGlyphView::Information,
        Actor::Arbitrageur => SceneGlyphView::Move,
        Actor::Whale => SceneGlyphView::Money,
    }
}

fn actor_status(world: &World, actor: Actor) -> Option<String> {
    let account = AccountId::Actor(actor);
    if world.account(&AccountId::World).is_some() {
        let read = |asset| world.balance(&account, &asset).to_u64();
        return Some(match actor {
            Actor::Generator => format!(
                "{} capacity · {} building",
                read(Asset::ProductionCapacity),
                read(Asset::PendingCapacity)
            ),
            Actor::Factory | Actor::Household => format!(
                "{} needs met · {} missed",
                read(Asset::SatisfiedNeed(actor)),
                read(Asset::MissedNeed(actor))
            ),
            Actor::Founder | Actor::AdaptiveLp => {
                format!("{} liquidity shares", read(Asset::LpShare))
            }
            Actor::Speculator if read(Asset::Informed(actor)) > 0 => {
                "next-day shortage observed".into()
            }
            _ => format!("{} stored energy", read(Asset::Energy)),
        });
    }
    if world
        .balance(&account, &Asset::SatisfiedNeed(actor))
        .to_u64()
        > 0
    {
        Some("need satisfied".into())
    } else if world.balance(&account, &Asset::Informed(actor)).to_u64() > 0 {
        Some("shortage informed".into())
    } else if world
        .balance(&account, &Asset::SettledObligation(actor))
        .to_u64()
        > 0
    {
        Some("obligation settled".into())
    } else if world.balance(&account, &Asset::LpShare).to_u64() > 0 {
        Some("liquidity provider".into())
    } else {
        None
    }
}

fn actor_tone(world: &World, actor: Actor) -> SceneToneView {
    let account = AccountId::Actor(actor);
    if !world.balance(&account, &Asset::MissedNeed(actor)).is_zero()
        || !world
            .balance(&account, &Asset::DefaultedObligation(actor))
            .is_zero()
    {
        SceneToneView::Warning
    } else if actor_status(world, actor).is_some() {
        SceneToneView::Success
    } else if matches!(actor, Actor::Whale | Actor::Speculator) {
        SceneToneView::Warning
    } else {
        SceneToneView::Neutral
    }
}

pub(super) fn scene(_: u64, world: &World) -> Option<Scene> {
    let state = amm::pool_state(world);
    let positions = [
        (12.0, 17.0),
        (11.0, 44.0),
        (13.0, 72.0),
        (36.0, 87.0),
        (64.0, 87.0),
        (87.0, 72.0),
        (89.0, 44.0),
        (88.0, 17.0),
    ];
    let actors = ACTORS
        .into_iter()
        .zip(positions)
        .map(|(actor, (x, y))| {
            let account = AccountId::Actor(actor);
            MarketActorView {
                id: ViewId::new(path!("amm/actor").then(&actor), actor.label()),
                account: account_key::<amm::Ontology>(&account),
                glyph: actor_glyph(actor),
                tone: actor_tone(world, actor),
                x,
                y,
                energy: exact(world.balance(&account, &Asset::Energy)),
                credit: exact(world.balance(&account, &Asset::Credit)),
                liquidity: exact(world.balance(&account, &Asset::LpShare)),
                utility: exact(world.balance(&account, &Asset::Utility)),
                status: actor_status(world, actor),
            }
        })
        .collect::<Vec<_>>();
    let entities = ACTORS
        .into_iter()
        .zip(&actors)
        .map(|(id, actor)| {
            let account = AccountId::Actor(id);
            SceneEntityView {
                id: actor.id.clone(),
                glyph: actor.glyph,
                anchor: SceneAnchorView::Unanchored,
                role: SceneEntityRoleView::Occupant,
                tone: actor.tone,
                status: actor.status.clone(),
                account: Some(actor.account.clone()),
                evidence: vec![SceneEvidenceRefView::Account {
                    account: actor.account.clone(),
                }],
                metrics: vec![
                    asset_metric(
                        path!("energy"),
                        "Energy",
                        &Asset::Energy,
                        world.balance(&account, &Asset::Energy),
                    ),
                    asset_metric(
                        path!("credit"),
                        "Credit",
                        &Asset::Credit,
                        world.balance(&account, &Asset::Credit),
                    ),
                ],
            }
        })
        .collect();
    let mut scene = Scene {
        title: "The Living Market".into(),
        surface: SceneSurfaceView::Market {
            pool: Box::new(MarketPoolView {
                id: ViewId::new(path!("amm/pool/energy-credit"), "Energy / Credit AMM"),
                base_asset: ViewId::new(asset_key::<amm::Ontology>(&Asset::Energy), "Energy"),
                quote_asset: ViewId::new(asset_key::<amm::Ontology>(&Asset::Credit), "Credit"),
                base_reserve: exact(state.energy),
                quote_reserve: exact(state.credit),
                price_milli: exact(state.price_milli),
                product: exact(state.product),
                issued_liquidity: exact(state.issued_lp_shares),
                fee_numerator: amm::FEE_NUMERATOR,
                fee_denominator: amm::FEE_DENOMINATOR,
                account: account_key::<amm::Ontology>(&AccountId::Pool),
            }),
            actors,
        },
        entities,
        paths: Vec::new(),
        annotations: Vec::new(),
        metrics: vec![
            visual_metric(
                path!("price"),
                "Discovered price",
                format_price(state.price_milli),
                Some("credit / energy"),
            ),
            asset_metric(
                path!("energy-reserve"),
                "Energy reserve",
                &Asset::Energy,
                state.energy,
            ),
            asset_metric(
                path!("liquidity"),
                "Liquidity shares issued",
                &Asset::LpShare,
                state.issued_lp_shares,
            ),
        ],
        legend: Vec::new(),
    };
    if world.account(&AccountId::World).is_some() {
        scene.metrics.insert(
            0,
            visual_metric(
                path!("day"),
                "Market day",
                amm::period(world) + 1,
                Some("day"),
            ),
        );
        scene.metrics.extend([
            visual_metric(
                path!("unmet"),
                "Unmet daily needs",
                unmet_needs(world),
                Some("needs"),
            ),
            visual_metric(
                path!("defaults"),
                "Missed obligations",
                defaults(world),
                Some("payments"),
            ),
            asset_metric(
                path!("sunlight"),
                "Sunlight",
                &Asset::CurrentSunlight,
                world.balance(&AccountId::World, &Asset::CurrentSunlight),
            ),
            asset_metric(
                path!("capacity"),
                "Generating capacity",
                &Asset::ProductionCapacity,
                world.balance(
                    &AccountId::Actor(Actor::Generator),
                    &Asset::ProductionCapacity,
                ),
            ),
            asset_metric(
                path!("goods"),
                "Goods delivered",
                &Asset::DeliveredGoods,
                world.balance(&AccountId::Treasury, &Asset::DeliveredGoods),
            ),
        ]);
    }
    Some(scene)
}

pub(super) fn unmet_needs(world: &World) -> u64 {
    [Actor::Factory, Actor::Household]
        .into_iter()
        .map(|actor| {
            world
                .balance(&AccountId::Actor(actor), &Asset::MissedNeed(actor))
                .to_u64()
        })
        .sum()
}

pub(super) fn defaults(world: &World) -> u64 {
    [Actor::Factory, Actor::Generator]
        .into_iter()
        .map(|actor| {
            world
                .balance(&AccountId::Actor(actor), &Asset::DefaultedObligation(actor))
                .to_u64()
        })
        .sum()
}

fn format_price(price_milli: u64) -> String {
    format!("{}.{:03}", price_milli / 1_000, price_milli % 1_000)
}

pub(super) fn attach_contribution_leaderboards(
    initial: &World,
    trace: &axionomy::Trace<amm::Ontology>,
    document: &mut ViewDocument,
    problem: &ProblemPath,
) -> Result<(), StudyError> {
    let mut world = initial.clone();
    let mut contributions = BTreeMap::<Actor, i128>::new();
    document.initial.leaderboards = vec![contribution_board(&contributions)];
    for (index, exchange) in trace.exchanges().iter().enumerate() {
        let before = i128::from(amm::pool_state(&world).price_milli);
        world
            .apply(exchange.clone())
            .map_err(|error| problem_error(problem, format!("{error:?}")))?;
        let after = i128::from(amm::pool_state(&world).price_milli);
        if let Some(actor) = amm::price_actor(exchange) {
            *contributions.entry(actor).or_default() += after - before;
        }
        let frame = &mut document.frames[index];
        frame.after.leaderboards = vec![contribution_board(&contributions)];
    }
    Ok(())
}

fn contribution_board(contributions: &BTreeMap<Actor, i128>) -> LeaderboardView {
    let mut values = ACTORS
        .into_iter()
        .map(|actor| (actor, *contributions.get(&actor).unwrap_or(&0)))
        .collect::<Vec<_>>();
    values.sort_by(|left, right| right.1.cmp(&left.1).then(left.0.cmp(&right.0)));
    let mut previous = None;
    let mut rank = 0;
    let entries = values
        .into_iter()
        .enumerate()
        .map(|(index, (actor, value))| {
            if previous != Some(value) {
                rank = index as u64 + 1;
                previous = Some(value);
            }
            LeaderboardEntryView {
                rank: Some(rank),
                participant: ViewId::new(
                    account_key::<amm::Ontology>(&AccountId::Actor(actor)),
                    actor.label(),
                ),
                value: format_signed_milli(value),
                unit: Some("Δ credit / energy".into()),
                eligible: true,
                components: Vec::new(),
            }
        })
        .collect();
    LeaderboardView {
        key: path!("price-contribution"),
        label: "Direct price contribution".into(),
        description: "Each actor's cumulative change in the replayed reserve price. Contributions sum exactly to total price movement because proportional liquidity does not move price.".into(),
        direction: ObjectiveDirectionView::Maximize,
        entries,
    }
}

fn format_signed_milli(value: i128) -> String {
    let sign = match value.cmp(&0) {
        std::cmp::Ordering::Greater => "+",
        std::cmp::Ordering::Less => "-",
        std::cmp::Ordering::Equal => "",
    };
    let magnitude = value.unsigned_abs();
    format!("{sign}{}.{:03}", magnitude / 1_000, magnitude % 1_000)
}

pub(super) fn causal_contribution_board(initial: &World) -> LeaderboardView {
    let mut contributions = amm::shapley_price_contributions(initial);
    contributions.sort_by(|left, right| {
        right
            .numerator
            .cmp(&left.numerator)
            .then(left.actor.cmp(&right.actor))
    });
    let mut previous = None;
    let mut rank = 0;
    let entries = contributions
        .into_iter()
        .enumerate()
        .map(|(index, contribution)| {
            if previous != Some(contribution.numerator) {
                rank = index as u64 + 1;
                previous = Some(contribution.numerator);
            }
            let denominator = i128::from(contribution.denominator) * 1_000;
            let divisor = gcd(contribution.numerator.unsigned_abs(), denominator as u128);
            let reduced_numerator = contribution.numerator / divisor as i128;
            let reduced_denominator = denominator / divisor as i128;
            LeaderboardEntryView {
                rank: Some(rank),
                participant: ViewId::new(
                    account_key::<amm::Ontology>(&AccountId::Actor(contribution.actor)),
                    contribution.actor.label(),
                ),
                value: format!("{reduced_numerator}/{reduced_denominator}"),
                unit: Some("Δ credit / energy".into()),
                eligible: true,
                components: vec![visual_metric(
                    path!("coalitions"),
                    "Counterfactual coalitions",
                    1_u64 << amm::PRICE_ACTORS.len(),
                    None,
                )],
            }
        })
        .collect();
    LeaderboardView {
        key: path!("causal-price-contribution"),
        label: "Causal price contribution".into(),
        description: "Exact Shapley allocation across every coalition of active market actors. Each coalition is independently re-quoted and replayed through Axionomy; the reduced fractions sum to the full counterfactual price change.".into(),
        direction: ObjectiveDirectionView::Maximize,
        entries,
    }
}

// How Studio names and writes this problem's values.

/// Energy is counted in units and credit in credits, as the market scene
/// reads them; the season is counted in market days. The reference price is
/// kept in thousandths of a credit per unit of energy.
impl Format for amm::Asset {
    fn format(&self) -> QuantityFormat {
        match self {
            Self::Energy
            | Self::SpentEnergy
            | Self::SolarPotential
            | Self::StorageCapacity
            | Self::ProductionAllowance => QuantityFormat::COUNT.unit("units"),
            Self::ProductionCapacity | Self::FutureCapacity | Self::PendingCapacity => {
                QuantityFormat::COUNT.unit("units / day")
            }
            Self::Credit => QuantityFormat::COUNT.unit("credits"),
            Self::ReferencePrice => QuantityFormat::decimal(3).unit("credit / energy"),
            Self::Day | Self::Days | Self::ConstructionDue => QuantityFormat::COUNT.unit("days"),
            Self::Sunlight(_) | Self::CurrentSunlight => QuantityFormat::COUNT.unit("%"),
            _ => QuantityFormat::COUNT,
        }
    }
}

impl Label for amm::Actor {
    fn label(&self) -> String {
        match self {
            Self::Founder => "Founding LP",
            Self::Generator => "Solar generator",
            Self::Factory => "Factory",
            Self::Household => "Household",
            Self::Speculator => "Informed speculator",
            Self::AdaptiveLp => "Adaptive LP",
            Self::Arbitrageur => "Internal arbitrageur",
            Self::Whale => "Demand whale",
        }
        .into()
    }
}

impl Label for amm::AccountId {
    fn label(&self) -> String {
        match self {
            Self::Pool => "Energy / credit AMM".into(),
            Self::Treasury => "Closed-economy treasury".into(),
            Self::Information => "Scarcity information".into(),
            Self::Actor(actor) => actor.label(),
            Self::World => "Market calendar and production conditions".into(),
        }
    }
}

impl Label for amm::Asset {
    fn label(&self) -> String {
        match self {
            Self::ActorIdentity(actor) => format!("Identity: {}", actor.label()),
            Self::PoolIdentity => "Identity: energy pool".into(),
            Self::TreasuryIdentity => "Identity: procurement treasury".into(),
            Self::WorldIdentity => "Identity: market calendar".into(),
            Self::MarketActive => "Market season active".into(),
            Self::MarketEnded => "Market season closed".into(),
            Self::Day => "Time elapsed".into(),
            Self::AtDay(day) => format!("Market day {}", day + 1),
            Self::Days => "Season length".into(),
            Self::Scale => "Production scale".into(),
            Self::MarketOpen => "Market access open".into(),
            Self::Trading => "Still trading today".into(),
            Self::Done => "Trading finished for today".into(),
            Self::Decisions => "Decisions remaining today".into(),
            Self::StorageCapacity => "Energy storage capacity".into(),
            Self::ProductionCapacity => "Generating capacity".into(),
            Self::ProductionAllowance => "Remaining production today".into(),
            Self::FutureCapacity => "Capacity left to build".into(),
            Self::PendingCapacity => "Capacity under construction".into(),
            Self::ConstructionDue => "Construction completes at".into(),
            Self::ReferencePrice => "Smoothed prior closing price".into(),
            Self::Sunlight(day) => format!("Day {} sunlight", day + 1),
            Self::CurrentSunlight => "Current sunlight".into(),
            Self::LiquidityDecision => "Liquidity adjustment available".into(),
            Self::Need(actor) => format!("{} need due today", actor.label()),
            Self::FutureNeed(actor) => format!("{} needs still to come", actor.label()),
            Self::SatisfiedNeed(actor) => format!("{} needs met", actor.label()),
            Self::MissedNeed(actor) => format!("{} needs unmet", actor.label()),
            Self::Obligation(actor) => format!("{} obligation due today", actor.label()),
            Self::FutureObligation(actor) => {
                format!("{} obligations still to come", actor.label())
            }
            Self::SettledObligation(actor) => {
                format!("{} obligations settled", actor.label())
            }
            Self::DefaultedObligation(actor) => {
                format!("{} obligations missed", actor.label())
            }
            Self::UnmadeGoods => "Goods still to make".into(),
            Self::Goods => "Manufactured goods awaiting sale".into(),
            Self::DeliveredGoods => "Goods delivered to procurement".into(),
            Self::IncomeDue => "Income payment due today".into(),
            Self::FutureIncome => "Income payments still to come".into(),
            Self::ReceivedIncome => "Income payments received".into(),
            Self::MissedIncome => "Income payments missed".into(),
            Self::Energy => "Usable energy".into(),
            Self::Credit => "Settlement credit".into(),
            Self::SolarPotential => "Unharvested solar potential".into(),
            Self::SpentEnergy => "Energy consumed toward goals".into(),
            Self::LpShare => "AMM liquidity share".into(),
            Self::UnissuedLpShare => "Unissued liquidity share".into(),
            Self::Uninformed(actor) => format!("{} has no shortage signal", actor.label()),
            Self::Informed(actor) => format!("{} knows about the shortage", actor.label()),
            Self::ShortageSignal => "Verified energy shortage signal".into(),
            Self::Utility => "Realized utility".into(),
        }
    }
}

impl Label for amm::RateId {
    fn label(&self) -> String {
        match self {
            Self::EndTrading(actor) => format!("{} finishes trading today", actor.label()),
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
            Self::UseEnergy(actor) => format!("{} uses energy", actor.label()),
            Self::SettleObligation(actor) => {
                format!("{} settles a credit obligation", actor.label())
            }
            Self::LearnShortage(actor) => {
                format!("{} receives the shortage signal", actor.label())
            }
        }
    }
}

impl Label for amm::Role {
    fn label(&self) -> String {
        match self {
            Self::Pool => "Pool".into(),
            Self::Trader => "Trader".into(),
            Self::LiquidityProvider => "Liquidity provider".into(),
            Self::Producer => "Producer".into(),
            Self::Consumer => "Consumer".into(),
            Self::Treasury => "Treasury".into(),
            Self::Information => "Information source".into(),
            Self::Observer => "Observer".into(),
            Self::World => "Market calendar".into(),
            Self::Participant(actor) => actor.label(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::format_signed_milli;

    #[test]
    fn signed_milli_preserves_the_sign_of_fractional_moves() {
        assert_eq!(format_signed_milli(12_345), "+12.345");
        assert_eq!(format_signed_milli(0), "0.000");
        assert_eq!(format_signed_milli(-500), "-0.500");
        assert_eq!(
            format_signed_milli(i128::MIN),
            "-170141183460469231731687303715884105.728"
        );
    }
}
