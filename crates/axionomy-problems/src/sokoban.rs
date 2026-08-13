//! Sokoban encoded entirely as accounts, assets, rates, and exchanges.
//!
//! The board is not solver-owned state. Each cell is an account whose terrain
//! and occupant are assets, and a push is one atomic exchange across three cell
//! accounts. Crates keep stable identities so viewers and callers can follow
//! the same object through a replay.

use axionomy::{
    Account, Basket, Economy, EconomyBuilder, Exchange, Goal, LinearInvariant, Quantity, Rate,
    Trace, basket,
};
use axionomy_search::{SearchSolution, astar, bfs};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AccountId {
    Cell(u8),
    Success,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
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
    Active,
    Solved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Role {
    From,
    Middle,
    To,
    Puzzle,
    Goal(u8),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RateId {
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
    Finish {
        assignment: Vec<(u8, u8)>,
    },
}

pub type World = Economy<AccountId, Asset, RateId, Role>;
pub type Action = Exchange<RateId, Role, AccountId>;
pub type Solution = SearchSolution<RateId, Role, AccountId>;

const MICRO: &[&str] = &["#######", "#@ $. #", "#######"];
const SHOWCASE: &[&str] = &[
    "#########",
    "# .   . #",
    "#       #",
    "#  $ $  #",
    "#   @   #",
    "#   #   #",
    "#########",
];
const STRESS: &[&str] = &[
    "##########",
    "# .  . . #",
    "#        #",
    "# $$ $   #",
    "#   @    #",
    "#  ##    #",
    "#        #",
    "##########",
];
const DEADLOCKED: &[&str] = &["#######", "#$ @ .#", "#     #", "#######"];

pub fn initial() -> World {
    build_board(MICRO)
}

pub fn deadlocked() -> World {
    build_board(DEADLOCKED)
}

/// A bounded warehouse with two distinguishable crates, two goals, an
/// internal wall, and enough room for legal moves that do not make progress.
pub fn initial_showcase() -> World {
    build_board(SHOWCASE)
}

/// A larger three-crate warehouse intended to pressure heuristic search.
pub fn initial_stress() -> World {
    build_board(STRESS)
}

pub fn goal() -> Goal<AccountId, Asset> {
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
            proposals.push(exchange(RateId::Move {
                from: player,
                to: adjacent,
            }));
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
            proposals.push(exchange(RateId::Push {
                behind: player,
                crate_at: adjacent,
                to,
                crate_id: *crate_id,
            }));
        }
    }
    if !crates.is_empty()
        && crates.iter().all(|(_, cell)| {
            !world
                .balance(&AccountId::Cell(*cell), &Asset::GoalCell)
                .is_zero()
        })
    {
        proposals.push(exchange(RateId::Finish { assignment: crates }));
    }
    world.applicable(proposals)
}

pub fn solve(world: &World) -> Option<Solution> {
    bfs(world, &goal(), candidates)
}

pub fn solve_astar(world: &World) -> Option<Solution> {
    astar(world, &goal(), candidates, |_, _, _| 1, crate_goal_distance)
}

