use super::*;
use axionomy_problems::amm::{self, ACTORS, AccountId, Actor, Asset, Scenario, World};
use axionomy_view::{
    ExactQuantity, LeaderboardEntryView, LeaderboardView, MarketActorView, MarketPoolView,
    ObjectiveDirectionView, SceneGlyphView, SceneSurfaceView, SceneToneView, TelemetryKindView,
};
use std::collections::BTreeMap;

pub(super) fn build(
    run: &ArtifactKey,
    descriptor: &ProblemDescriptor,
) -> Result<RunArtifact, ServiceError> {
    let initial = match run.instance {
        InstanceProfile::Micro => amm::initial(),
        InstanceProfile::Showcase => amm::initial_showcase_with_seed(run.seed),
        InstanceProfile::Stress => amm::initial_stress_with_seed(run.seed),
    };
    let scenarios = [
        (
            "market-day",
            Scenario::MarketDay,
            "Living Market · endogenous price discovery",
            "Actors repeatedly reconsider trades as production, inventory, daily needs and obligations change. Capacity investment takes two days; scarce storage and cloudy days change the cost of waiting. No external price exists.",
        ),
        (
            "no-whale",
            Scenario::NoWhale,
            "Living Market · without the demand whale",
            "The same closed economy replays without its largest buyer, exposing that actor's causal effect on the discovered exchange value.",
        ),
        (
            "thin-liquidity",
            Scenario::ThinLiquidity,
            "Living Market · thin-liquidity reality",
            "Half the founding liquidity exits at the opening. Actors respond to the changed prices and depth across the same production conditions and daily obligations.",
        ),
    ];

    let mut documents = Vec::new();
    for (strategy, scenario, title, description) in scenarios {
        let trace = amm::trace(&initial, scenario);
        let final_world = initial
            .replayed(&trace)
            .map_err(|error| problem_error("amm", format!("{error:?}")))?;
        let final_pool = amm::pool_state(&final_world);
        let mut view = document(
            DocumentSpec {
                problem: "amm",
                strategy,
                title,
                description,
                source_label: "Closed endogenous market",
            },
            &initial,
            &amm::market_goal(&initial),
            &trace,
            vec![
                ObjectiveView {
                    key: slug("needs"),
                    label: "Needs satisfied".into(),
                    direction: ObjectiveDirectionView::Maximize,
                    value: satisfied_needs(&final_world).to_string(),
                },
                ObjectiveView {
                    key: slug("utility"),
                    label: "Realized utility".into(),
                    direction: ObjectiveDirectionView::Maximize,
                    value: total_utility(&final_world).to_string(),
                },
                ObjectiveView {
                    key: slug("depth"),
                    label: "Energy liquidity".into(),
                    direction: ObjectiveDirectionView::Maximize,
                    value: final_pool.energy.to_string(),
                },
                ObjectiveView {
                    key: slug("unmet"),
                    label: "Unmet daily needs".into(),
                    direction: ObjectiveDirectionView::Minimize,
                    value: unmet_needs(&final_world).to_string(),
                },
                ObjectiveView {
                    key: slug("defaults"),
                    label: "Missed obligations".into(),
                    direction: ObjectiveDirectionView::Minimize,
                    value: defaults(&final_world).to_string(),
                },
            ],
            scene,
        )
        .map_err(|error| problem_error("amm", error))?;
        attach_contribution_leaderboards(&initial, &trace, &mut view)?;
        if matches!(scenario, Scenario::MarketDay)
            && let Some(final_frame) = view.frames.last_mut()
        {
            final_frame
                .after
                .leaderboards
                .push(causal_contribution_board(&initial));
        }
        view.telemetry.push(telemetry(
            "verified market replay",
            true,
            [
                (
                    TelemetryKindView::Transitions,
                    trace.exchanges().len() as u64,
                    "authoritative exchanges".into(),
                ),
                (
                    TelemetryKindView::Accounts,
                    initial.accounts().count() as u64,
                    "closed-economy accounts".into(),
                ),
                (
                    TelemetryKindView::Alternatives,
                    ACTORS.len() as u64,
                    "heterogeneous actors".into(),
                ),
            ],
        ));

        let quote = amm::quote_output(&initial, Asset::Credit, 10_000).unwrap_or_default();
        let guarded = amm::buy_energy(Actor::Factory, 10_000, quote.saturating_add(1));
        view.proposals.push(proposal(ProposalSpec {
                id: "impossible-minimum-output",
                label: "Demand more than the curve returns",
                description: "The trader supplies enough credit, but its minimum-output protection correctly refuses the AMM quote.",
            },
            &initial,
            &guarded,
        ));
        let wrong_ratio = amm::add_liquidity(Actor::AdaptiveLp, 1_000, 9_000, 1);
        view.proposals.push(proposal(ProposalSpec {
                id: "price-moving-liquidity",
                label: "Deposit liquidity at the wrong ratio",
                description: "Liquidity may deepen the discovered market price, but this rule refuses to rewrite that price by depositing an inconsistent reserve ratio.",
            },
            &initial,
            &wrong_ratio,
        ));
        documents.push(view);
    }

    artifact(run, descriptor, documents)
}

