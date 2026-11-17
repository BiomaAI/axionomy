//! A DQN whose network sees what is offered and only chooses that: its
//! values for exchanges the candidate set does not offer are pinned far
//! below any reward, so neither its greedy choice nor its target picks one,
//! and it explores among the offered exchanges alone.
//!
//! The learner follows Burn's `dqn-agent` example (MIT or Apache-2.0), with
//! the mask added and the action count read from the task.

use crate::environment::Environment;
use axionomy_rl::{Episode, Observation, RlError, Task};
use axionomy_store::Point;
use burn::grad_clipping::GradientClippingConfig;
use burn::module::{Module, Param};
use burn::nn::{self, Linear, LinearConfig, Relu, loss::MseLoss};
use burn::optim::{AdamWConfig, GradientsParams, ModuleOptimizer, OptimizerRecord};
use burn::rl::{
    Batchable, LearnerTransitionBatch, Policy, PolicyLearner, PolicyState, RLTrainOutput,
    SliceAccess,
};
use burn::store::{ModuleRecord, RecordError};
use burn::tensor::{Bytes, Device, Int, Tensor, TensorData, Transaction};
use burn::train::checkpoint::{Checkpoint, CheckpointerError};
use burn::train::metric::{
    Adaptor, CumulativeRewardMetric, EpisodeLengthMetric, ExplorationRateInput, LossInput,
};
use burn::train::{ItemLazy, OffPolicyConfig, RLStrategies, RLTraining};
use rand::rngs::SmallRng;
use rand::{RngExt, SeedableRng};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// The value given an exchange that is not offered: far below any reward,
/// and finite, so a terminal step's zeroed target stays zero.
pub const UNOFFERED: f32 = -1.0e9;

/// Observations as tensors, one row per observation: the balances, and
/// whether each action is offered, one or zero.
#[derive(Clone)]
pub struct Observed {
    pub balances: Tensor<2>,
    pub offered: Tensor<2>,
}

impl Observed {
    /// `observation` as one row on `device`.
    pub fn new(observation: &Observation, device: &Device) -> Self {
        let row = |values: Vec<f32>| {
            let width = values.len();
            Tensor::<2>::from_data(TensorData::new(values, [1, width]), device)
        };
        Self {
            balances: row(observation.balances.clone()),
            offered: row(observation
                .offered
                .iter()
                .map(|&offered| if offered { 1.0 } else { 0.0 })
                .collect()),
        }
    }
}

impl Batchable for Observed {
    fn batch(observed: Vec<Self>) -> Self {
        let (balances, offered) = observed
            .into_iter()
            .map(|row| (row.balances, row.offered))
            .unzip();
        Self {
            balances: Tensor::cat(balances, 0),
            offered: Tensor::cat(offered, 0),
        }
    }

    fn unbatch(self) -> Vec<Self> {
        self.balances
            .split(1, 0)
            .into_iter()
            .zip(self.offered.split(1, 0))
            .map(|(balances, offered)| Self { balances, offered })
            .collect()
    }
}

/// `value` on `into`'s device, as a plain or an autodiff tensor as
/// `into` is.
fn moved<const D: usize>(value: Tensor<D>, into: &Device) -> Tensor<D> {
    let value = if value.device().is_autodiff() {
        value.inner()
    } else {
        value
    };
    let plain = into.clone().inner();
    if into.is_autodiff() {
        Tensor::from_inner(value.to_device(&plain))
    } else {
        value.to_device(&plain)
    }
}

impl SliceAccess for Observed {
    fn zeros_like(sample: &Self, capacity: usize, device: &Device) -> Self {
        Self {
            balances: Tensor::zeros([capacity, sample.balances.dims()[1]], device),
            offered: Tensor::zeros([capacity, sample.offered.dims()[1]], device),
        }
    }

    fn select(self, dim: usize, indices: Tensor<1, Int>) -> Self {
        Self {
            balances: self.balances.select(dim, indices.clone()),
            offered: self.offered.select(dim, indices),
        }
    }