/// Finds a legal replay from this exact source board that strands a crate on a
/// non-goal corner. The returned trace is decision-quality counterevidence,
/// not an invalid-proposal fixture.
pub fn losing_trace(world: &World) -> Option<Trace<RateId, Role, AccountId>> {
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
    if is_wall(world, index)
        || !world
            .balance(&AccountId::Cell(index), &Asset::GoalCell)
            .is_zero()
    {
        return false;
    }
    let (width, height) = dimensions(world);
    let x = index % width;
    let y = index / width;
    let wall = |x: i16, y: i16| {
        x < 0
            || y < 0
            || x >= i16::from(width)
            || y >= i16::from(height)
            || is_wall(world, cell(width, x as u8, y as u8))
    };
    (wall(i16::from(x) - 1, i16::from(y)) || wall(i16::from(x) + 1, i16::from(y)))
        && (wall(i16::from(x), i16::from(y) - 1) || wall(i16::from(x), i16::from(y) + 1))
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
    let mut builder = EconomyBuilder::new().account(
        AccountId::Success,
        Account::from(basket([
            (Asset::Active, 1),
            (Asset::BoardWidth(width), 1),
            (Asset::BoardHeight(height), 1),
        ])),
    );

    for (y, row) in rows.iter().enumerate() {
        for (x, tile) in row.chars().enumerate() {
            let index = cell(width, x as u8, y as u8);
            let mut assets = Basket::from([(Asset::CellIdentity(index), Quantity::new(1))]);
            if tile == '#' {
                assets.insert(Asset::Wall, Quantity::new(1));
            } else {
                assets.insert(Asset::Floor, Quantity::new(1));
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
    for &from in &floors {
        for to in neighbors(from, width, height)
            .into_iter()
            .filter(|to| floors.contains(to))
        {
            builder = builder.rate(
                RateId::Move { from, to },
                Rate::new()
                    .preserve(Role::Puzzle, basket([(Asset::Active, 1)]))
                    .preserve(Role::From, basket([(Asset::CellIdentity(from), 1)]))
                    .preserve(Role::To, basket([(Asset::CellIdentity(to), 1)]))
                    .consume(Role::From, basket([(Asset::Player, 1)]))
                    .consume(Role::To, basket([(Asset::Empty, 1)]))
                    .produce(Role::From, basket([(Asset::Empty, 1)]))
                    .produce(Role::To, basket([(Asset::Player, 1)]))
                    .distinct(Role::From, Role::To),
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
                Rate::new()
                    .preserve(Role::Puzzle, basket([(Asset::Active, 1)]))
                    .preserve(Role::From, basket([(Asset::CellIdentity(behind), 1)]))
                    .preserve(Role::Middle, basket([(Asset::CellIdentity(middle), 1)]))
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
            );
        }
    }
    for assignment in assignments(&crate_ids, &goal_cells) {
        let mut finish = Rate::new()
            .consume(Role::Puzzle, basket([(Asset::Active, 1)]))
            .produce(Role::Puzzle, basket([(Asset::Solved, 1)]));
        for (crate_id, goal_cell) in &assignment {
            finish = finish
                .preserve(
                    Role::Goal(*crate_id),
                    basket([
                        (Asset::CellIdentity(*goal_cell), 1),
                        (Asset::Crate(*crate_id), 1),
                        (Asset::GoalCell, 1),
                    ]),
                )
                .distinct(Role::Goal(*crate_id), Role::Puzzle);
        }
        for left in 0..crate_ids.len() {
            for right in left + 1..crate_ids.len() {
                finish = finish.distinct(Role::Goal(crate_ids[left]), Role::Goal(crate_ids[right]));
            }
        }
        builder = builder.rate(RateId::Finish { assignment }, finish);
    }

    let mut occupancy = LinearInvariant::new("one occupant per floor")
        .weight(Asset::Player, 1)
        .weight(Asset::Empty, 1);
    for &crate_id in &crate_ids {
        builder = builder.invariant(
            LinearInvariant::new(format!("one crate {crate_id}")).weight(Asset::Crate(crate_id), 1),
        );
        occupancy = occupancy.weight(Asset::Crate(crate_id), 1);
    }
    builder
        .invariant(LinearInvariant::new("one player").weight(Asset::Player, 1))
        .invariant(occupancy)
        .invariant(
            LinearInvariant::new("puzzle lifecycle")
                .weight(Asset::Active, 1)
                .weight(Asset::Solved, 1),
        )
        .build()
        .expect("sokoban model is valid")
}

fn assignments(crate_ids: &[u8], goals: &[u8]) -> Vec<Vec<(u8, u8)>> {
    fn visit(
        crate_ids: &[u8],
        remaining: &mut Vec<u8>,
        current: &mut Vec<(u8, u8)>,
        output: &mut Vec<Vec<(u8, u8)>>,
    ) {
        if current.len() == crate_ids.len() {
            output.push(current.clone());
            return;
        }
        let crate_id = crate_ids[current.len()];
        for index in 0..remaining.len() {
            let goal = remaining.remove(index);
            current.push((crate_id, goal));
            visit(crate_ids, remaining, current, output);
            current.pop();
            remaining.insert(index, goal);
        }
    }
    let mut output = Vec::new();
    visit(crate_ids, &mut goals.to_vec(), &mut Vec::new(), &mut output);
    output
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

pub fn exchange(rate: RateId) -> Action {
    match rate {
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
        RateId::Finish { assignment } => {
            let mut exchange = Exchange::new(
                RateId::Finish {
                    assignment: assignment.clone(),
                },
                Quantity::new(1),
            )
            .bind(Role::Puzzle, AccountId::Success);
            for (crate_id, cell) in assignment {
                exchange = exchange.bind(Role::Goal(crate_id), AccountId::Cell(cell));
            }
            exchange
        }
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
}
