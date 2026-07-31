//! Information-set Monte Carlo tree search over core-validated economies.

use crate::{
    action_source::{ActionSource, collect_actions},
    mcts::MctsConfig,
    sampling::{SamplingError, SeededSampler, TicketSource, WeightedExchange, sample},
};
use axionomy::{ApplyError, Economy, Exchange, QuantityScalar};
use std::collections::HashMap;
use std::hash::Hash;

/// An acting player and the canonical economic observation available to it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct InformationState<Key> {
    actor: usize,
    key: Key,
}

impl<Key> InformationState<Key> {
    pub const fn new(actor: usize, key: Key) -> Self {
        Self { actor, key }
    }

    pub const fn actor(&self) -> usize {
        self.actor
    }

    pub const fn key(&self) -> &Key {
        &self.key
    }
}

#[derive(Debug, Clone)]
pub struct IsmctsChild<Action> {
    action: Action,
    visits: u64,
    availability: u64,
    mean_values: Vec<f64>,
}

impl<Action> IsmctsChild<Action> {
    pub const fn action(&self) -> &Action {
        &self.action
    }

    pub const fn visits(&self) -> u64 {
        self.visits
    }

    /// Number of sampled determinizations in which this action was applicable.
    pub const fn availability(&self) -> u64 {
        self.availability
    }

    pub fn mean_values(&self) -> &[f64] {
        &self.mean_values
    }
}

#[derive(Debug, Clone)]
pub struct IsmctsDecision<Action> {
    action: Action,
    iterations: usize,
    information_sets: usize,
    children: Vec<IsmctsChild<Action>>,
}

impl<Action> IsmctsDecision<Action> {
    pub const fn action(&self) -> &Action {
        &self.action
    }

    pub const fn iterations(&self) -> usize {
        self.iterations
    }

    pub const fn information_sets(&self) -> usize {
        self.information_sets
    }

    pub fn children(&self) -> &[IsmctsChild<Action>] {
        &self.children
    }
}

#[derive(Debug, Clone)]
pub enum IsmctsError<RateId, Role, AccountId, A, N = u64>
where
    N: QuantityScalar,
{
    NoPlayers,
    ZeroIterations,
    ZeroDepth,
    InvalidExploration,
    TerminalRoot,
    NoRootActions,
    NoExploredAction,
    NoDeterminization { iteration: usize },
    InconsistentDeterminization,
    InvalidActor { actor: usize, players: usize },
    InvalidValueDimensions { expected: usize, actual: usize },
    NonFiniteValue { player: usize },
    Sampling(SamplingError),
    Rejected(ApplyError<RateId, Role, AccountId, A, N>),
    SelectedActionRejected(ApplyError<RateId, Role, AccountId, A, N>),
}

pub type IsmctsResult<RateId, Role, AccountId, A, N = u64> = Result<
    IsmctsDecision<Exchange<RateId, Role, AccountId, N>>,
    IsmctsError<RateId, Role, AccountId, A, N>,
>;

/// Random rollout selection that can inspect only the information state and
/// the already filtered concrete actions.
pub fn random_action<Key, Action>(
    _: &InformationState<Key>,
    actions: &[Action],
    random: &mut SeededSampler,
) -> Option<Action>
where
    Action: Clone,
{
    (!actions.is_empty()).then(|| {
        let index = random.ticket(actions.len() as u64) as usize;
        actions[index].clone()
    })
}

/// Searches a shared information tree using one root determinization per
/// iteration.
///
/// `information` must derive its key from an actor-scoped economic view.
/// `determinize` receives only that root information state and must return a
/// possible encoded economy consistent with it. Decision proposals and rollout
/// policies receive information states rather than full economies; only
/// environment chance, terminal, and cutoff projections may inspect a sampled
/// closed world.
///
/// Every emitted decision remains a concrete exchange. Proposals are assessed
/// in each sampled economy, and the selected root exchange is revalidated
/// against `initial` before it is returned.
#[allow(clippy::too_many_arguments)]
pub fn search<
    AccountId,
    A,
    RateId,
    Role,
    N,
    Key,
    Information,
    Determinize,
    Source,
    Chance,
    Terminal,
    Cutoff,
    RolloutPolicy,