    fn slice_assign_inplace(&mut self, index: usize, value: Self) {
        let balances = moved(value.balances, &self.balances.device());
        let offered = moved(value.offered, &self.offered.device());
        self.balances
            .inplace(|tensor| tensor.slice_assign(index..index + 1, balances));
        self.offered
            .inplace(|tensor| tensor.slice_assign(index..index + 1, offered));
    }
}

/// The actions chosen, one row each: an action's place in the task's
/// exchanges, as a float.
#[derive(Clone)]
pub struct Chosen {
    pub actions: Tensor<2>,
}

impl Chosen {
    /// `actions`, one row each, on `device`.
    pub fn new(actions: &[usize], device: &Device) -> Self {
        let rows: Vec<f32> = actions.iter().map(|&action| action as f32).collect();
        let count = rows.len();
        Self {
            actions: Tensor::from_data(TensorData::new(rows, [count, 1]), device),
        }
    }

    /// Every row's action, in order.
    pub fn all(&self) -> Vec<usize> {
        self.actions
            .to_data()
            .try_to_vec::<f32>()
            .expect("actions are floats")
            .into_iter()
            .map(|action| action as usize)
            .collect()
    }

    /// The first row's action.
    pub fn first(&self) -> usize {
        self.all()[0]
    }
}

impl Batchable for Chosen {
    fn batch(chosen: Vec<Self>) -> Self {
        Self {
            actions: Tensor::cat(chosen.into_iter().map(|row| row.actions).collect(), 0),
        }
    }

    fn unbatch(self) -> Vec<Self> {
        self.actions
            .split(1, 0)
            .into_iter()
            .map(|actions| Self { actions })
            .collect()
    }
}

impl SliceAccess for Chosen {
    fn zeros_like(sample: &Self, capacity: usize, device: &Device) -> Self {
        Self {
            actions: Tensor::zeros([capacity, sample.actions.dims()[1]], device),
        }
    }

    fn select(self, dim: usize, indices: Tensor<1, Int>) -> Self {
        Self {
            actions: self.actions.select(dim, indices),
        }
    }

    fn slice_assign_inplace(&mut self, index: usize, value: Self) {
        let actions = moved(value.actions, &self.actions.device());
        self.actions
            .inplace(|tensor| tensor.slice_assign(index..index + 1, actions));
    }
}

/// Each action's value, one row per observation.
#[derive(Clone)]
pub struct Values(pub Tensor<2>);

impl Batchable for Values {
    fn batch(values: Vec<Self>) -> Self {
        Self(Tensor::cat(
            values.into_iter().map(|row| row.0).collect(),
            0,
        ))
    }

    fn unbatch(self) -> Vec<Self> {
        self.0.split(1, 0).into_iter().map(Self).collect()
    }
}

/// A multilayer perceptron from an observation, its balances and what is
/// offered, to each action's value, the unoffered pinned at [`UNOFFERED`].
#[derive(Module, Debug)]
pub struct QNetwork {
    layers: Vec<Linear>,
    activation: Relu,
}

impl QNetwork {
    /// A network for `observed` balances and `actions` actions, with
    /// `depth` layers, the inner ones `hidden` wide.
    pub fn new(
        observed: usize,
        actions: usize,
        hidden: usize,
        depth: usize,
        device: &Device,
    ) -> Self {
        let inputs = observed + actions;
        let layers = (0..depth)
            .map(|layer| {
                let from = if layer == 0 { inputs } else { hidden };
                let to = if layer + 1 == depth { actions } else { hidden };
                LinearConfig::new(from, to).init(device)
            })
            .collect();
        Self {
            layers,
            activation: Relu::new(),
        }
    }

    /// Each action's value, the unoffered pinned at [`UNOFFERED`].
    pub fn forward(&self, observed: Observed) -> Tensor<2> {
        let mut values = Tensor::cat(vec![observed.balances, observed.offered.clone()], 1);
        let last = self.layers.len() - 1;
        for (index, layer) in self.layers.iter().enumerate() {
            values = layer.forward(values);
            if index < last {
                values = self.activation.forward(values);
            }
        }
        values.mask_fill(observed.offered.lower_elem(0.5), UNOFFERED)
    }

