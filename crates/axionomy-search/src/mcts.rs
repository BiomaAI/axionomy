//! Monte Carlo tree search over core-validated economy branches.

use crate::{
    action_source::{ActionSource, collect_actions, eager_actions},
    sampling::{SamplingError, SeededSampler, TicketSource, WeightedExchange, sample},
};
use axionomy::{ApplyError, Economy, Exchange, Quantity, QuantityScalar};
use std::collections::HashMap;
use std::hash::Hash;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MctsConfig {
    iterations: usize,
    max_depth: usize,
    exploration: f64,
    seed: u64,
}

impl MctsConfig {
    pub const fn new(iterations: usize, max_depth: usize) -> Self {
        Self {
            iterations,
            max_depth,
            exploration: std::f64::consts::SQRT_2,
            seed: 0,
        }
    }

    pub const fn with_exploration(mut self, exploration: f64) -> Self {
        self.exploration = exploration;
        self
    }

    pub const fn with_seed(mut self, seed: u64) -> Self {
        self.seed = seed;
        self
    }

    pub const fn iterations(self) -> usize {
        self.iterations
    }

    pub const fn max_depth(self) -> usize {
        self.max_depth
    }

    pub const fn exploration(self) -> f64 {
        self.exploration
    }

    pub const fn seed(self) -> u64 {
        self.seed
    }
}

#[derive(Debug, Clone)]
pub struct MctsChild<Action> {
    action: Action,
    visits: u64,
    mean_values: Vec<f64>,
}

impl<Action> MctsChild<Action> {
    pub const fn action(&self) -> &Action {
        &self.action
    }

    pub const fn visits(&self) -> u64 {
        self.visits
    }

    pub fn mean_values(&self) -> &[f64] {
        &self.mean_values
    }
}

#[derive(Debug, Clone)]
pub struct MctsDecision<Action> {
    action: Action,
    iterations: usize,
    nodes: usize,
    children: Vec<MctsChild<Action>>,
}

impl<Action> MctsDecision<Action> {
    pub const fn action(&self) -> &Action {
        &self.action
    }

    pub const fn iterations(&self) -> usize {
        self.iterations
    }

    pub const fn nodes(&self) -> usize {
        self.nodes
    }

    pub fn children(&self) -> &[MctsChild<Action>] {
        &self.children
    }
}

#[derive(Debug, Clone)]
pub enum MctsError<RateId, Role, AccountId, A, N = u64>
where
    N: QuantityScalar,
{
    NoPlayers,
    ZeroIterations,
    ZeroDepth,
    InvalidExploration,
    TerminalRoot,
    ChanceRoot,
    NoRootActions,
    NoExploredAction,
    InvalidActor { actor: usize, players: usize },
    InvalidValueDimensions { expected: usize, actual: usize },
    NonFiniteValue { player: usize },
    Sampling(SamplingError),
    Rejected(ApplyError<RateId, Role, AccountId, A, N>),
}

pub type MctsResult<RateId, Role, AccountId, A, N = u64> = Result<
    MctsDecision<Exchange<RateId, Role, AccountId, N>>,
    MctsError<RateId, Role, AccountId, A, N>,
>;

type Transpositions<AccountId, A, N> = HashMap<Vec<(AccountId, A, Quantity<N>)>, usize>;
type ExpansionResult<RateId, Role, AccountId, A, N> =
    Result<(usize, bool), MctsError<RateId, Role, AccountId, A, N>>;