>(
    initial: &Economy<AccountId, A, RateId, Role, N>,
    config: MctsConfig,
    players: usize,
    mut information: Information,
    mut determinize: Determinize,
    mut source: Source,
    mut chance: Chance,
    mut terminal: Terminal,
    mut cutoff: Cutoff,
    mut rollout_policy: RolloutPolicy,
) -> IsmctsResult<RateId, Role, AccountId, A, N>
where
    AccountId: Clone + Eq + Hash + Ord,
    A: Clone + Eq + Hash + Ord,
    RateId: Clone + Eq + Hash + Ord,
    Role: Clone + Ord,
    N: QuantityScalar,
    Key: Clone + Eq + Hash,
    Information: FnMut(&Economy<AccountId, A, RateId, Role, N>) -> InformationState<Key>,
    Determinize: FnMut(
        &InformationState<Key>,
        &mut SeededSampler,
    ) -> Option<Economy<AccountId, A, RateId, Role, N>>,
    Source: ActionSource<InformationState<Key>, Exchange<RateId, Role, AccountId, N>>,
    Chance: FnMut(
        &Economy<AccountId, A, RateId, Role, N>,
    ) -> Vec<WeightedExchange<Exchange<RateId, Role, AccountId, N>>>,
    Terminal: FnMut(&Economy<AccountId, A, RateId, Role, N>) -> Option<Vec<f64>>,
    Cutoff: FnMut(&Economy<AccountId, A, RateId, Role, N>) -> Vec<f64>,
    RolloutPolicy: FnMut(
        &InformationState<Key>,
        &[Exchange<RateId, Role, AccountId, N>],
        &mut SeededSampler,
    ) -> Option<Exchange<RateId, Role, AccountId, N>>,
{
    validate_config(config, players)?;
    if terminal(initial).is_some() {
        return Err(IsmctsError::TerminalRoot);
    }

    let root = information(initial);
    validate_actor(root.actor(), players)?;
    if unique_actions(collect_actions(&mut source, &root)).is_empty() {
        return Err(IsmctsError::NoRootActions);
    }

    let mut nodes = vec![Node::new(root.clone(), players)];
    let mut transpositions = HashMap::from([(root.clone(), 0_usize)]);
    let mut random = SeededSampler::new(config.seed());

    for iteration in 0..config.iterations() {
        let mut world =
            determinize(&root, &mut random).ok_or(IsmctsError::NoDeterminization { iteration })?;
        if information(&world) != root {
            return Err(IsmctsError::InconsistentDeterminization);
        }

        let mut depth = 0;
        let mut path = Vec::<(usize, usize)>::new();
        let values = loop {
            if let Some(values) = terminal(&world) {
                break validate_values(values, players)?;
            }
            if depth >= config.max_depth() {
                break validate_values(cutoff(&world), players)?;
            }

            let outcomes = chance(&world);
            if !outcomes.is_empty() {
                let action = sample(&outcomes, &mut random)
                    .map_err(IsmctsError::Sampling)?
                    .clone();
                world.apply(action).map_err(IsmctsError::Rejected)?;
                depth += 1;
                continue;
            }

            let current = information(&world);
            validate_actor(current.actor(), players)?;
            let node_index = node_for(current.clone(), &mut nodes, &mut transpositions, players);
            if path.iter().any(|(visited, _)| *visited == node_index) {
                break validate_values(cutoff(&world), players)?;
            }

            let actions = applicable_actions(&mut source, &current, &world);
            if actions.is_empty() {
                break validate_values(cutoff(&world), players)?;
            }
            let available = register_available(&mut nodes[node_index], actions);
            let unvisited = available
                .iter()
                .copied()
                .filter(|edge| nodes[node_index].edges[*edge].visits == 0)
                .collect::<Vec<_>>();
            let (edge_index, expanded) = if unvisited.is_empty() {
                (
                    select_uct(
                        &nodes[node_index],
                        &available,
                        current.actor(),
                        config.exploration(),
                    ),
                    false,
                )
            } else {
                let selected = random.ticket(unvisited.len() as u64) as usize;
                (unvisited[selected], true)
            };
            let action = nodes[node_index].edges[edge_index].action.clone();
            world.apply(action).map_err(IsmctsError::Rejected)?;
            path.push((node_index, edge_index));
            depth += 1;

            if expanded {
                break simulate(
                    &world,
                    depth,
                    config.max_depth(),
                    players,
                    &mut information,
                    &mut source,
                    &mut chance,
                    &mut terminal,
                    &mut cutoff,
                    &mut rollout_policy,
                    &mut random,
                )?;
            }
        };

        for (node_index, edge_index) in path {
            let node = &mut nodes[node_index];
            node.visits += 1;
            let edge = &mut node.edges[edge_index];
            edge.visits += 1;
            for (total, value) in edge.value_totals.iter_mut().zip(&values) {
                *total += value;
            }
        }
    }

    let root_node = &nodes[0];
    let root_player = root.actor();
    let mut children = root_node
        .edges
        .iter()
        .filter(|edge| edge.visits > 0)
        .map(|edge| IsmctsChild {
            action: edge.action.clone(),
            visits: edge.visits,
            availability: edge.availability,
            mean_values: mean_values(edge),
        })
        .collect::<Vec<_>>();
    children.sort_by(|left, right| {
        right
            .visits
            .cmp(&left.visits)
            .then_with(|| right.mean_values[root_player].total_cmp(&left.mean_values[root_player]))
    });
    let action = children
        .first()
        .ok_or(IsmctsError::NoExploredAction)?
        .action
        .clone();

    let mut validation = initial.fork();
    validation
        .apply(action.clone())
        .map_err(IsmctsError::SelectedActionRejected)?;

    Ok(IsmctsDecision {
        action,
        iterations: config.iterations(),
        information_sets: nodes.len(),
        children,
    })
}