    /// This network moved `tau` of the way toward `toward`.
    fn soft_update(&self, toward: &Self, tau: f64) -> Self {
        let layers = self
            .layers
            .iter()
            .zip(&toward.layers)
            .map(|(this, that)| Linear {
                weight: blend(&this.weight, &that.weight, tau),
                bias: match (&this.bias, &that.bias) {
                    (Some(this), Some(that)) => Some(blend(this, that, tau)),
                    _ => None,
                },
            })
            .collect();
        Self {
            layers,
            activation: self.activation.clone(),
        }
    }
}

/// `this` moved `tau` of the way toward `that`, detached: the target is
/// never trained, and keeping the graph would chain every update to all the
/// ones before it.
fn blend<const D: usize>(
    this: &Param<Tensor<D>>,
    that: &Param<Tensor<D>>,
    tau: f64,
) -> Param<Tensor<D>> {
    Param::initialized(
        this.id,
        (this.val() * (1.0 - tau) + that.val() * tau).detach(),
    )
}

/// The greedy policy of a [`QNetwork`]: the offered action it values most.
#[derive(Clone)]
pub struct Dqn {
    network: QNetwork,
}

/// A [`Dqn`]'s network, as Burn's runners pass it around.
#[derive(Clone)]
pub struct DqnState {
    network: QNetwork,
}

impl PolicyState for DqnState {
    type Record = ModuleRecord;

    fn into_record(self) -> ModuleRecord {
        self.network.into_record()
    }

    fn load_record(&self, record: ModuleRecord) -> Self {
        Self {
            network: self.network.clone().load_record(record),
        }
    }
}

impl Dqn {
    /// The offered action `observed`'s first row values most.
    pub fn choose(&self, observed: Observed) -> usize {
        Chosen {
            actions: self.network.forward(observed).argmax(1).float(),
        }
        .first()
    }
}

impl Policy for Dqn {
    type Observation = Observed;
    type ActionDistribution = Values;
    type Action = Chosen;
    type ActionContext = ();
    type PolicyState = DqnState;

    fn forward(&mut self, observed: Observed) -> Values {
        Values(self.network.forward(observed))
    }

    fn action(&mut self, observed: Observed, _deterministic: bool) -> (Chosen, Vec<()>) {
        let rows = observed.balances.dims()[0];
        let actions = self.network.forward(observed).argmax(1).float();
        (Chosen { actions }, vec![(); rows])
    }

    fn update(&mut self, update: DqnState) {
        self.network = update.network;
    }

    fn state(&self) -> DqnState {
        DqnState {
            network: self.network.clone(),
        }
    }

    fn to_device(self, device: &Device) -> Self {
        Self {
            network: self.network.to_device(device),
        }
    }

    fn load_record(self, record: ModuleRecord) -> Self {
        Self {
            network: self.network.load_record(record),
        }
    }
}

/// How far exploration had decayed when an action was chosen.
#[derive(Clone)]
pub struct Exploring {
    pub epsilon: f64,
}

impl ItemLazy for Exploring {
    fn sync(self) -> Result<Self, burn::tensor::ExecutionError> {
        Ok(self)
    }
}

impl Adaptor<ExplorationRateInput> for Exploring {
    fn adapt(&self) -> ExplorationRateInput {
        ExplorationRateInput::new(self.epsilon)
    }
}

/// A [`Dqn`] that, with a chance decaying from `start` to `end` over
/// `decay` choices, picks an offered action at random instead.
#[derive(Clone)]
pub struct Explore {
    greedy: Dqn,
    start: f64,
    end: f64,
    decay: f64,
    step: usize,
    random: SmallRng,
}

/// An [`Explore`]'s network and how many choices it has made.
#[derive(Clone)]
pub struct ExploreState {
    greedy: DqnState,
    step: usize,
}

impl PolicyState for ExploreState {
    type Record = ModuleRecord;

    fn into_record(self) -> ModuleRecord {
        self.greedy.into_record()
    }

    fn load_record(&self, record: ModuleRecord) -> Self {
        Self {
            greedy: self.greedy.load_record(record),
            step: self.step,
        }
    }
}

impl Explore {
    /// The greedy policy it explores around.
    pub fn greedy(&self) -> &Dqn {
        &self.greedy
    }

    fn epsilon(&self) -> f64 {
        self.end + (self.start - self.end) * (-(self.step as f64) / self.decay).exp()
    }
}

