# axionomy-burn

`axionomy-burn` trains with [Burn](https://burn.dev) on Axionomy
experiments. An `axionomy_rl::Task` goes behind Burn's `Environment`: its
observation is the task's balances and which exchanges the candidate set
offers, an action is one of the task's exchanges, and the reward is the
task's. Every step is an `apply` through the engine, so each episode is a
recorded run.

The DQN here follows Burn's `dqn-agent` example with one change that matters
for an economy: the network sees what is offered, and its value for an
exchange that is not offered is pinned far below any reward, so neither its
greedy choice, its exploration nor its target ever picks one.

`train` runs Burn's off-policy loop and returns the greedy policy it
learned; `play` plays one episode with it. The `maze` example trains on the
key-door maze's Showcase twice, once weighing energy and once time, and sets
each learned route beside the routes the experiment's searches find:

```text
cargo run --release --example maze
```

What it found, at 150,000 steps for each weighing and four seeds, against
the searches' routes (energy, time): weighing time, the learner took the
(15, 4) route the Pareto search ranked by time finds, every time. Weighing
energy, it took a 9-energy route every time, the least any route spends:
Dijkstra's (9, 12) for two seeds and (9, 13) for the other two, one tick
more, which those weights count at a tenth of an energy. It needed four
updates for each step every environment takes: at one, the 12-exchange
least-energy route, which exploration completes in about one episode in four
thousand, was never learned.

Two choices in `Settings` matter for an economy. The return is not
discounted, since an objective is a total and discounting would favor a
route that reaches the goal in fewer steps over one that spends less. And the
target network is blended detached, so updates do not build one graph that
grows with every update before them.

This crate is its own workspace: Burn takes minutes to build, so the root
workspace excludes it and nothing else pays for it.