#[allow(clippy::too_many_arguments)]
fn simulate<
    AccountId,
    A,
    RateId,
    Role,
    N,
    Key,
    Information,
    Source,
    Chance,
    Terminal,
    Cutoff,
    RolloutPolicy,
>(
    initial: &Economy<AccountId, A, RateId, Role, N>,
    mut depth: usize,
    max_depth: usize,
    players: usize,
    information: &mut Information,
    source: &mut Source,
    chance: &mut Chance,
    terminal: &mut Terminal,
    cutoff: &mut Cutoff,
    rollout_policy: &mut RolloutPolicy,
    random: &mut SeededSampler,
) -> Result<Vec<f64>, IsmctsError<RateId, Role, AccountId, A, N>>
where
    AccountId: Clone + Eq + Hash + Ord,
    A: Clone + Eq + Hash + Ord,
    RateId: Clone + Eq + Hash + Ord,
    Role: Clone + Ord,
    N: QuantityScalar,
    Key: Clone + Eq + Hash,
    Information: FnMut(&Economy<AccountId, A, RateId, Role, N>) -> InformationState<Key>,
    Source: ActionSource<InformationState<Key>, Exchange<RateId, Role, AccountId, N>>,
    Chance: FnMut(
        &Economy<AccountId, A, RateId, Role, N>,
    ) -> Vec<WeightedExchange<Exchange<RateId, Role, AccountId, N>>>,
    Terminal: FnMut(&Economy<AccountId, A, RateId, Role, N>) -> Option<Vec<f64>>,
    Cutoff: FnMut(&Economy<AccountId, A, RateId, Role, N>) -> Vec<f64>,
    RolloutPolicy: FnMut(
        &InformationState<Key>,
        &[Exchange<RateId, Role, AccountId, N>],
        &mut SeededSampler,
    ) -> Option<Exchange<RateId, Role, AccountId, N>>,
{
    let mut world = initial.fork();
    loop {
        if let Some(values) = terminal(&world) {
            return validate_values(values, players);
        }
        if depth >= max_depth {
            return validate_values(cutoff(&world), players);
        }

        let outcomes = chance(&world);
        let action = if outcomes.is_empty() {
            let current = information(&world);
            validate_actor(current.actor(), players)?;
            let actions = applicable_actions(source, &current, &world);
            if actions.is_empty() {
                return validate_values(cutoff(&world), players);
            }
            let Some(action) = rollout_policy(&current, &actions, random) else {
                return validate_values(cutoff(&world), players);
            };
            action
        } else {
            sample(&outcomes, random)
                .map_err(IsmctsError::Sampling)?
                .clone()
        };
        world.apply(action).map_err(IsmctsError::Rejected)?;
        depth += 1;
    }
}