impl Policy for Explore {
    type Observation = Observed;
    type ActionDistribution = Values;
    type Action = Chosen;
    type ActionContext = Exploring;
    type PolicyState = ExploreState;

    fn forward(&mut self, observed: Observed) -> Values {
        self.greedy.forward(observed)
    }

    fn action(&mut self, observed: Observed, deterministic: bool) -> (Chosen, Vec<Exploring>) {
        let device = observed.offered.device();
        let [rows, actions] = observed.offered.dims();
        let offered = observed
            .offered
            .to_data()
            .try_to_vec::<f32>()
            .expect("the mask is floats");
        let greedy = self.greedy.action(observed, true).0.all();
        let mut chosen = Vec::with_capacity(rows);
        let mut contexts = Vec::with_capacity(rows);
        for (row, greedy) in greedy.into_iter().enumerate() {
            let epsilon = if deterministic { 0.0 } else { self.epsilon() };
            self.step += 1;
            contexts.push(Exploring { epsilon });
            let open: Vec<usize> = (0..actions)
                .filter(|&action| offered[row * actions + action] > 0.5)
                .collect();
            let explore = !open.is_empty() && self.random.random::<f64>() < epsilon;
            chosen.push(if explore {
                open[self.random.random_range(0..open.len())]
            } else {
                greedy
            });
        }
        (Chosen::new(&chosen, &device), contexts)
    }

    fn update(&mut self, update: ExploreState) {
        self.greedy.update(update.greedy);
    }

    fn state(&self) -> ExploreState {
        ExploreState {
            greedy: self.greedy.state(),
            step: self.step,
        }
    }

    fn to_device(self, device: &Device) -> Self {
        Self {
            greedy: self.greedy.to_device(device),
            ..self
        }
    }

    fn load_record(self, record: ModuleRecord) -> Self {
        Self {
            greedy: self.greedy.load_record(record),
            ..self
        }
    }
}

/// How a DQN learns, and for how long.
#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    /// Environment steps to train for, over every environment.
    pub steps: usize,
    /// Layers of the network, and the width of the inner ones.
    pub depth: usize,
    pub hidden: usize,
    pub learning_rate: f64,
    /// How much a later reward counts against one now. One by default: an
    /// objective is a total, and discounting would favor a route that
    /// reaches the goal in fewer steps over one that spends less.
    pub gamma: f64,
    /// How far the target network moves toward the network each update.
    pub tau: f64,
    /// The chance of exploring, from `epsilon_start` toward `epsilon_end`,
    /// decaying by `e` every `epsilon_decay` choices.
    pub epsilon_start: f64,
    pub epsilon_end: f64,
    pub epsilon_decay: f64,
    /// Episodes stepped side by side.
    pub environments: usize,
    /// Transitions kept to learn from, and drawn per update.
    pub replay: usize,
    pub batch: usize,
    /// Updates each time every environment has taken a step.
    pub updates: usize,
    /// Seeds exploration.
    pub seed: u64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            steps: 30_000,
            depth: 3,
            hidden: 128,
            learning_rate: 1e-3,
            gamma: 1.0,
            tau: 0.01,
            epsilon_start: 1.0,
            epsilon_end: 0.05,
            epsilon_decay: 5_000.0,
            environments: 8,
            replay: 50_000,
            batch: 128,
            updates: 4,
            seed: 17,
        }
    }
}

/// A DQN learner: the network it trains, the target it bootstraps from,
/// the exploring policy it acts with, and its optimizer.
#[derive(Clone)]
pub struct DqnLearner {
    network: QNetwork,
    target: QNetwork,
    explore: Explore,
    optimizer: ModuleOptimizer,
    learning_rate: f64,
    gamma: f64,
    tau: f64,
}

impl DqnLearner {
    /// A learner starting from `network`, as `settings` say.
    pub fn new(network: QNetwork, optimizer: ModuleOptimizer, settings: &Settings) -> Self {
        Self {
            target: network.clone(),
            explore: Explore {
                greedy: Dqn {
                    network: network.clone(),
                },
                start: settings.epsilon_start,
                end: settings.epsilon_end,
                decay: settings.epsilon_decay,
                step: 0,
                random: SmallRng::seed_from_u64(settings.seed),
            },
            network,
            optimizer,
            learning_rate: settings.learning_rate,
            gamma: settings.gamma,
            tau: settings.tau,
        }
    }
}