/// Random rollout action selection with deterministic seeded exploration.
pub fn random_action<World, Action>(
    _: &World,
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

/// Runs vector-valued MCTS over one closed economy.
///
/// `terminal` and `cutoff` must project values from encoded state.
/// `chance` must derive weighted Nature exchanges from encoded weights.
#[allow(clippy::too_many_arguments)]
pub fn search<
    AccountId,
    A,
    RateId,
    Role,
    N,
    Candidates,
    Chance,
    Actor,
    Terminal,
    Cutoff,
    RolloutPolicy,
>(
    initial: &Economy<AccountId, A, RateId, Role, N>,
    config: MctsConfig,
    players: usize,
    candidates: Candidates,
    chance: Chance,
    actor: Actor,
    terminal: Terminal,
    cutoff: Cutoff,
    rollout_policy: RolloutPolicy,
) -> MctsResult<RateId, Role, AccountId, A, N>
where
    AccountId: Clone + Eq + Hash + Ord,
    A: Clone + Eq + Hash + Ord,
    RateId: Clone + Eq + Hash + Ord,
    Role: Clone + Ord,
    N: QuantityScalar,
    Candidates:
        FnMut(&Economy<AccountId, A, RateId, Role, N>) -> Vec<Exchange<RateId, Role, AccountId, N>>,
    Chance: FnMut(
        &Economy<AccountId, A, RateId, Role, N>,
    ) -> Vec<WeightedExchange<Exchange<RateId, Role, AccountId, N>>>,
    Actor: FnMut(&Economy<AccountId, A, RateId, Role, N>) -> usize,
    Terminal: FnMut(&Economy<AccountId, A, RateId, Role, N>) -> Option<Vec<f64>>,
    Cutoff: FnMut(&Economy<AccountId, A, RateId, Role, N>) -> Vec<f64>,
    RolloutPolicy: FnMut(
        &Economy<AccountId, A, RateId, Role, N>,
        &[Exchange<RateId, Role, AccountId, N>],
        &mut SeededSampler,
    ) -> Option<Exchange<RateId, Role, AccountId, N>>,
{
    search_with_source(
        initial,
        config,
        players,
        eager_actions(candidates),
        chance,
        actor,
        terminal,
        cutoff,
        rollout_policy,
    )
}

/// Runs vector-valued MCTS with visitor-based concrete action generation.
///
/// Emitted proposals remain non-authoritative: this function retains only
/// exchanges applicable to each simulated economy before traversing them.
#[allow(clippy::too_many_arguments)]
pub fn search_with_source<
    AccountId,
    A,
    RateId,
    Role,
    N,
    Source,
    Chance,
    Actor,
    Terminal,
    Cutoff,
    RolloutPolicy,
>(
    initial: &Economy<AccountId, A, RateId, Role, N>,
    config: MctsConfig,
    players: usize,
    mut source: Source,
    mut chance: Chance,
    mut actor: Actor,
    mut terminal: Terminal,
    mut cutoff: Cutoff,
    mut rollout_policy: RolloutPolicy,
) -> MctsResult<RateId, Role, AccountId, A, N>
where
    AccountId: Clone + Eq + Hash + Ord,
    A: Clone + Eq + Hash + Ord,
    RateId: Clone + Eq + Hash + Ord,
    Role: Clone + Ord,
    N: QuantityScalar,
    Source:
        ActionSource<Economy<AccountId, A, RateId, Role, N>, Exchange<RateId, Role, AccountId, N>>,
    Chance: FnMut(
        &Economy<AccountId, A, RateId, Role, N>,
    ) -> Vec<WeightedExchange<Exchange<RateId, Role, AccountId, N>>>,
    Actor: FnMut(&Economy<AccountId, A, RateId, Role, N>) -> usize,
    Terminal: FnMut(&Economy<AccountId, A, RateId, Role, N>) -> Option<Vec<f64>>,
    Cutoff: FnMut(&Economy<AccountId, A, RateId, Role, N>) -> Vec<f64>,
    RolloutPolicy: FnMut(
        &Economy<AccountId, A, RateId, Role, N>,
        &[Exchange<RateId, Role, AccountId, N>],
        &mut SeededSampler,
    ) -> Option<Exchange<RateId, Role, AccountId, N>>,
{
    validate_config(config, players)?;
    if terminal(initial).is_some() {
        return Err(MctsError::TerminalRoot);
    }
    if !chance(initial).is_empty() {
        return Err(MctsError::ChanceRoot);
    }
    let root_actions = applicable_actions(&mut source, initial);
    if root_actions.is_empty() {
        return Err(MctsError::NoRootActions);
    }
    validate_actor(actor(initial), players)?;

    let mut nodes = vec![Node::new(initial.fork(), players)];
    let mut transpositions = HashMap::from([(initial.state_key(), 0_usize)]);
    let mut random = SeededSampler::new(config.seed());

    for _ in 0..config.iterations() {
        let mut node_index = 0;
        let mut path = vec![0_usize];
        let mut depth = 0;

        let values = loop {
            if let Some(values) = terminal(&nodes[node_index].world) {
                break validate_values(values, players)?;
            }
            if depth >= config.max_depth() {
                break validate_values(cutoff(&nodes[node_index].world), players)?;
            }

            let outcomes = chance(&nodes[node_index].world);
            if !outcomes.is_empty() {
                let action = sample(&outcomes, &mut random)
                    .map_err(MctsError::Sampling)?
                    .clone();
                let (child, expanded) =
                    descend_or_expand(&mut nodes, &mut transpositions, node_index, action)?;
                depth += 1;
                if path.contains(&child) {
                    break validate_values(cutoff(&nodes[child].world), players)?;
                }
                path.push(child);
                node_index = child;
                if expanded {
                    break simulate(
                        &nodes[node_index].world,
                        depth,
                        config.max_depth(),
                        players,
                        &mut source,
                        &mut chance,
                        &mut terminal,
                        &mut cutoff,
                        &mut rollout_policy,
                        &mut random,
                    )?;
                }
                continue;
            }

            let actions = applicable_actions(&mut source, &nodes[node_index].world);
            if actions.is_empty() {
                break validate_values(cutoff(&nodes[node_index].world), players)?;
            }
            let unexpanded = actions
                .into_iter()
                .filter(|action| {
                    !nodes[node_index]
                        .children
                        .iter()
                        .any(|edge| edge.action == *action)
                })
                .collect::<Vec<_>>();
            if !unexpanded.is_empty() {
                let selected = random.ticket(unexpanded.len() as u64) as usize;
                let (child, _) = descend_or_expand(
                    &mut nodes,
                    &mut transpositions,
                    node_index,
                    unexpanded[selected].clone(),
                )?;
                depth += 1;
                path.push(child);
                node_index = child;
                break simulate(
                    &nodes[node_index].world,
                    depth,
                    config.max_depth(),
                    players,
                    &mut source,
                    &mut chance,
                    &mut terminal,
                    &mut cutoff,
                    &mut rollout_policy,
                    &mut random,
                )?;
            }

            let player = actor(&nodes[node_index].world);
            validate_actor(player, players)?;
            let child = select_uct(&nodes, node_index, player, config.exploration());
            depth += 1;
            if path.contains(&child) {
                break validate_values(cutoff(&nodes[child].world), players)?;
            }
            path.push(child);
            node_index = child;
        };

        for index in path {
            let node = &mut nodes[index];
            node.visits += 1;
            for (total, value) in node.value_totals.iter_mut().zip(&values) {
                *total += value;
            }
        }
    }

    let root_player = actor(initial);
    validate_actor(root_player, players)?;
    let mut children = nodes[0]
        .children
        .iter()
        .map(|edge| {
            let child = &nodes[edge.child];
            MctsChild {
                action: edge.action.clone(),
                visits: child.visits,
                mean_values: mean_values(child),
            }
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
        .ok_or(MctsError::NoExploredAction)?
        .action
        .clone();

    Ok(MctsDecision {
        action,
        iterations: config.iterations(),
        nodes: nodes.len(),
        children,
    })
}

#[allow(clippy::too_many_arguments)]
fn simulate<AccountId, A, RateId, Role, N, Source, Chance, Terminal, Cutoff, RolloutPolicy>(
    initial: &Economy<AccountId, A, RateId, Role, N>,
    mut depth: usize,
    max_depth: usize,
    players: usize,
    source: &mut Source,
    chance: &mut Chance,
    terminal: &mut Terminal,
    cutoff: &mut Cutoff,
    rollout_policy: &mut RolloutPolicy,
    random: &mut SeededSampler,
) -> Result<Vec<f64>, MctsError<RateId, Role, AccountId, A, N>>
where
    AccountId: Clone + Eq + Hash + Ord,
    A: Clone + Eq + Hash + Ord,
    RateId: Clone + Eq + Hash + Ord,
    Role: Clone + Ord,
    N: QuantityScalar,
    Source:
        ActionSource<Economy<AccountId, A, RateId, Role, N>, Exchange<RateId, Role, AccountId, N>>,
    Chance: FnMut(
        &Economy<AccountId, A, RateId, Role, N>,
    ) -> Vec<WeightedExchange<Exchange<RateId, Role, AccountId, N>>>,
    Terminal: FnMut(&Economy<AccountId, A, RateId, Role, N>) -> Option<Vec<f64>>,
    Cutoff: FnMut(&Economy<AccountId, A, RateId, Role, N>) -> Vec<f64>,
    RolloutPolicy: FnMut(
        &Economy<AccountId, A, RateId, Role, N>,
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
            let actions = applicable_actions(source, &world);
            if actions.is_empty() {
                return validate_values(cutoff(&world), players);
            }
            let Some(action) = rollout_policy(&world, &actions, random) else {
                return validate_values(cutoff(&world), players);
            };
            action
        } else {
            sample(&outcomes, random)
                .map_err(MctsError::Sampling)?
                .clone()
        };
        world.apply(action).map_err(MctsError::Rejected)?;
        depth += 1;
    }
}

fn applicable_actions<AccountId, A, RateId, Role, N>(
    source: &mut impl ActionSource<
        Economy<AccountId, A, RateId, Role, N>,
        Exchange<RateId, Role, AccountId, N>,
    >,
    world: &Economy<AccountId, A, RateId, Role, N>,
) -> Vec<Exchange<RateId, Role, AccountId, N>>
where
    AccountId: Clone + Eq + Hash + Ord,
    A: Clone + Eq + Hash + Ord,
    RateId: Clone + Eq + Hash + Ord,
    Role: Clone + Ord,
    N: QuantityScalar,
{
    let mut actions = Vec::new();
    for action in collect_actions(source, world) {
        if world.is_applicable(&action) && !actions.contains(&action) {
            actions.push(action);
        }
    }
    actions
}

fn descend_or_expand<AccountId, A, RateId, Role, N>(
    nodes: &mut Vec<Node<AccountId, A, RateId, Role, N>>,
    transpositions: &mut Transpositions<AccountId, A, N>,
    parent: usize,
    action: Exchange<RateId, Role, AccountId, N>,
) -> ExpansionResult<RateId, Role, AccountId, A, N>
where
    AccountId: Clone + Eq + Hash + Ord,
    A: Clone + Eq + Hash + Ord,
    RateId: Clone + Eq + Hash + Ord,
    Role: Clone + Ord,
    N: QuantityScalar,
{
    if let Some(edge) = nodes[parent]
        .children
        .iter()
        .find(|edge| edge.action == action)
    {
        return Ok((edge.child, false));
    }

    let mut next = nodes[parent].world.fork();
    next.apply(action.clone()).map_err(MctsError::Rejected)?;
    let key = next.state_key();
    let (child, expanded) = if let Some(child) = transpositions.get(&key) {
        (*child, false)
    } else {
        let child = nodes.len();
        let players = nodes[parent].value_totals.len();
        nodes.push(Node::new(next, players));
        transpositions.insert(key, child);
        (child, true)
    };
    nodes[parent].children.push(Edge { action, child });
    Ok((child, expanded))
}

fn select_uct<AccountId, A, RateId, Role, N>(
    nodes: &[Node<AccountId, A, RateId, Role, N>],
    parent: usize,
    player: usize,
    exploration: f64,
) -> usize {
    let parent_visits = nodes[parent].visits.max(1) as f64;
    nodes[parent]
        .children
        .iter()
        .max_by(|left, right| {
            let left_score = uct_score(&nodes[left.child], player, parent_visits, exploration);
            let right_score = uct_score(&nodes[right.child], player, parent_visits, exploration);
            left_score.total_cmp(&right_score)
        })
        .expect("fully expanded decision nodes have children")
        .child
}

fn uct_score<AccountId, A, RateId, Role, N>(
    child: &Node<AccountId, A, RateId, Role, N>,
    player: usize,
    parent_visits: f64,
    exploration: f64,
) -> f64 {
    if child.visits == 0 {
        return f64::INFINITY;
    }
    let exploitation = child.value_totals[player] / child.visits as f64;
    exploitation + exploration * (parent_visits.ln() / child.visits as f64).sqrt()
}

fn mean_values<AccountId, A, RateId, Role, N>(
    node: &Node<AccountId, A, RateId, Role, N>,
) -> Vec<f64> {
    if node.visits == 0 {
        return vec![0.0; node.value_totals.len()];
    }
    node.value_totals
        .iter()
        .map(|total| total / node.visits as f64)
        .collect()
}

fn validate_config<RateId, Role, AccountId, A, N>(
    config: MctsConfig,
    players: usize,
) -> Result<(), MctsError<RateId, Role, AccountId, A, N>>
where
    N: QuantityScalar,
{
    if players == 0 {
        return Err(MctsError::NoPlayers);
    }
    if config.iterations() == 0 {
        return Err(MctsError::ZeroIterations);
    }
    if config.max_depth() == 0 {
        return Err(MctsError::ZeroDepth);
    }
    if !config.exploration().is_finite() || config.exploration() < 0.0 {
        return Err(MctsError::InvalidExploration);
    }
    Ok(())
}

fn validate_actor<RateId, Role, AccountId, A, N>(
    actor: usize,
    players: usize,
) -> Result<(), MctsError<RateId, Role, AccountId, A, N>>
where
    N: QuantityScalar,
{
    if actor >= players {
        Err(MctsError::InvalidActor { actor, players })
    } else {
        Ok(())
    }
}

fn validate_values<RateId, Role, AccountId, A, N>(
    values: Vec<f64>,
    players: usize,
) -> Result<Vec<f64>, MctsError<RateId, Role, AccountId, A, N>>
where
    N: QuantityScalar,
{
    if values.len() != players {
        return Err(MctsError::InvalidValueDimensions {
            expected: players,
            actual: values.len(),
        });
    }
    if let Some(player) = values.iter().position(|value| !value.is_finite()) {
        return Err(MctsError::NonFiniteValue { player });
    }
    Ok(values)
}

struct Node<AccountId, A, RateId, Role, N> {
    world: Economy<AccountId, A, RateId, Role, N>,
    children: Vec<Edge<RateId, Role, AccountId, N>>,
    visits: u64,
    value_totals: Vec<f64>,
}

impl<AccountId, A, RateId, Role, N> Node<AccountId, A, RateId, Role, N> {
    fn new(world: Economy<AccountId, A, RateId, Role, N>, players: usize) -> Self {
        Self {
            world,
            children: Vec::new(),
            visits: 0,
            value_totals: vec![0.0; players],
        }
    }
}

struct Edge<RateId, Role, AccountId, N> {
    action: Exchange<RateId, Role, AccountId, N>,
    child: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action_source::lazy_actions;
    use axionomy::{Account, EconomyBuilder, Rate, basket};

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
    enum AccountId {
        Game,
        Nature,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
    enum Asset {
        Ready,
        Won,
        Lost,
        Pending,
        SafeResult,
        GoodResult,
        BadResult,
        GoodWeight,
        BadWeight,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
    enum RateId {
        Win,
        Lose,
        Safe,
        Gamble,
        Good,
        Bad,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
    enum Role {
        Game,
        Nature,
    }

    type World = Economy<AccountId, Asset, RateId, Role>;
    type Action = Exchange<RateId, Role, AccountId>;

    fn game() -> World {
        EconomyBuilder::new()
            .account(AccountId::Game, Account::from(basket([(Asset::Ready, 1)])))
            .rate(
                RateId::Win,
                Rate::new()
                    .consume(Role::Game, basket([(Asset::Ready, 1)]))
                    .produce(Role::Game, basket([(Asset::Won, 1)])),
            )
            .rate(
                RateId::Lose,
                Rate::new()
                    .consume(Role::Game, basket([(Asset::Ready, 1)]))
                    .produce(Role::Game, basket([(Asset::Lost, 1)])),
            )
            .build()
            .expect("test model is valid")
    }

    fn action(rate: RateId) -> Action {
        let exchange = Exchange::new(rate, Quantity::new(1)).bind(Role::Game, AccountId::Game);
        if matches!(rate, RateId::Good | RateId::Bad) {
            exchange.bind(Role::Nature, AccountId::Nature)
        } else {
            exchange
        }
    }

    #[test]
    fn selects_the_core_encoded_winning_exchange() {
        let decision = search(
            &game(),
            MctsConfig::new(64, 4).with_seed(7),
            1,
            |world| world.applicable([action(RateId::Win), action(RateId::Lose)]),
            |_| Vec::new(),
            |_| 0,
            |world| {
                if !world.balance(&AccountId::Game, &Asset::Won).is_zero() {
                    Some(vec![1.0])
                } else if !world.balance(&AccountId::Game, &Asset::Lost).is_zero() {
                    Some(vec![0.0])
                } else {
                    None
                }
            },
            |_| vec![0.0],
            random_action,
        )
        .expect("the game has a decision");

        assert_eq!(decision.action().rate(), &RateId::Win);
        assert_eq!(decision.iterations(), 64);
        assert_eq!(decision.children().len(), 2);
    }

    #[test]
    fn lazy_proposals_are_core_filtered_before_traversal() {
        let decision = search_with_source(
            &game(),
            MctsConfig::new(32, 3).with_seed(5),
            1,
            lazy_actions(
                |_: &World, emit: &mut dyn FnMut(Exchange<RateId, Role, AccountId>)| {
                    emit(action(RateId::Win));
                    emit(Exchange::new(RateId::Lose, Quantity::new(1)));
                },
            ),
            |_| Vec::new(),
            |_| 0,
            |world| (!world.balance(&AccountId::Game, &Asset::Won).is_zero()).then_some(vec![1.0]),
            |_| vec![0.0],
            random_action,
        )
        .expect("the one applicable lazy proposal can be searched");

        assert_eq!(decision.action().rate(), &RateId::Win);
        assert_eq!(decision.children().len(), 1);
    }

    #[test]
    fn chance_nodes_sample_only_encoded_nature_exchanges() {
        let game = EconomyBuilder::new()
            .account(AccountId::Game, Account::from(basket([(Asset::Ready, 1)])))
            .account(
                AccountId::Nature,
                Account::from(basket([(Asset::GoodWeight, 1), (Asset::BadWeight, 3)])),
            )
            .rate(
                RateId::Safe,
                Rate::new()
                    .consume(Role::Game, basket([(Asset::Ready, 1)]))
                    .produce(Role::Game, basket([(Asset::SafeResult, 1)])),
            )
            .rate(
                RateId::Gamble,
                Rate::new()
                    .consume(Role::Game, basket([(Asset::Ready, 1)]))
                    .produce(Role::Game, basket([(Asset::Pending, 1)])),
            )
            .rate(
                RateId::Good,
                Rate::new()
                    .consume(Role::Game, basket([(Asset::Pending, 1)]))
                    .produce(Role::Game, basket([(Asset::GoodResult, 1)]))
                    .preserve(Role::Nature, basket([(Asset::GoodWeight, 1)]))
                    .distinct(Role::Game, Role::Nature),
            )
            .rate(
                RateId::Bad,
                Rate::new()
                    .consume(Role::Game, basket([(Asset::Pending, 1)]))
                    .produce(Role::Game, basket([(Asset::BadResult, 1)]))
                    .preserve(Role::Nature, basket([(Asset::BadWeight, 1)]))
                    .distinct(Role::Game, Role::Nature),
            )
            .build()
            .expect("test model is valid");

        let decision = search(
            &game,
            MctsConfig::new(512, 4).with_seed(11),
            1,
            |world| world.applicable([action(RateId::Safe), action(RateId::Gamble)]),
            |world| {
                [
                    (RateId::Good, Asset::GoodWeight),
                    (RateId::Bad, Asset::BadWeight),
                ]
                .into_iter()
                .filter_map(|(rate, weight)| {
                    let action = action(rate);
                    let quantity = world.balance(&AccountId::Nature, &weight).get();
                    (quantity > 0 && world.is_applicable(&action))
                        .then(|| WeightedExchange::new(action, quantity))
                })
                .collect()
            },
            |_| 0,
            |world| {
                [
                    (Asset::SafeResult, 0.6),
                    (Asset::GoodResult, 1.0),
                    (Asset::BadResult, 0.0),
                ]
                .into_iter()
                .find_map(|(asset, value)| {
                    (!world.balance(&AccountId::Game, &asset).is_zero()).then_some(vec![value])
                })
            },
            |_| vec![0.0],
            random_action,
        )
        .expect("the encoded gamble can be evaluated");

        assert_eq!(decision.action().rate(), &RateId::Safe);
        assert!(decision.nodes() >= 5);
    }
}