fn applicable_actions<AccountId, A, RateId, Role, N, Key>(
    source: &mut impl ActionSource<InformationState<Key>, Exchange<RateId, Role, AccountId, N>>,
    information: &InformationState<Key>,
    world: &Economy<AccountId, A, RateId, Role, N>,
) -> Vec<Exchange<RateId, Role, AccountId, N>>
where
    AccountId: Clone + Eq + Hash + Ord,
    A: Clone + Eq + Hash + Ord,
    RateId: Clone + Eq + Hash + Ord,
    Role: Clone + Ord,
    N: QuantityScalar,
{
    unique_actions(
        collect_actions(source, information)
            .into_iter()
            .filter(|action| world.is_applicable(action))
            .collect(),
    )
}

fn unique_actions<Action: PartialEq>(actions: Vec<Action>) -> Vec<Action> {
    let mut unique = Vec::new();
    for action in actions {
        if !unique.contains(&action) {
            unique.push(action);
        }
    }
    unique
}

fn node_for<Key, Action>(
    information: InformationState<Key>,
    nodes: &mut Vec<Node<Key, Action>>,
    transpositions: &mut HashMap<InformationState<Key>, usize>,
    players: usize,
) -> usize
where
    Key: Clone + Eq + Hash,
{
    if let Some(index) = transpositions.get(&information) {
        *index
    } else {
        let index = nodes.len();
        nodes.push(Node::new(information.clone(), players));
        transpositions.insert(information, index);
        index
    }
}

fn register_available<Key, Action>(node: &mut Node<Key, Action>, actions: Vec<Action>) -> Vec<usize>
where
    Action: Clone + PartialEq,
{
    let mut available = Vec::with_capacity(actions.len());
    for action in actions {
        let edge_index =
            if let Some(index) = node.edges.iter().position(|edge| edge.action == action) {
                index
            } else {
                let index = node.edges.len();
                node.edges.push(Edge::new(action, node.value_dimensions));
                index
            };
        node.edges[edge_index].availability += 1;
        available.push(edge_index);
    }
    available
}

fn select_uct<Key, Action>(
    node: &Node<Key, Action>,
    available: &[usize],
    player: usize,
    exploration: f64,
) -> usize {
    available
        .iter()
        .copied()
        .max_by(|left, right| {
            let left_score = uct_score(&node.edges[*left], player, exploration);
            let right_score = uct_score(&node.edges[*right], player, exploration);
            left_score.total_cmp(&right_score)
        })
        .expect("an information set with available actions can select one")
}

fn uct_score<Action>(edge: &Edge<Action>, player: usize, exploration: f64) -> f64 {
    if edge.visits == 0 {
        return f64::INFINITY;
    }
    let exploitation = edge.value_totals[player] / edge.visits as f64;
    let available = edge.availability.max(1) as f64;
    exploitation + exploration * (available.ln() / edge.visits as f64).sqrt()
}