/// The loss of one update.
#[derive(Clone)]
pub struct Trained {
    pub loss: Tensor<1>,
}

impl ItemLazy for Trained {
    fn sync(self) -> Result<Self, burn::tensor::ExecutionError> {
        let [loss] = Transaction::default()
            .register(self.loss)
            .execute()
            .try_into()
            .expect("one tensor registered");
        Ok(Self {
            loss: Tensor::from_data(loss, &Device::flex()),
        })
    }
}

impl Adaptor<LossInput> for Trained {
    fn adapt(&self) -> LossInput {
        LossInput::new(self.loss.clone())
    }
}

/// A learner's networks and optimizer, saved as one checkpoint.
pub struct DqnRecord {
    network: ModuleRecord,
    target: ModuleRecord,
    optimizer: OptimizerRecord,
}

/// `bytes` with their length before them.
fn frame(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
    out.extend_from_slice(bytes);
}

/// The bytes [`frame`] wrote at `offset`, moving `offset` past them.
fn unframe<'a>(data: &'a [u8], offset: &mut usize) -> Result<&'a [u8], RecordError> {
    let corrupt = |what: &str| RecordError::Io(format!("corrupt checkpoint: {what}"));
    let body = *offset + 8;
    let length = data
        .get(*offset..body)
        .ok_or_else(|| corrupt("no length"))?;
    let length = u64::from_le_bytes(length.try_into().expect("eight bytes")) as usize;
    let bytes = data
        .get(body..body + length)
        .ok_or_else(|| corrupt("cut short"))?;
    *offset = body + length;
    Ok(bytes)
}

impl Checkpoint for DqnRecord {
    fn save(self, path: PathBuf) -> Result<(), CheckpointerError> {
        let bytes = self
            .checkpoint_into_bytes()
            .map_err(CheckpointerError::Record)?;
        std::fs::write(path, &*bytes).map_err(CheckpointerError::IOError)
    }

    fn load(path: PathBuf) -> Result<Self, CheckpointerError> {
        let data = std::fs::read(path).map_err(CheckpointerError::IOError)?;
        Self::checkpoint_from_bytes(Bytes::from_bytes_vec(data)).map_err(CheckpointerError::Record)
    }

    fn checkpoint_into_bytes(self) -> Result<Bytes, RecordError> {
        let mut out = Vec::new();
        frame(&mut out, &self.network.into_bytes()?);
        frame(&mut out, &self.target.into_bytes()?);
        frame(&mut out, &self.optimizer.into_bytes()?);
        Ok(Bytes::from_bytes_vec(out))
    }

    fn checkpoint_from_bytes(bytes: Bytes) -> Result<Self, RecordError> {
        let mut offset = 0;
        let mut next = || -> Result<Bytes, RecordError> {
            Ok(Bytes::from_bytes_vec(
                unframe(&bytes, &mut offset)?.to_vec(),
            ))
        };
        Ok(Self {
            network: ModuleRecord::from_bytes(next()?)?,
            target: ModuleRecord::from_bytes(next()?)?,
            optimizer: OptimizerRecord::from_bytes(next()?)?,
        })
    }
}

impl PolicyLearner for DqnLearner {
    type TrainContext = Trained;
    type InnerPolicy = Explore;
    type Record = DqnRecord;

    fn train(
        &mut self,
        input: LearnerTransitionBatch<Explore>,
    ) -> RLTrainOutput<Trained, ExploreState> {
        let values = self
            .network
            .forward(input.states)
            .gather(1, input.actions.actions.int());
        let next = self
            .target
            .forward(input.next_states)
            .max_dim(1)
            .squeeze::<1>();
        let going_on = Tensor::ones_like(&input.dones) - input.dones;
        let expected = (next * going_on.squeeze())
            .mul_scalar(self.gamma)
            .add(input.rewards.squeeze())
            .detach()
            .unsqueeze_dim::<2>(1);
        let loss = MseLoss::new().forward(values, expected, nn::loss::Reduction::Mean);
        let gradients = GradientsParams::from_grads(loss.backward(), &self.network);
        self.network = self
            .optimizer
            .step(self.learning_rate, self.network.clone(), gradients);
        self.target = self.target.soft_update(&self.network, self.tau);
        // The policy only acts, so it gets the network without its graph.
        let update = ExploreState {
            greedy: DqnState {
                network: self.network.valid(),
            },
            step: self.explore.step,
        };
        self.explore.update(update.clone());
        RLTrainOutput {
            policy: update,
            item: Trained { loss },
        }
    }

