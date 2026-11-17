# axionomy-rl

`axionomy-rl` lets a learner act in an Axionomy experiment. A `Task` reads
the experiment's start, goal and objectives, and fixes what a learner needs
fixed before it trains:

- **Actions**: the exchanges of a candidate set, each listed once, in the
  order the set offers them. A `Listed` set gives its exchanges. An
  `Instances` set gives one exchange per instance, so every rate under it
  must fix every role. A set a script proposes has no fixed list and is
  refused.
- **Observations**: the balance of every account and asset the start holds
  or an action touches, each as the nearest `f32` (the economy keeps the
  exact quantity), and which actions the candidate set offers at the point.
- **Rewards**: each step's change in the weighed objectives, counted in each
  objective's direction, plus a reward for the step that reaches the goal.
  The weights are the learner's choice; the balances they read are the
  economy's.

An `Episode` steps a task through the engine. Each action is an `apply` at
the episode's point, so every episode is a recorded run that replays,
branches and draws like any other. An episode ends when the goal holds or
when the candidate set offers nothing.

No learning framework is built in. `crates/axionomy-burn` puts a task behind
Burn's `Environment` and trains a DQN on it; it is kept out of the workspace
so only it builds Burn.
