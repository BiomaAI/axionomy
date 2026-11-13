#![doc = include_str!("README.md")]

use axionomy::path;
use axionomy::{
    Account, Basket, Economy, EconomyBuilder, Exchange, Goal, LinearInvariant, Quantity,
    QuantityComparison as C, QuantityExpression as Q, Rate, Trace, basket,
};
use axionomy_search::{SearchSolution, astar, bfs};

#[cfg(feature = "packages")]
pub mod package;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum AccountId {
    Cell(u8),
    Success,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum Asset {
    CellIdentity(u8),
    BoardWidth(u8),
    BoardHeight(u8),
    Floor,
    Wall,
    Player,
    Crate(u8),
    Empty,
    GoalCell,
    DeadSquare,
    ExitCell,
    AtExit,
    AwayFromExit,
    Plate(u8),
    Door(u8),
    Panel(u8),
    Pressed(u8),
    Released(u8),
    LatchReady(u8),
    Latched(u8),
    Active,
    Solved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum Role {
    From,
    Middle,
    To,
    Puzzle,
    Goal(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum RateId {
    Latch {
        circuit: u8,
        at: u8,
    },
    Move {
        from: u8,
        to: u8,
    },
    Push {
        behind: u8,
        crate_at: u8,
        to: u8,
        crate_id: u8,
    },
    /// Every crate rests on a goal cell. One law for every placement: an
    /// exchange binds each crate's goal role to the cell the crate is on.
    Finish,
}

pub type World = Economy<Ontology>;

/// The identifier types of [`World`], named `sokoban` where economies are
/// stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Ontology {}

impl axionomy::Ontology for Ontology {
    const NAME: &'static str = "sokoban";
    type Account = AccountId;
    type Asset = Asset;
    type Rate = RateId;
    type Role = Role;
}
pub type Action = Exchange<Ontology>;
pub type Solution = SearchSolution<Ontology>;

const MICRO: &[&str] = &["#######", "#@ $. #", "#######"];
const SHOWCASE: &[&str] = &[
    "#########",
    "##.###.##",
    "# s #   #",
    "# $ d $ #",
    "# @ # l #",
    "######x##",
    "#########",
];
const STRESS: &[&str] = &[
    "#############",
    "##.###.###.##",
    "# s # t #   #",
    "# $ d $ e $ #",
    "# @ # l # m #",
    "##########x##",
    "#############",
];
const DEADLOCKED: &[&str] = &["#######", "#$ @ .#", "#     #", "#######"];

pub fn initial() -> World {
    build_board(MICRO)
}

pub fn deadlocked() -> World {
    build_board(DEADLOCKED)
}

/// A crate must hold the pressure plate while the player crosses the door
/// and latches it from the other chamber, before retrieving the staged crate.
pub fn initial_showcase() -> World {
    build_board(SHOWCASE)
}

/// Two coupled plate/door/panel circuits require ordered staging of three crates.
pub fn initial_stress() -> World {
    build_board(STRESS)
}

pub fn goal() -> Goal<Ontology> {
    Goal::new().require(AccountId::Success, basket([(Asset::Solved, 1)]))
}

pub fn candidates(world: &World) -> Vec<Action> {
    let Some(player) = player_cell(world) else {
        return Vec::new();
    };
    let (width, height) = dimensions(world);
    let crates = crate_cells(world);
    let mut proposals = Vec::with_capacity(5);
    for adjacent in neighbors(player, width, height) {
        if !world
            .balance(&AccountId::Cell(adjacent), &Asset::Empty)
            .is_zero()
        {
            proposals.push(exchange(
                world,
                RateId::Move {
                    from: player,
                    to: adjacent,
                },
            ));
            continue;
        }
        let Some((crate_id, _)) = crates.iter().find(|(_, cell)| *cell == adjacent) else {
            continue;
        };
        let player_x = i16::from(player % width);
        let player_y = i16::from(player / width);
        let adjacent_x = i16::from(adjacent % width);
        let adjacent_y = i16::from(adjacent / width);
        let to_x = adjacent_x + (adjacent_x - player_x);
        let to_y = adjacent_y + (adjacent_y - player_y);
        if to_x < 0 || to_y < 0 || to_x >= i16::from(width) || to_y >= i16::from(height) {
            continue;
        }
        let to = cell(width, to_x as u8, to_y as u8);
        if !world.balance(&AccountId::Cell(to), &Asset::Empty).is_zero() {
            proposals.push(exchange(
                world,
                RateId::Push {
                    behind: player,
                    crate_at: adjacent,
                    to,
                    crate_id: *crate_id,
                },
            ));
        }
    }
    if !crates.is_empty()
        && crates.iter().all(|(_, cell)| {
            !world
                .balance(&AccountId::Cell(*cell), &Asset::GoalCell)
                .is_zero()
        })
    {
        proposals.push(exchange(world, RateId::Finish));
    }
    for circuit in 0..2 {
        if !world
            .balance(&AccountId::Cell(player), &Asset::Panel(circuit))
            .is_zero()
        {
            proposals.push(exchange(
                world,
                RateId::Latch {
                    circuit,
                    at: player,
                },
            ));
        }
    }
    world.applicable(proposals)
}

/// Search may omit provably losing pushes; the economy still permits them.
pub fn search_candidates(world: &World) -> Vec<Action> {
    candidates(world)
        .into_iter()
        .filter(|action| match action.rate() {
            RateId::Push { to, .. } => !is_dead_square(world, *to),
            _ => true,
        })
        .collect()
}

pub fn solve(world: &World) -> Option<Solution> {
    bfs(world, &goal(), search_candidates)
}

pub fn solve_astar(world: &World) -> Option<Solution> {
    astar(
        world,
        &goal(),
        search_candidates,
        |_, _, _| 1,
        crate_goal_distance,
    )
}

/// Finds a legal replay from this exact source board that strands a crate on a
/// non-goal corner. The returned trace is decision-quality counterevidence,
/// not an invalid-proposal fixture.
pub fn losing_trace(world: &World) -> Option<Trace<Ontology>> {
    let (width, height) = dimensions(world);
    let targets: Vec<_> = (0..width * height)
        .filter(|cell| is_dead_square(world, *cell))
        .collect();
    for (crate_id, _) in crate_cells(world) {
        for target in &targets {
            let target_goal = Goal::new().require(
                AccountId::Cell(*target),
                basket([(Asset::Crate(crate_id), 1)]),
            );
            let solution = astar(
                world,
                &target_goal,
                candidates,
                |_, _, _| 1,
                |state| {
                    crate_cells(state)
                        .into_iter()
                        .find_map(|(id, cell)| (id == crate_id).then_some(cell))
                        .map_or(0, |cell| manhattan(cell, *target, width))
                },
            );
            if let Some(solution) = solution {
                return Some(solution.trace().clone());
            }
        }
    }
    None
}

/// An admissible lower bound that ignores walls, player repositioning, and
/// crate interference. It remains caller-owned search policy, not a rule.
pub fn crate_goal_distance(world: &World) -> u64 {
    let (width, _) = dimensions(world);
    crate_cells(world)
        .into_iter()
        .map(|(_, crate_cell)| {
            goal_cells(world)
                .into_iter()
                .map(|goal_cell| manhattan(crate_cell, goal_cell, width))
                .min()
                .unwrap_or(0)
        })
        .sum()
}

pub fn player_cell(world: &World) -> Option<u8> {
    floor_cells(world).find(|cell| {
        !world
            .balance(&AccountId::Cell(*cell), &Asset::Player)
            .is_zero()
    })
}

pub fn crate_cells(world: &World) -> Vec<(u8, u8)> {
    let crate_ids = crate_ids(world);
    floor_cells(world)
        .flat_map(|cell| {
            crate_ids.iter().copied().filter_map(move |crate_id| {
                (!world
                    .balance(&AccountId::Cell(cell), &Asset::Crate(crate_id))
                    .is_zero())
                .then_some((crate_id, cell))
            })
        })
        .collect()
}

pub fn is_wall(world: &World, cell: u8) -> bool {
    !world
        .balance(&AccountId::Cell(cell), &Asset::Wall)
        .is_zero()
}

pub fn is_dead_square(world: &World, index: u8) -> bool {
    !world
        .balance(&AccountId::Cell(index), &Asset::DeadSquare)
        .is_zero()
}

fn build_board(rows: &[&str]) -> World {
    let height = u8::try_from(rows.len()).expect("small encoded board");
    let width = u8::try_from(rows.first().expect("board has rows").chars().count())
        .expect("small encoded board");
    assert!(
        rows.iter()
            .all(|row| row.chars().count() == usize::from(width))
    );

    let mut next_crate = 0_u8;
    let mut crate_ids = Vec::new();
    let mut goal_cells = Vec::new();
    let tile_at =
        |index: u8| rows[(index / width) as usize].as_bytes()[(index % width) as usize] as char;
    let circuit_of = |tile| match tile {
        's' | 'd' | 'l' => Some(0),
        't' | 'e' | 'm' => Some(1),
        _ => None,
    };
    // Reverse-push reachability assumes every door is open and no other crate
    // exists. A square outside this optimistic set cannot reach any goal.
    let floors: std::collections::BTreeSet<_> = (0..width * height)
        .filter(|index| tile_at(*index) != '#')
        .collect();
    let mut reachable: std::collections::BTreeSet<_> = floors
        .iter()
        .copied()
        .filter(|index| matches!(tile_at(*index), '.' | '*' | '+'))
        .collect();
    loop {
        let before = reachable.len();
        for (behind, middle, to) in push_lines(width, height) {
            if floors.contains(&behind) && floors.contains(&middle) && reachable.contains(&to) {
                reachable.insert(middle);
            }
        }
        if before == reachable.len() {
            break;
        }
    }
    let mut puzzle = basket([
        (Asset::Active, 1),
        (Asset::BoardWidth(width), 1),
        (Asset::BoardHeight(height), 1),
    ]);
    for index in 0..width * height {
        if matches!(tile_at(index), 's' | 't') {
            let circuit = circuit_of(tile_at(index)).unwrap();
            puzzle.insert(Asset::Released(circuit), Quantity::new(1));
            puzzle.insert(Asset::LatchReady(circuit), Quantity::new(1));
        }
    }
    let has_exit = rows.iter().any(|row| row.contains('x'));
    if has_exit {
        puzzle.insert(Asset::AwayFromExit, Quantity::new(1));
    }
    let mut builder = EconomyBuilder::new().account(AccountId::Success, Account::from(puzzle));

    for (y, row) in rows.iter().enumerate() {
        for (x, tile) in row.chars().enumerate() {
            let index = cell(width, x as u8, y as u8);
            let mut assets = Basket::from([(Asset::CellIdentity(index), Quantity::new(1))]);
            if tile == '#' {
                assets.insert(Asset::Wall, Quantity::new(1));
            } else {
                assets.insert(Asset::Floor, Quantity::new(1));
                if !reachable.contains(&index) {
                    assets.insert(Asset::DeadSquare, Quantity::new(1));
                }
                match tile {
                    '@' | '+' => {
                        assets.insert(Asset::Player, Quantity::new(1));
                    }
                    '$' | '*' => {
                        assets.insert(Asset::Crate(next_crate), Quantity::new(1));
                        crate_ids.push(next_crate);
                        next_crate += 1;
                    }
                    _ => {
                        assets.insert(Asset::Empty, Quantity::new(1));
                    }
                }
                if tile == 'x' {
                    assets.insert(Asset::ExitCell, Quantity::new(1));
                }
                if let Some(circuit) = circuit_of(tile) {
                    let asset = match tile {
                        's' | 't' => Asset::Plate(circuit),
                        'd' | 'e' => Asset::Door(circuit),
                        _ => Asset::Panel(circuit),
                    };
                    assets.insert(asset, Quantity::new(1));
                }
                if matches!(tile, '.' | '*' | '+') {
                    assets.insert(Asset::GoalCell, Quantity::new(1));
                    goal_cells.push(index);
                }
            }
            builder = builder.account(AccountId::Cell(index), Account::from(assets));
        }
    }
    assert_eq!(
        crate_ids.len(),
        goal_cells.len(),
        "one goal is required per crate"
    );

    let floors: Vec<_> = (0..width * height)
        .filter(|index| {
            rows[usize::from(*index / width)].as_bytes()[usize::from(*index % width)] != b'#'
        })
        .collect();
    let exit_state = |mut rate: Rate<Ontology>, from: u8, to: u8| {
        if tile_at(from) == 'x' {
            rate = rate
                .consume(Role::Puzzle, basket([(Asset::AtExit, 1)]))
                .produce(Role::Puzzle, basket([(Asset::AwayFromExit, 1)]));
        }
        if tile_at(to) == 'x' {
            rate = rate
                .consume(Role::Puzzle, basket([(Asset::AwayFromExit, 1)]))
                .produce(Role::Puzzle, basket([(Asset::AtExit, 1)]));
        }
        rate
    };
    let gate = |mut rate: Rate<Ontology>, to: u8| {
        if matches!(tile_at(to), 'd' | 'e') {
            let circuit = circuit_of(tile_at(to)).unwrap();
            rate = rate.condition(
                path!("door-has-pressure-or-a-permanent-latch"),
                Q::balance(Role::Puzzle, Asset::Pressed(circuit))
                    + Q::balance(Role::Puzzle, Asset::Latched(circuit)),
                C::GreaterThanOrEqual,
                Q::constant(1),
            );
        }
        rate
    };
    let pressure = |mut rate: Rate<Ontology>, from: u8, to: u8| {
        for (index, entering) in [(from, false), (to, true)] {
            if matches!(tile_at(index), 's' | 't') {
                let circuit = circuit_of(tile_at(index)).unwrap();
                let (consume, produce) = if entering {
                    (Asset::Released(circuit), Asset::Pressed(circuit))
                } else {
                    (Asset::Pressed(circuit), Asset::Released(circuit))
                };
                rate = rate
                    .consume(Role::Puzzle, basket([(consume, 1)]))
                    .produce(Role::Puzzle, basket([(produce, 1)]));
            }
        }
        rate
    };
    for &from in &floors {
        for to in neighbors(from, width, height)
            .into_iter()
            .filter(|to| floors.contains(to))
        {
            builder = builder.rate(
                RateId::Move { from, to },
                exit_state(
                    gate(
                        Rate::new()
                            .preserve(Role::Puzzle, basket([(Asset::Active, 1)]))
                            .preserve(Role::From, basket([(Asset::CellIdentity(from), 1)]))
                            .preserve(Role::To, basket([(Asset::CellIdentity(to), 1)]))
                            .consume(Role::From, basket([(Asset::Player, 1)]))
                            .consume(Role::To, basket([(Asset::Empty, 1)]))
                            .produce(Role::From, basket([(Asset::Empty, 1)]))
                            .produce(Role::To, basket([(Asset::Player, 1)]))
                            .distinct(Role::From, Role::To),
                        to,
                    ),
                    from,
                    to,
                ),
            );
        }
    }
    for (behind, middle, to) in
        push_lines(width, height)
            .into_iter()
            .filter(|(behind, middle, to)| {
                floors.contains(behind) && floors.contains(middle) && floors.contains(to)
            })
    {
        for &crate_id in &crate_ids {
            builder = builder.rate(
                RateId::Push {
                    behind,
                    crate_at: middle,
                    to,
                    crate_id,
                },
                exit_state(
                    pressure(
                        gate(
                            gate(
                                Rate::new()
                                    .preserve(Role::Puzzle, basket([(Asset::Active, 1)]))
                                    .preserve(
                                        Role::From,
                                        basket([(Asset::CellIdentity(behind), 1)]),
                                    )
                                    .preserve(
                                        Role::Middle,
                                        basket([(Asset::CellIdentity(middle), 1)]),
                                    )
                                    .preserve(Role::To, basket([(Asset::CellIdentity(to), 1)]))
                                    .consume(Role::From, basket([(Asset::Player, 1)]))
                                    .consume(Role::Middle, basket([(Asset::Crate(crate_id), 1)]))
                                    .consume(Role::To, basket([(Asset::Empty, 1)]))
                                    .produce(Role::From, basket([(Asset::Empty, 1)]))
                                    .produce(Role::Middle, basket([(Asset::Player, 1)]))
                                    .produce(Role::To, basket([(Asset::Crate(crate_id), 1)]))
                                    .distinct(Role::From, Role::Middle)
                                    .distinct(Role::Middle, Role::To)
                                    .distinct(Role::From, Role::To),
                                middle,
                            ),
                            to,
                        ),
                        middle,
                        to,
                    ),
                    behind,
                    middle,
                ),
            );
        }
    }
    for &at in &floors {
        if matches!(tile_at(at), 'l' | 'm') {
            let circuit = circuit_of(tile_at(at)).unwrap();
            builder = builder.rate(
                RateId::Latch { circuit, at },
                Rate::new()
                    .preserve(Role::Puzzle, basket([(Asset::Active, 1)]))
                    .preserve(
                        Role::From,
                        basket([
                            (Asset::CellIdentity(at), 1),
                            (Asset::Player, 1),
                            (Asset::Panel(circuit), 1),
                        ]),
                    )
                    .consume(Role::Puzzle, basket([(Asset::LatchReady(circuit), 1)]))
                    .produce(Role::Puzzle, basket([(Asset::Latched(circuit), 1)]))
                    .distinct(Role::Puzzle, Role::From),
            );
            builder = builder
                .invariant(
                    LinearInvariant::new(path!("circuit-pressure").join(&path::write(&circuit)))
                        .weight(Asset::Pressed(circuit), 1)
                        .weight(Asset::Released(circuit), 1),
                )
                .invariant(
                    LinearInvariant::new(path!("circuit-latch").join(&path::write(&circuit)))
                        .weight(Asset::LatchReady(circuit), 1)
                        .weight(Asset::Latched(circuit), 1),
                );
        }
    }
    // Each crate's goal role must hold that crate on a goal cell. A crate is
    // in one cell, so the binding names where it rests and the law needs no
    // placement of its own.
    let mut finish = Rate::new()
        .consume(Role::Puzzle, basket([(Asset::Active, 1)]))
        .produce(Role::Puzzle, basket([(Asset::Solved, 1)]));
    if has_exit {
        finish = finish.preserve(Role::Puzzle, basket([(Asset::AtExit, 1)]));
    }
    for &crate_id in &crate_ids {
        finish = finish
            .preserve(
                Role::Goal(crate_id),
                basket([(Asset::Crate(crate_id), 1), (Asset::GoalCell, 1)]),
            )
            .distinct(Role::Goal(crate_id), Role::Puzzle);
    }
    for left in 0..crate_ids.len() {
        for right in left + 1..crate_ids.len() {
            finish = finish.distinct(Role::Goal(crate_ids[left]), Role::Goal(crate_ids[right]));
        }
    }
    builder = builder.rate(RateId::Finish, finish);

    if has_exit {
        builder = builder.invariant(
            LinearInvariant::new(path!("player-exit-position"))
                .weight(Asset::AtExit, 1)
                .weight(Asset::AwayFromExit, 1),
        );
    }
    let mut occupancy = LinearInvariant::new(path!("one-occupant-per-floor"))
        .weight(Asset::Player, 1)
        .weight(Asset::Empty, 1);
    for &crate_id in &crate_ids {
        builder = builder.invariant(
            LinearInvariant::new(path!("one-crate").join(&path::write(&crate_id)))
                .weight(Asset::Crate(crate_id), 1),
        );
        occupancy = occupancy.weight(Asset::Crate(crate_id), 1);
    }
    builder
        .invariant(LinearInvariant::new(path!("one-player")).weight(Asset::Player, 1))
        .invariant(occupancy)
        .invariant(
            LinearInvariant::new(path!("puzzle-lifecycle"))
                .weight(Asset::Active, 1)
                .weight(Asset::Solved, 1),
        )
        .build()
        .expect("sokoban model is valid")
}

fn crate_ids(world: &World) -> Vec<u8> {
    (0..16)
        .filter(|crate_id| {
            floor_cells(world).any(|cell| {
                !world
                    .balance(&AccountId::Cell(cell), &Asset::Crate(*crate_id))
                    .is_zero()
            })
        })
        .collect()
}

fn goal_cells(world: &World) -> Vec<u8> {
    floor_cells(world)
        .filter(|cell| {
            !world
                .balance(&AccountId::Cell(*cell), &Asset::GoalCell)
                .is_zero()
        })
        .collect()
}

fn floor_cells(world: &World) -> impl Iterator<Item = u8> + '_ {
    let (width, height) = dimensions(world);
    (0..width * height).filter(|cell| {
        !world
            .balance(&AccountId::Cell(*cell), &Asset::Floor)
            .is_zero()
    })
}

fn neighbors(index: u8, width: u8, height: u8) -> Vec<u8> {
    let x = index % width;
    let y = index / width;
    [
        x.checked_sub(1).map(|next| cell(width, next, y)),
        (x + 1 < width).then(|| cell(width, x + 1, y)),
        y.checked_sub(1).map(|next| cell(width, x, next)),
        (y + 1 < height).then(|| cell(width, x, y + 1)),
    ]
    .into_iter()
    .flatten()
    .collect()
}

fn push_lines(width: u8, height: u8) -> Vec<(u8, u8, u8)> {
    let mut lines = Vec::new();
    for y in 0..height {
        for x in 0..width {
            if x + 2 < width {
                lines.push((
                    cell(width, x, y),
                    cell(width, x + 1, y),
                    cell(width, x + 2, y),
                ));
                lines.push((
                    cell(width, x + 2, y),
                    cell(width, x + 1, y),
                    cell(width, x, y),
                ));
            }
            if y + 2 < height {
                lines.push((
                    cell(width, x, y),
                    cell(width, x, y + 1),
                    cell(width, x, y + 2),
                ));
                lines.push((
                    cell(width, x, y + 2),
                    cell(width, x, y + 1),
                    cell(width, x, y),
                ));
            }
        }
    }
    lines
}

const fn cell(width: u8, x: u8, y: u8) -> u8 {
    y * width + x
}

fn manhattan(left: u8, right: u8, width: u8) -> u64 {
    let (left_x, left_y) = (left % width, left / width);
    let (right_x, right_y) = (right % width, right / width);
    u64::from(left_x.abs_diff(right_x)) + u64::from(left_y.abs_diff(right_y))
}

pub fn dimensions(world: &World) -> (u8, u8) {
    let width = (1..=32)
        .find(|value| {
            !world
                .balance(&AccountId::Success, &Asset::BoardWidth(*value))
                .is_zero()
        })
        .expect("encoded board width");
    let height = (1..=32)
        .find(|value| {
            !world
                .balance(&AccountId::Success, &Asset::BoardHeight(*value))
                .is_zero()
        })
        .expect("encoded board height");
    (width, height)
}

/// The exchange applying `rate` to `world`'s board: a latch, move or push
/// binds the cells its rate names, and finishing binds each crate's goal role
/// to the cell the crate is on.
pub fn exchange(world: &World, rate: RateId) -> Action {
    match rate {
        RateId::Latch { circuit, at } => {
            Exchange::new(RateId::Latch { circuit, at }, Quantity::new(1))
                .bind(Role::Puzzle, AccountId::Success)
                .bind(Role::From, AccountId::Cell(at))
        }
        RateId::Move { from, to } => Exchange::new(RateId::Move { from, to }, Quantity::new(1))
            .bind(Role::Puzzle, AccountId::Success)
            .bind(Role::From, AccountId::Cell(from))
            .bind(Role::To, AccountId::Cell(to)),
        RateId::Push {
            behind,
            crate_at,
            to,
            crate_id,
        } => Exchange::new(
            RateId::Push {
                behind,
                crate_at,
                to,
                crate_id,
            },
            Quantity::new(1),
        )
        .bind(Role::Puzzle, AccountId::Success)
        .bind(Role::From, AccountId::Cell(behind))
        .bind(Role::Middle, AccountId::Cell(crate_at))
        .bind(Role::To, AccountId::Cell(to)),
        RateId::Finish => crate_cells(world).into_iter().fold(
            Exchange::new(RateId::Finish, Quantity::new(1)).bind(Role::Puzzle, AccountId::Success),
            |exchange, (crate_id, cell)| exchange.bind(Role::Goal(crate_id), AccountId::Cell(cell)),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn solves_by_atomic_three_account_pushes() {
        let solution = solve(&initial()).expect("puzzle is solvable");
        let mut replay = initial();
        replay.replay(solution.trace()).expect("trace must replay");
        assert!(replay.matches(&goal()));
        assert!(
            solution
                .trace()
                .exchanges()
                .iter()
                .any(|exchange| matches!(exchange.rate(), RateId::Push { .. }))
        );
    }

    #[test]
    fn showcase_has_stable_crates_walls_and_a_solution() {
        let world = initial_showcase();
        assert_eq!(crate_cells(&world).len(), 2);
        assert!((0..63).any(|cell| is_wall(&world, cell)));
        let solution = solve_astar(&world).expect("showcase must be solvable");
        assert!(solution.trace().exchanges().len() > 10);
    }

    #[test]
    fn legal_bad_push_has_replayable_deadlock_evidence() {
        let setup = initial_showcase();
        let trace = losing_trace(&setup).expect("showcase has a reachable bad corner");
        let final_world = setup.replayed(&trace).expect("losing trace must replay");
        assert!(
            crate_cells(&final_world)
                .iter()
                .any(|(_, cell)| is_dead_square(&final_world, *cell))
        );
        assert!(!final_world.matches(&goal()));
    }

    #[test]
    fn encoded_cell_identities_reject_teleports() {
        let world = initial();
        let teleport = Exchange::new(RateId::Move { from: 1, to: 2 }, Quantity::new(1))
            .bind(Role::Puzzle, AccountId::Success)
            .bind(Role::From, AccountId::Cell(1))
            .bind(Role::To, AccountId::Cell(5));
        assert!(!world.is_applicable(&teleport));
    }

    #[test]
    fn solved_puzzle_is_quiescent() {
        let solution = solve(&initial()).expect("puzzle is solvable");
        let solved = initial()
            .replayed(solution.trace())
            .expect("solution must replay");
        assert!(candidates(&solved).is_empty());
    }

    #[test]
    fn finishing_is_one_law_bound_to_where_each_crate_rests() {
        let world = initial_showcase();
        assert_eq!(
            world
                .rate_ids()
                .filter(|rate| **rate == RateId::Finish)
                .count(),
            1
        );
        let solution = solve_astar(&world).expect("showcase must be solvable");
        let (finish, moves) = solution
            .trace()
            .exchanges()
            .split_last()
            .expect("a solution finishes");
        assert_eq!(finish.rate(), &RateId::Finish);
        let mut before = world;
        for exchange in moves {
            before.apply(exchange.clone()).unwrap();
        }
        assert_eq!(finish, &exchange(&before, RateId::Finish));
        assert!(before.is_applicable(finish));

        // Each goal role must be bound to the cell holding its own crate.
        let placements = crate_cells(&before);
        let [(first, first_cell), (second, second_cell)] = placements[..] else {
            panic!("the showcase has two crates");
        };
        let bound = |pairs: [(u8, u8); 2]| {
            pairs.into_iter().fold(
                Exchange::new(RateId::Finish, Quantity::new(1))
                    .bind(Role::Puzzle, AccountId::Success),
                |exchange, (crate_id, cell)| {
                    exchange.bind(Role::Goal(crate_id), AccountId::Cell(cell))
                },
            )
        };
        assert!(before.is_applicable(&bound([(first, first_cell), (second, second_cell)])));
        assert!(!before.is_applicable(&bound([(first, second_cell), (second, first_cell)])));
        let player = player_cell(&before).unwrap();
        assert!(!before.is_applicable(&bound([(first, player), (second, second_cell)])));
    }

    #[test]
    fn pressure_needs_a_crate_and_latching_preserves_access_after_retrieval() {
        let mut world = initial_showcase();
        let walk = |world: &mut World, to| {
            world
                .apply(exchange(
                    world,
                    RateId::Move {
                        from: player_cell(world).unwrap(),
                        to,
                    },
                ))
                .unwrap();
        };
        walk(&mut world, 39);
        walk(&mut world, 30);
        let before = world.state_key();
        assert!(
            world
                .apply(exchange(&world, RateId::Move { from: 30, to: 31 }))
                .is_err()
        );
        assert_eq!(world.state_key(), before);
        walk(&mut world, 21);
        walk(&mut world, 20); // a player on the plate does not supply crate pressure
        assert!(
            world
                .balance(&AccountId::Success, &Asset::Pressed(0))
                .is_zero()
        );
        walk(&mut world, 21);
        walk(&mut world, 30);
        walk(&mut world, 39);
        walk(&mut world, 38);
        world
            .apply(exchange(
                &world,
                RateId::Push {
                    behind: 38,
                    crate_at: 29,
                    to: 20,
                    crate_id: 0,
                },
            ))
            .unwrap();
        assert_eq!(
            world
                .balance(&AccountId::Success, &Asset::Pressed(0))
                .to_u64(),
            1
        );
        for to in [30, 31, 32, 41, 42] {
            walk(&mut world, to);
        }
        world
            .apply(exchange(&world, RateId::Latch { circuit: 0, at: 42 }))
            .unwrap();
        for to in [41, 32, 31, 30, 29] {
            walk(&mut world, to);
        }
        world
            .apply(exchange(
                &world,
                RateId::Push {
                    behind: 29,
                    crate_at: 20,
                    to: 11,
                    crate_id: 0,
                },
            ))
            .unwrap();
        assert!(
            world
                .balance(&AccountId::Success, &Asset::Pressed(0))
                .is_zero()
        );
        assert_eq!(
            world
                .balance(&AccountId::Success, &Asset::Latched(0))
                .to_u64(),
            1
        );
        for to in [21, 30, 31] {
            walk(&mut world, to);
        }
    }

    #[test]
    fn two_circuits_compose_and_every_solved_trace_latches_both() {
        let world = initial_stress();
        let solution = solve_astar(&world).expect("two circuits are solvable");
        let final_world = world.replayed(solution.trace()).unwrap();
        assert!(final_world.matches(&goal()));
        for circuit in 0..2 {
            assert_eq!(
                final_world
                    .balance(&AccountId::Success, &Asset::Latched(circuit))
                    .to_u64(),
                1
            );
            assert!(
                final_world
                    .balance(&AccountId::Success, &Asset::Pressed(circuit))
                    .is_zero()
            );
        }
    }
}