    fn policy(&self) -> Explore {
        let mut explore = self.explore.clone();
        explore.greedy.network = explore.greedy.network.valid();
        explore
    }

    fn update_policy(&mut self, update: Explore) {
        self.explore = update;
    }

    fn record(&self) -> DqnRecord {
        DqnRecord {
            network: self.network.clone().into_record(),
            target: self.target.clone().into_record(),
            optimizer: self.optimizer.to_record(),
        }
    }

    fn load_record(self, record: DqnRecord) -> Self {
        Self {
            network: self.network.load_record(record.network),
            target: self.target.load_record(record.target),
            optimizer: self.optimizer.load_record(record.optimizer),
            ..self
        }
    }

    fn device(&self) -> Device {
        self.network.devices()[0].clone()
    }
}

/// Trains a DQN on `task`'s episodes, each cut off after `MAX_STEPS`
/// steps, as `settings` say, Burn keeping its logs and checkpoints in
/// `artifacts`; returns the greedy policy it learned.
pub fn train<const MAX_STEPS: usize>(
    task: &Arc<Task>,
    settings: &Settings,
    artifacts: &Path,
) -> Dqn {
    let device = Device::flex().autodiff();
    let network = QNetwork::new(
        task.observed().len(),
        task.actions().len(),
        settings.hidden,
        settings.depth,
        &device,
    );
    let optimizer = AdamWConfig::new()
        .with_grad_clipping(Some(GradientClippingConfig::Value(100.0)))
        .init();
    let learner = DqnLearner::new(network, optimizer, settings);
    let episodes = {
        let task = Arc::clone(task);
        move || Environment::<MAX_STEPS>::new(Arc::clone(&task))
    };
    let learning = OffPolicyConfig {
        num_envs: settings.environments,
        autobatch_size: settings.environments,
        replay_buffer_size: settings.replay,
        train_interval: settings.environments,
        train_steps: settings.updates,
        eval_interval: settings.steps,
        eval_episodes: 1,
        train_batch_size: settings.batch,
        warmup_steps: settings.batch,
    };
    let result = RLTraining::new(artifacts, episodes)
        .metrics_episode((EpisodeLengthMetric::new(), CumulativeRewardMetric::new()))
        .num_steps(settings.steps)
        .with_learning_strategy(RLStrategies::OffPolicyStrategy(learning))
        .with_inference_device(Device::flex())
        .renderer(Box::new(crate::silent::Silent))
        .launch(learner);
    result.policy.greedy
}

/// An episode a policy played.
#[derive(Debug, Clone, PartialEq)]
pub struct Played {
    /// Where it ended.
    pub point: Point,
    /// The exchanges it applied, by action.
    pub actions: Vec<usize>,
    /// What its steps earned together.
    pub reward: f64,
    /// Whether it reached the goal.
    pub reached: bool,
}

/// Plays one episode of `task` with `policy` choosing greedily, stopping
/// when it ends or after `limit` steps.
pub fn play(task: &Arc<Task>, policy: &Dqn, limit: usize) -> Result<Played, RlError> {
    let device = Device::flex();
    let policy = policy.clone().to_device(&device);
    let mut episode = Episode::new(Arc::clone(task));
    let mut played = Played {
        point: episode.point(),
        actions: Vec::new(),
        reward: 0.0,
        reached: false,
    };
    while played.actions.len() < limit {
        let action = policy.choose(Observed::new(episode.observation(), &device));
        let step = episode.step(action)?;
        played.actions.push(action);
        played.reward += step.reward;
        played.point = step.point;
        played.reached = step.reached;
        if step.ended() {
            break;
        }
    }
    Ok(played)
}