fn mean_values<Action>(edge: &Edge<Action>) -> Vec<f64> {
    if edge.visits == 0 {
        return vec![0.0; edge.value_totals.len()];
    }
    edge.value_totals
        .iter()
        .map(|total| total / edge.visits as f64)
        .collect()
}

fn validate_config<RateId, Role, AccountId, A, N>(
    config: MctsConfig,
    players: usize,
) -> Result<(), IsmctsError<RateId, Role, AccountId, A, N>>
where
    N: QuantityScalar,
{
    if players == 0 {
        return Err(IsmctsError::NoPlayers);
    }
    if config.iterations() == 0 {
        return Err(IsmctsError::ZeroIterations);
    }
    if config.max_depth() == 0 {
        return Err(IsmctsError::ZeroDepth);
    }
    if !config.exploration().is_finite() || config.exploration() < 0.0 {
        return Err(IsmctsError::InvalidExploration);
    }
    Ok(())
}

fn validate_actor<RateId, Role, AccountId, A, N>(
    actor: usize,
    players: usize,
) -> Result<(), IsmctsError<RateId, Role, AccountId, A, N>>
where
    N: QuantityScalar,
{
    if actor >= players {
        Err(IsmctsError::InvalidActor { actor, players })
    } else {
        Ok(())
    }
}

fn validate_values<RateId, Role, AccountId, A, N>(
    values: Vec<f64>,
    players: usize,
) -> Result<Vec<f64>, IsmctsError<RateId, Role, AccountId, A, N>>
where
    N: QuantityScalar,
{
    if values.len() != players {
        return Err(IsmctsError::InvalidValueDimensions {
            expected: players,
            actual: values.len(),
        });
    }
    if let Some(player) = values.iter().position(|value| !value.is_finite()) {
        return Err(IsmctsError::NonFiniteValue { player });
    }
    Ok(values)
}

struct Node<Key, Action> {
    #[allow(dead_code)]
    information: InformationState<Key>,
    edges: Vec<Edge<Action>>,
    visits: u64,
    value_dimensions: usize,
}

impl<Key, Action> Node<Key, Action> {
    fn new(information: InformationState<Key>, players: usize) -> Self {
        Self {
            information,
            edges: Vec::new(),
            visits: 0,
            value_dimensions: players,
        }
    }
}

struct Edge<Action> {
    action: Action,
    visits: u64,
    availability: u64,
    value_totals: Vec<f64>,
}