fn exact(value: impl ToString) -> ExactQuantity {
    ExactQuantity(value.to_string())
}

fn actor_key(actor: Actor) -> &'static str {
    match actor {
        Actor::Founder => "founder",
        Actor::Generator => "generator",
        Actor::Factory => "factory",
        Actor::Household => "household",
        Actor::Speculator => "speculator",
        Actor::AdaptiveLp => "adaptive-lp",
        Actor::Arbitrageur => "arbitrageur",
        Actor::Whale => "whale",
    }
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

fn scene(_: u64, world: &World) -> Option<Scene> {
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
                id: ViewId::new(
                    slug(format!("amm/actor/{}", actor_key(actor))),
                    actor.studio_label(),
                ),
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
                        slug("energy"),
                        "Energy",
                        &Asset::Energy,
                        world.balance(&account, &Asset::Energy),
                    ),
                    asset_metric(
                        slug("credit"),
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
                id: ViewId::new(slug("amm/pool/energy-credit"), "Energy / Credit AMM"),
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
                slug("price"),
                "Discovered price",
                format_price(state.price_milli),
                Some("credit / energy"),
            ),
            asset_metric(
                slug("energy-reserve"),
                "Energy reserve",
                &Asset::Energy,
                state.energy,
            ),
            asset_metric(
                slug("liquidity"),
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
                slug("day"),
                "Market day",
                amm::period(world) + 1,
                Some("day"),
            ),
        );
        scene.metrics.extend([
            visual_metric(
                slug("unmet"),
                "Unmet daily needs",
                unmet_needs(world),
                Some("needs"),
            ),
            visual_metric(
                slug("defaults"),
                "Missed obligations",
                defaults(world),
                Some("payments"),
            ),
            asset_metric(
                slug("sunlight"),
                "Sunlight",
                &Asset::CurrentSunlight,
                world.balance(&AccountId::World, &Asset::CurrentSunlight),
            ),
            asset_metric(
                slug("capacity"),
                "Generating capacity",
                &Asset::ProductionCapacity,
                world.balance(
                    &AccountId::Actor(Actor::Generator),
                    &Asset::ProductionCapacity,
                ),
            ),
            asset_metric(
                slug("goods"),
                "Goods delivered",
                &Asset::DeliveredGoods,
                world.balance(&AccountId::Treasury, &Asset::DeliveredGoods),
            ),
        ]);
    }
    Some(scene)
}

fn unmet_needs(world: &World) -> u64 {
    [Actor::Factory, Actor::Household]
        .into_iter()
        .map(|actor| {
            world
                .balance(&AccountId::Actor(actor), &Asset::MissedNeed(actor))
                .to_u64()
        })
        .sum()
}