impl<Action> Edge<Action> {
    fn new(action: Action, players: usize) -> Self {
        Self {
            action,
            visits: 0,
            availability: 0,
            value_totals: vec![0.0; players],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action_source::lazy_actions;
    use axionomy::{Account, EconomyBuilder, ObservationKey, Quantity, Rate, basket};

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
    enum AccountId {
        Agent,
        Nature,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
    enum Asset {
        Ready,
        AwaitingSignal,
        Decision,
        SignalHeads,
        SignalTails,
        GuessHeads,
        GuessTails,
        TruthHeads,
        TruthTails,
        Won,
        Lost,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
    enum RateId {
        Inspect,
        RevealHeads,
        RevealTails,
        DirectHeads,
        DirectTails,
        InformedHeads,
        InformedTails,
        HeadsCorrect,
        HeadsWrong,
        TailsCorrect,
        TailsWrong,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
    enum Role {
        Agent,
        Nature,
    }

    type World = Economy<AccountId, Asset, RateId, Role>;
    type Action = Exchange<RateId, Role, AccountId>;
    type Observation = ObservationKey<AccountId, Asset>;

    fn hidden_world(heads: bool) -> World {
        let truth = if heads {
            Asset::TruthHeads
        } else {
            Asset::TruthTails
        };
        EconomyBuilder::new()
            .account(AccountId::Agent, Account::from(basket([(Asset::Ready, 1)])))
            .account(AccountId::Nature, Account::from(basket([(truth, 1)])))
            .rate(
                RateId::Inspect,
                Rate::new()
                    .consume(Role::Agent, basket([(Asset::Ready, 1)]))
                    .produce(Role::Agent, basket([(Asset::AwaitingSignal, 1)])),
            )
            .rate(
                RateId::RevealHeads,
                Rate::new()
                    .consume(Role::Agent, basket([(Asset::AwaitingSignal, 1)]))
                    .produce(
                        Role::Agent,
                        basket([(Asset::Decision, 1), (Asset::SignalHeads, 1)]),
                    )
                    .preserve(Role::Nature, basket([(Asset::TruthHeads, 1)]))
                    .distinct(Role::Agent, Role::Nature),
            )
            .rate(
                RateId::RevealTails,
                Rate::new()
                    .consume(Role::Agent, basket([(Asset::AwaitingSignal, 1)]))
                    .produce(
                        Role::Agent,
                        basket([(Asset::Decision, 1), (Asset::SignalTails, 1)]),
                    )
                    .preserve(Role::Nature, basket([(Asset::TruthTails, 1)]))
                    .distinct(Role::Agent, Role::Nature),
            )
            .rate(
                RateId::DirectHeads,
                Rate::new()
                    .consume(Role::Agent, basket([(Asset::Ready, 1)]))
                    .produce(Role::Agent, basket([(Asset::GuessHeads, 1)])),
            )
            .rate(
                RateId::DirectTails,
                Rate::new()
                    .consume(Role::Agent, basket([(Asset::Ready, 1)]))
                    .produce(Role::Agent, basket([(Asset::GuessTails, 1)])),
            )
            .rate(
                RateId::InformedHeads,
                Rate::new()
                    .consume(Role::Agent, basket([(Asset::Decision, 1)]))
                    .produce(Role::Agent, basket([(Asset::GuessHeads, 1)])),
            )
            .rate(
                RateId::InformedTails,
                Rate::new()
                    .consume(Role::Agent, basket([(Asset::Decision, 1)]))
                    .produce(Role::Agent, basket([(Asset::GuessTails, 1)])),
            )
            .rate(
                RateId::HeadsCorrect,
                resolution(Asset::GuessHeads, Asset::TruthHeads, Asset::Won),
            )
            .rate(
                RateId::HeadsWrong,
                resolution(Asset::GuessHeads, Asset::TruthTails, Asset::Lost),
            )
            .rate(
                RateId::TailsCorrect,
                resolution(Asset::GuessTails, Asset::TruthTails, Asset::Won),
            )
            .rate(
                RateId::TailsWrong,
                resolution(Asset::GuessTails, Asset::TruthHeads, Asset::Lost),
            )
            .build()
            .expect("test model is valid")
    }

    fn resolution(guess: Asset, truth: Asset, result: Asset) -> Rate<Role, Asset> {
        Rate::new()
            .consume(Role::Agent, basket([(guess, 1)]))
            .produce(Role::Agent, basket([(result, 1)]))
            .preserve(Role::Nature, basket([(truth, 1)]))
            .distinct(Role::Agent, Role::Nature)
    }

    fn action(rate: RateId) -> Action {
        let exchange = Exchange::new(rate, Quantity::new(1)).bind(Role::Agent, AccountId::Agent);
        if matches!(
            rate,
            RateId::RevealHeads
                | RateId::RevealTails
                | RateId::HeadsCorrect
                | RateId::HeadsWrong
                | RateId::TailsCorrect
                | RateId::TailsWrong
        ) {
            exchange.bind(Role::Nature, AccountId::Nature)
        } else {
            exchange
        }
    }

    fn information(world: &World) -> InformationState<Observation> {
        InformationState::new(0, world.view([AccountId::Agent]).observation_key())
    }

    fn has(information: &InformationState<Observation>, asset: Asset) -> bool {
        information
            .key()
            .balances()
            .iter()
            .any(|(account, present, _)| account == &AccountId::Agent && present == &asset)
    }

    fn source() -> impl ActionSource<InformationState<Observation>, Action> {
        lazy_actions(
            |information: &InformationState<Observation>, emit: &mut dyn FnMut(Action)| {
                if has(information, Asset::Ready) {
                    emit(action(RateId::Inspect));
                    emit(action(RateId::DirectHeads));
                    emit(action(RateId::DirectTails));
                } else if has(information, Asset::Decision) {
                    emit(action(RateId::InformedHeads));
                    emit(action(RateId::InformedTails));
                }
            },
        )
    }

    fn chance(world: &World) -> Vec<WeightedExchange<Action>> {
        [
            RateId::RevealHeads,
            RateId::RevealTails,
            RateId::HeadsCorrect,
            RateId::HeadsWrong,
            RateId::TailsCorrect,
            RateId::TailsWrong,
        ]
        .into_iter()
        .filter_map(|rate| {
            let action = action(rate);
            world
                .is_applicable(&action)
                .then(|| WeightedExchange::new(action, 1))
        })
        .collect()
    }

    fn terminal(world: &World) -> Option<Vec<f64>> {
        if !world.balance(&AccountId::Agent, &Asset::Won).is_zero() {
            Some(vec![1.0])
        } else if !world.balance(&AccountId::Agent, &Asset::Lost).is_zero() {
            Some(vec![0.0])
        } else {
            None
        }
    }

    fn decide(actual: &World) -> IsmctsDecision<Action> {
        search(
            actual,
            MctsConfig::new(2_000, 8).with_seed(19),
            1,
            information,
            |_, random| Some(hidden_world(random.ticket(2) == 0)),
            source(),
            chance,
            terminal,
            |_| vec![0.0],
            random_action,
        )
        .expect("the hidden coin has a safe information-set decision")
    }

    #[test]
    fn equal_observations_produce_the_same_information_set_decision() {
        let heads = hidden_world(true);
        let tails = hidden_world(false);

        assert_ne!(heads.state_key(), tails.state_key());
        assert_eq!(information(&heads), information(&tails));
        assert!(
            information(&heads)
                .key()
                .balances()
                .iter()
                .all(|(_, asset, _)| { !matches!(asset, Asset::TruthHeads | Asset::TruthTails) })
        );

        let heads_decision = decide(&heads);
        let tails_decision = decide(&tails);

        assert_eq!(heads_decision.action().rate(), &RateId::Inspect);
        assert_eq!(heads_decision.action(), tails_decision.action());
        assert!(heads.is_applicable(heads_decision.action()));
        assert!(tails.is_applicable(tails_decision.action()));
        assert!(heads_decision.information_sets() >= 3);
    }

    #[test]
    fn encoded_revelation_separates_information_sets() {
        let mut heads = hidden_world(true);
        let mut tails = hidden_world(false);
        heads
            .apply(action(RateId::Inspect))
            .expect("inspect is applicable");
        tails
            .apply(action(RateId::Inspect))
            .expect("inspect is applicable");
        heads
            .apply(action(RateId::RevealHeads))
            .expect("heads truth enables heads signal");
        tails
            .apply(action(RateId::RevealTails))
            .expect("tails truth enables tails signal");

        assert_ne!(information(&heads), information(&tails));
    }

    #[test]
    fn inconsistent_belief_samples_are_rejected() {
        let actual = hidden_world(true);
        let result = search(
            &actual,
            MctsConfig::new(1, 4),
            1,
            information,
            |_, _| {
                let mut progressed = hidden_world(false);
                progressed
                    .apply(action(RateId::Inspect))
                    .expect("inspect progresses the sample");
                Some(progressed)
            },
            source(),
            chance,
            terminal,
            |_| vec![0.0],
            random_action,
        );

        assert!(matches!(
            result,
            Err(IsmctsError::InconsistentDeterminization)
        ));
    }
}