fn defaults(world: &World) -> u64 {
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

fn satisfied_needs(world: &World) -> u64 {
    [Actor::Factory, Actor::Household]
        .into_iter()
        .map(|actor| {
            world
                .balance(&AccountId::Actor(actor), &Asset::SatisfiedNeed(actor))
                .to_u64()
        })
        .sum()
}

fn total_utility(world: &World) -> u64 {
    ACTORS
        .into_iter()
        .map(|actor| {
            world
                .balance(&AccountId::Actor(actor), &Asset::Utility)
                .to_u64()
        })
        .sum()
}

fn attach_contribution_leaderboards(
    initial: &World,
    trace: &axionomy::Trace<amm::Ontology>,
    document: &mut ViewDocument,
) -> Result<(), ServiceError> {
    let mut world = initial.clone();
    let mut contributions = BTreeMap::<Actor, i128>::new();
    document.initial.leaderboards = vec![contribution_board(&contributions)];
    for (index, exchange) in trace.exchanges().iter().enumerate() {
        let before = i128::from(amm::pool_state(&world).price_milli);
        world
            .apply(exchange.clone())
            .map_err(|error| problem_error("amm", format!("{error:?}")))?;
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
                    actor.studio_label(),
                ),
                value: format_signed_milli(value),
                unit: Some("Δ credit / energy".into()),
                eligible: true,
                components: Vec::new(),
            }
        })
        .collect();
    LeaderboardView {
        key: slug("price-contribution"),
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

fn causal_contribution_board(initial: &World) -> LeaderboardView {
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
                    contribution.actor.studio_label(),
                ),
                value: format!("{reduced_numerator}/{reduced_denominator}"),
                unit: Some("Δ credit / energy".into()),
                eligible: true,
                components: vec![visual_metric(
                    slug("coalitions"),
                    "Counterfactual coalitions",
                    1_u64 << amm::PRICE_ACTORS.len(),
                    None,
                )],
            }
        })
        .collect();
    LeaderboardView {
        key: slug("causal-price-contribution"),
        label: "Causal price contribution".into(),
        description: "Exact Shapley allocation across every coalition of active market actors. Each coalition is independently re-quoted and replayed through Axionomy; the reduced fractions sum to the full counterfactual price change.".into(),
        direction: ObjectiveDirectionView::Maximize,
        entries,
    }
}

fn gcd(mut left: u128, mut right: u128) -> u128 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left.max(1)
}

// How Studio names and writes this problem's values.

/// Energy is counted in units and credit in credits, as the market scene
/// reads them; the season is counted in market days. The reference price is
/// kept in thousandths of a credit per unit of energy.
impl StudioFormat for amm::Asset {
    fn studio_format(&self) -> QuantityFormat {
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

impl StudioLabel for amm::Actor {
    fn studio_label(&self) -> String {
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

impl StudioLabel for amm::AccountId {
    fn studio_label(&self) -> String {
        match self {
            Self::Pool => "Energy / credit AMM".into(),
            Self::Treasury => "Closed-economy treasury".into(),
            Self::Information => "Scarcity information".into(),
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
            Self::Need(actor) => format!("{} need due today", actor.studio_label()),
            Self::FutureNeed(actor) => format!("{} needs still to come", actor.studio_label()),
            Self::SatisfiedNeed(actor) => format!("{} needs met", actor.studio_label()),
            Self::MissedNeed(actor) => format!("{} needs unmet", actor.studio_label()),
            Self::Obligation(actor) => format!("{} obligation due today", actor.studio_label()),
            Self::FutureObligation(actor) => {
                format!("{} obligations still to come", actor.studio_label())
            }
            Self::SettledObligation(actor) => {
                format!("{} obligations settled", actor.studio_label())
            }
            Self::DefaultedObligation(actor) => {
                format!("{} obligations missed", actor.studio_label())
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

impl StudioLabel for amm::Role {
    fn studio_label(&self) -> String {
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
            Self::Participant(actor) => actor.studio_label(),
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
