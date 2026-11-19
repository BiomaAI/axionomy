#![doc = include_str!("README.md")]

use axionomy::path;
use axionomy::{
    Account, Basket, Economy, EconomyBuilder, Exchange, Goal, LinearInvariant, Param, Pattern,
    Quantity, QuantityComparison as C, QuantityExpression as Q, Rate, Relation, Segment, basket,
};

#[cfg(feature = "packages")]
pub mod package;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum AccountId {
    Cell(u8),
    Success,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum Asset {
    Floor,
    Wall,
    Player,
    Crate(u8),
    Empty,
    DeadSquare,
    AtExit,
    AwayFromExit,
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
    /// A goal cell, by its order on the board, which finishing fixes to it.
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
    /// Every goal cell holds a crate. One law for every placement: its goal
    /// roles are fixed to the goal cells, whichever crates rest on them.
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

#[cfg(test)]
const MICRO: &[&str] = &["#######", "#@ $. #", "#######"];
#[cfg(test)]
const SHOWCASE: &[&str] = &[
    "#########",
    "##.###.##",
    "# s #   #",
    "# $ d $ #",
    "# @ # l #",
    "######x##",
    "#########",
];
#[cfg(test)]
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
    crate::fixtures::initial(axionomy::path!("sokoban/micro"))
}

pub fn deadlocked() -> World {
    build_board(DEADLOCKED)
}

/// A crate must hold the pressure plate while the player crosses the door
/// and latches it from the other chamber, before retrieving the staged crate.
pub fn initial_showcase() -> World {
    crate::fixtures::initial(axionomy::path!("sokoban/showcase"))
}

/// Two coupled plate/door/panel circuits require ordered staging of three crates.
pub fn initial_stress() -> World {
    crate::fixtures::initial(axionomy::path!("sokoban/stress"))
}

pub fn goal() -> Goal<Ontology> {
    Goal::new().require(AccountId::Success, basket([(Asset::Solved, 1)]))
}

/// Each crate and the cell it rests on.
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

/// What a floor cell is, which no exchange changes: the exit, or a
/// circuit's door or plate. A law reads it of the cells it binds from its
/// relations' rows, which give `1` where the cell is so and `0` where not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Fact {
    Exit,
    Door(u8),
    Plate(u8),
}

impl Fact {
    /// Whether a cell drawn as `tile` is so.
    fn of(self, tile: char) -> bool {
        match self {
            Self::Exit => tile == 'x',
            Self::Door(circuit) => matches!(tile, 'd' | 'e') && circuit_of(tile) == Some(circuit),
            Self::Plate(circuit) => matches!(tile, 's' | 't') && circuit_of(tile) == Some(circuit),
        }
    }

    /// The number a relation gives of the cell `cell` names, named after
    /// it: `behind-exit`, `to-door-0`, `crate-at-plate-1`.
    fn given(self, cell: &Param<u8>) -> Param<u64> {
        let fact = match self {
            Self::Exit => "exit".to_owned(),
            Self::Door(circuit) => format!("door-{circuit}"),
            Self::Plate(circuit) => format!("plate-{circuit}"),
        };
        Param::new(
            format!("{}-{fact}", cell.name())
                .parse()
                .expect("a variable's name is one segment"),
        )
    }
}

/// Whether a cell drawn as `tile` is a goal.
const fn is_goal(tile: char) -> bool {
    matches!(tile, '.' | '*' | '+')
}

/// The circuit a plate, door or panel drawn as `tile` belongs to.
const fn circuit_of(tile: char) -> Option<u8> {
    match tile {
        's' | 'd' | 'l' => Some(0),
        't' | 'e' | 'm' => Some(1),
        _ => None,
    }
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
    let tile_at =
        |index: u8| rows[(index / width) as usize].as_bytes()[(index % width) as usize] as char;
    let goal_cells: Vec<u8> = (0..width * height)
        .filter(|index| is_goal(tile_at(*index)))
        .collect();
    // Reverse-push reachability assumes every door is open and no other crate
    // exists. A square outside this optimistic set cannot reach any goal.
    let floors: std::collections::BTreeSet<_> = (0..width * height)
        .filter(|index| tile_at(*index) != '#')
        .collect();
    let mut reachable: std::collections::BTreeSet<_> = floors
        .iter()
        .copied()
        .filter(|index| is_goal(tile_at(*index)))
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
    let mut puzzle = basket([(Asset::Active, 1)]);
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
    // The board's size is the whole warehouse's and never changes: two
    // constants, which the search and the view read.
    let mut builder = EconomyBuilder::new()
        .account(AccountId::Success, Account::from(puzzle))
        .constant(&board_width(), width)
        .constant(&board_height(), height);

    for (y, row) in rows.iter().enumerate() {
        for (x, tile) in row.chars().enumerate() {
            let index = cell(width, x as u8, y as u8);
            let mut assets = Basket::new();
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
    let circuits: Vec<u8> = (0..width * height)
        .filter(|index| matches!(tile_at(*index), 's' | 't'))
        .filter_map(|index| circuit_of(tile_at(index)))
        .collect();
    // Which floors adjoin, and which line up three in a row: the board's
    // geometry, which no exchange changes.
    let steps: Vec<(u8, u8)> = floors
        .iter()
        .flat_map(|&from| {
            neighbors(from, width, height)
                .into_iter()
                .filter(|to| floors.contains(to))
                .map(move |to| (from, to))
        })
        .collect();
    let lines: Vec<(u8, u8, u8)> = push_lines(width, height)
        .into_iter()
        .filter(|(behind, middle, to)| {
            floors.contains(behind) && floors.contains(middle) && floors.contains(to)
        })
        .collect();
    let panels: Vec<(u8, u8)> = floors
        .iter()
        .filter(|at| matches!(tile_at(**at), 'l' | 'm'))
        .map(|&at| (circuit_of(tile_at(at)).unwrap(), at))
        .collect();

    let (from, to) = (Param::<u8>::new(path!("from")), Param::new(path!("to")));
    let (behind, crate_at) = (
        Param::<u8>::new(path!("behind")),
        Param::new(path!("crate-at")),
    );
    let crate_id = Param::<u8>::new(path!("crate"));
    let (circuit, at) = (Param::<u8>::new(path!("circuit")), Param::new(path!("at")));
    // A variable over the floor cells, and the cell account it names.
    let floor = |variable: &Param<u8>| variable.values(floors.iter().copied());
    let cell_at = |variable: &Param<u8>| AccountIdPattern::Cell(variable.var()).into();
    let crate_on = || AssetPattern::Crate(crate_id.var()).into();
    // `rate`, given what the cell `cell` binds is in the respects `facts`
    // lists: the board's layout, a row for each floor.
    let described = |rate: Rate<Ontology>, cell: &Param<u8>, facts: &[Fact]| {
        if facts.is_empty() {
            return rate;
        }
        rate.relation(Relation::giving(
            [cell.name().clone()],
            facts.iter().map(|fact| fact.given(cell).name().clone()),
            floors.iter().map(|&at| {
                std::iter::once(Segment::from(u64::from(at))).chain(
                    facts
                        .iter()
                        .map(move |fact| Segment::from(u64::from(fact.of(tile_at(at))))),
                )
            }),
        ))
    };
    let exit: &[Fact] = if has_exit { &[Fact::Exit] } else { &[] };
    let doors: Vec<Fact> = circuits
        .iter()
        .map(|&circuit| Fact::Door(circuit))
        .collect();
    let plates: Vec<Fact> = circuits
        .iter()
        .map(|&circuit| Fact::Plate(circuit))
        .collect();
    // A door in the cell `cell` binds lets the role bound to it in only
    // while its circuit's plate holds a crate or the door is latched.
    let gated = |rate: Rate<Ontology>, role: Role, cell: &Param<u8>| {
        circuits.iter().fold(rate, |rate, &circuit| {
            rate.condition(
                path!("door-has-pressure-or-a-permanent-latch")
                    .join(&path::write(&circuit))
                    .join(&path::write(&role)),
                Fact::Door(circuit).given(cell).quantity(),
                C::LessThanOrEqual,
                Q::balance(Role::Puzzle, Asset::Pressed(circuit))
                    + Q::balance(Role::Puzzle, Asset::Latched(circuit)),
            )
        })
    };
    // The player leaving the exit, the cell `left` binds, or reaching it,
    // the cell `entered` binds, moves the puzzle's record of where the
    // player is.
    let walked = |rate: Rate<Ontology>, left: &Param<u8>, entered: &Param<u8>| {
        if !has_exit {
            return rate;
        }
        let leaves = || Fact::Exit.given(left).quantity();
        let reaches = || Fact::Exit.given(entered).quantity();
        rate.consume_computed(Role::Puzzle, Asset::AtExit, leaves())
            .produce_computed(Role::Puzzle, Asset::AwayFromExit, leaves())
            .consume_computed(Role::Puzzle, Asset::AwayFromExit, reaches())
            .produce_computed(Role::Puzzle, Asset::AtExit, reaches())
    };
    // A crate pushed off a plate releases it; one pushed onto a plate
    // presses it.
    let pressed = |rate: Rate<Ontology>| {
        circuits.iter().fold(rate, |rate, &circuit| {
            let leaves = || Fact::Plate(circuit).given(&crate_at).quantity();
            let reaches = || Fact::Plate(circuit).given(&to).quantity();
            rate.consume_computed(Role::Puzzle, Asset::Pressed(circuit), leaves())
                .produce_computed(Role::Puzzle, Asset::Released(circuit), leaves())
                .consume_computed(Role::Puzzle, Asset::Released(circuit), reaches())
                .produce_computed(Role::Puzzle, Asset::Pressed(circuit), reaches())
        })
    };
    let board = || {
        Rate::new()
            .account(Role::Puzzle, Pattern::literal(AccountId::Success))
            .preserve(Role::Puzzle, basket([(Asset::Active, 1)]))
    };
    let step = board()
        .variable(floor(&from))
        .variable(floor(&to))
        .relation(Relation::pairs(&from, &to, steps))
        .account(Role::From, cell_at(&from))
        .account(Role::To, cell_at(&to))
        .consume(Role::From, basket([(Asset::Player, 1)]))
        .consume(Role::To, basket([(Asset::Empty, 1)]))
        .produce(Role::From, basket([(Asset::Empty, 1)]))
        .produce(Role::To, basket([(Asset::Player, 1)]));
    let step = described(step, &from, exit);
    let step = described(step, &to, &[exit, &doors].concat());
    builder = builder.rate_at(
        RateIdPattern::Move {
            from: from.var(),
            to: to.var(),
        }
        .into(),
        walked(gated(step, Role::To, &to), &from, &to),
    );
    if !crate_ids.is_empty() && !lines.is_empty() {
        let line = |(behind, middle, to): (u8, u8, u8)| {
            [behind, middle, to].map(|cell| Segment::from(u64::from(cell)))
        };
        let push = board()
            .variable(floor(&behind))
            .variable(floor(&crate_at))
            .variable(floor(&to))
            .variable(crate_id.values(crate_ids.iter().copied()))
            .relation(Relation::new(
                [behind.name(), crate_at.name(), to.name()].map(Clone::clone),
                lines.into_iter().map(line),
            ))
            .account(Role::From, cell_at(&behind))
            .account(Role::Middle, cell_at(&crate_at))
            .account(Role::To, cell_at(&to))
            .consume(Role::From, basket([(Asset::Player, 1)]))
            .consume_at(Role::Middle, [(crate_on(), Quantity::new(1))])
            .consume(Role::To, basket([(Asset::Empty, 1)]))
            .produce(Role::From, basket([(Asset::Empty, 1)]))
            .produce(Role::Middle, basket([(Asset::Player, 1)]))
            .produce_at(Role::To, [(crate_on(), Quantity::new(1))]);
        let push = described(push, &behind, exit);
        let push = described(push, &crate_at, &[exit, &doors, &plates].concat());
        let push = described(push, &to, &[&doors[..], &plates].concat());
        builder = builder.rate_at(
            RateIdPattern::Push {
                behind: behind.var(),
                crate_at: crate_at.var(),
                to: to.var(),
                crate_id: crate_id.var(),
            }
            .into(),
            walked(
                pressed(gated(gated(push, Role::Middle, &crate_at), Role::To, &to)),
                &behind,
                &crate_at,
            ),
        );
    }
    if !panels.is_empty() {
        builder = builder.rate_at(
            RateIdPattern::Latch {
                circuit: circuit.var(),
                at: at.var(),
            }
            .into(),
            board()
                .variable(circuit.values(panels.iter().map(|(circuit, _)| *circuit)))
                .variable(at.values(panels.iter().map(|(_, at)| *at)))
                .relation(Relation::pairs(&circuit, &at, panels.iter().copied()))
                .account(Role::From, cell_at(&at))
                .preserve(Role::From, basket([(Asset::Player, 1)]))
                .consume_at(
                    Role::Puzzle,
                    [(
                        AssetPattern::LatchReady(circuit.var()).into(),
                        Quantity::new(1),
                    )],
                )
                .produce_at(
                    Role::Puzzle,
                    [(
                        AssetPattern::Latched(circuit.var()).into(),
                        Quantity::new(1),
                    )],
                ),
        );
    }
    for &circuit in &circuits {
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
    // Finishing is one law for every placement: each goal role is fixed to
    // its goal cell, which must hold a crate, whichever crate it is.
    let mut finish = Rate::new()
        .account(Role::Puzzle, Pattern::literal(AccountId::Success))
        .consume(Role::Puzzle, basket([(Asset::Active, 1)]))
        .produce(Role::Puzzle, basket([(Asset::Solved, 1)]));
    if has_exit {
        finish = finish.preserve(Role::Puzzle, basket([(Asset::AtExit, 1)]));
    }
    for (goal, &cell) in (0..).zip(&goal_cells) {
        let role = Role::Goal(goal);
        let crates = crate_ids
            .iter()
            .map(|&crate_id| Q::balance(role, Asset::Crate(crate_id)))
            .reduce(|left, right| left + right)
            .expect("a goal for each crate, and a crate");
        finish = finish
            .account(role, Pattern::literal(AccountId::Cell(cell)))
            .condition(
                path!("goal-holds-a-crate").join(&path::write(&goal)),
                crates,
                C::GreaterThanOrEqual,
                Q::constant(1),
            );
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

/// How many cells wide the board is, `{board-width}`: a constant.
fn board_width() -> Param<u8> {
    Param::new(path!("board-width"))
}

/// How many cells high the board is, `{board-height}`: a constant.
fn board_height() -> Param<u8> {
    Param::new(path!("board-height"))
}

/// The board's width and height, as `world`'s constants give them.
pub fn dimensions(world: &World) -> (u8, u8) {
    let size = |side: Param<u8>| {
        world
            .constants()
            .find(|(name, _)| *name == side.name())
            .and_then(|(_, value)| value.number())
            .and_then(|value| u8::try_from(value).ok())
            .expect("a board names its size")
    };
    (size(board_width()), size(board_height()))
}

/// The exchange applying `rate` to `world`'s board: each role bound to the
/// account its rate fixes it to, the cells a latch, move or push names and
/// the puzzle; bound to nothing where `rate` names no instance of `world`'s
/// rates, so that `world` refuses it.
pub fn exchange(world: &World, rate: RateId) -> Action {
    let exchange = Exchange::new(rate, Quantity::new(1));
    let Ok(instance) = world.instance(&rate) else {
        return exchange;
    };
    instance
        .fixed_accounts()
        .fold(exchange, |exchange, (role, account)| {
            exchange.bind(
                *role,
                *account
                    .as_literal()
                    .expect("an instance's accounts are written out"),
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn step(from: u8, to: u8) -> RateId {
        RateId::Move { from, to }
    }

    const fn push(behind: u8, crate_at: u8, to: u8, crate_id: u8) -> RateId {
        RateId::Push {
            behind,
            crate_at,
            to,
            crate_id,
        }
    }

    const fn latch(circuit: u8, at: u8) -> RateId {
        RateId::Latch { circuit, at }
    }

    const MICRO_SOLUTION: [RateId; 3] = [step(8, 9), push(9, 10, 11, 0), RateId::Finish];

    /// Stages crate 0 on the plate, latches the door from the far chamber,
    /// delivers crate 1, retrieves crate 0 and leaves by the exit.
    const SHOWCASE_SOLUTION: [RateId; 23] = [
        push(38, 29, 20, 0),
        step(29, 30),
        step(30, 31),
        step(31, 32),
        step(32, 41),
        step(41, 42),
        latch(0, 42),
        push(42, 33, 24, 1),
        push(33, 24, 15, 1),
        step(24, 33),
        step(33, 32),
        step(32, 31),
        step(31, 30),
        step(30, 29),
        push(29, 20, 11, 0),
        step(20, 21),
        step(21, 30),
        step(30, 31),
        step(31, 32),
        step(32, 33),
        step(33, 42),
        step(42, 51),
        RateId::Finish,
    ];

    /// Both circuits' doors latched, then three crates delivered in turn.
    const STRESS_SOLUTION: [RateId; 40] = [
        push(54, 41, 28, 0),
        step(41, 42),
        step(42, 43),
        step(43, 44),
        step(44, 57),
        step(57, 58),
        latch(0, 58),
        push(58, 45, 32, 1),
        step(45, 46),
        step(46, 47),
        step(47, 48),
        step(48, 61),
        step(61, 62),
        latch(1, 62),
        push(62, 49, 36, 2),
        push(49, 36, 23, 2),
        step(36, 35),
        step(35, 48),
        step(48, 47),
        step(47, 46),
        step(46, 45),
        push(45, 32, 19, 1),
        step(32, 45),
        step(45, 44),
        step(44, 43),
        step(43, 42),
        step(42, 41),
        push(41, 28, 15, 0),
        step(28, 41),
        step(41, 42),
        step(42, 43),
        step(43, 44),
        step(44, 45),
        step(45, 46),
        step(46, 47),
        step(47, 48),
        step(48, 61),
        step(61, 62),
        step(62, 75),
        RateId::Finish,
    ];

    /// `world` after `plan`, each exchange bound to the board as it stands.
    fn played(world: &World, plan: &[RateId]) -> World {
        let mut world = world.clone();
        for rate in plan {
            world
                .apply(exchange(&world, *rate))
                .unwrap_or_else(|error| panic!("{rate:?}: {error}"));
        }
        world
    }

    fn player_cell(world: &World) -> Option<u8> {
        floor_cells(world).find(|cell| {
            !world
                .balance(&AccountId::Cell(*cell), &Asset::Player)
                .is_zero()
        })
    }

    fn is_wall(world: &World, cell: u8) -> bool {
        !world
            .balance(&AccountId::Cell(cell), &Asset::Wall)
            .is_zero()
    }

    fn is_dead_square(world: &World, cell: u8) -> bool {
        !world
            .balance(&AccountId::Cell(cell), &Asset::DeadSquare)
            .is_zero()
    }

    #[test]
    fn solves_by_atomic_three_account_pushes() {
        let stepped = played(&initial(), &MICRO_SOLUTION[..1]);
        let pushed = played(&stepped, &MICRO_SOLUTION[1..2]);
        // One push rewrites all three cells: player, crate and the empty beyond.
        for (cell, before, after) in [
            (9, Asset::Player, Asset::Empty),
            (10, Asset::Crate(0), Asset::Player),
            (11, Asset::Empty, Asset::Crate(0)),
        ] {
            let cell = AccountId::Cell(cell);
            assert_eq!(stepped.balance(&cell, &before).to_u64(), 1, "{cell:?}");
            assert!(pushed.balance(&cell, &before).is_zero(), "{cell:?}");
            assert_eq!(pushed.balance(&cell, &after).to_u64(), 1, "{cell:?}");
        }
        assert!(played(&pushed, &MICRO_SOLUTION[2..]).matches(&goal()));
    }

    #[test]
    fn showcase_has_stable_crates_walls_and_a_solution() {
        let world = initial_showcase();
        assert_eq!(crate_cells(&world).len(), 2);
        assert!((0..63).any(|cell| is_wall(&world, cell)));
        assert!(played(&world, &SHOWCASE_SOLUTION).matches(&goal()));
    }

    #[test]
    fn legal_bad_push_has_replayable_deadlock_evidence() {
        let setup = initial_showcase();
        let final_world = played(
            &setup,
            &[
                push(38, 29, 20, 0),
                step(29, 30),
                step(30, 21),
                push(21, 20, 19, 0),
            ],
        );
        assert!(
            crate_cells(&final_world)
                .iter()
                .any(|(_, cell)| is_dead_square(&final_world, *cell))
        );
        assert!(!final_world.matches(&goal()));
    }

    #[test]
    fn moves_join_adjacent_floors_and_their_own_cells() {
        let world = initial();
        assert!(world.is_applicable(&exchange(&world, step(8, 9))));
        // A move names the cells it joins: bound to another, it is refused.
        let teleport = Exchange::new(step(8, 9), Quantity::new(1))
            .bind(Role::Puzzle, AccountId::Success)
            .bind(Role::From, AccountId::Cell(8))
            .bind(Role::To, AccountId::Cell(11));
        assert!(!world.is_applicable(&teleport));
        // Cells that do not adjoin, or are not both floor, have no move.
        for (from, to) in [(8, 10), (1, 8)] {
            assert!(world.instance(&step(from, to)).is_err(), "{from} → {to}");
        }
    }

    #[test]
    fn solved_puzzle_is_quiescent() {
        let solved = played(&initial(), &MICRO_SOLUTION);
        assert!(
            solved
                .rate_ids()
                .flat_map(|key| solved.instances_of(key))
                .all(|rate| !solved.is_applicable(&exchange(&solved, rate)))
        );
    }

    /// The cells `rows` draws as goals, read from the drawing rather than
    /// the economy.
    fn drawn_goals(rows: &[&str]) -> Vec<AccountId> {
        rows.concat()
            .bytes()
            .enumerate()
            .filter(|(_, tile)| matches!(tile, b'.' | b'*' | b'+'))
            .map(|(cell, _)| AccountId::Cell(u8::try_from(cell).unwrap()))
            .collect()
    }

    #[test]
    fn finishing_is_one_law_fixed_to_the_goal_cells() {
        for (world, rows) in [
            (initial(), MICRO),
            (initial_showcase(), SHOWCASE),
            (initial_stress(), STRESS),
        ] {
            assert_eq!(
                world
                    .rate_ids()
                    .filter_map(|rate| rate.as_literal())
                    .filter(|rate| **rate == RateId::Finish)
                    .count(),
                1
            );
            // Every role it binds is fixed: there is no placement to choose.
            let finish = world.instance(&RateId::Finish).unwrap();
            assert_eq!(finish.roles().count(), finish.fixed_accounts().count());
            let goals: Vec<AccountId> = finish
                .fixed_accounts()
                .filter(|(role, _)| matches!(role, Role::Goal(_)))
                .map(|(_, account)| *account.as_literal().unwrap())
                .collect();
            assert_eq!(goals, drawn_goals(rows));
        }
    }

    #[test]
    fn finishing_needs_every_goal_to_hold_a_crate() {
        let world = initial_showcase();
        let (last, moves) = SHOWCASE_SOLUTION.split_last().expect("a solution finishes");
        assert_eq!(last, &RateId::Finish);
        let before = played(&world, moves);
        assert!(before.is_applicable(&exchange(&before, RateId::Finish)));
        // With crate 0 still staged on the plate, one goal is empty: it is
        // refused, though the player has reached the exit.
        let delivered_one = [
            &SHOWCASE_SOLUTION[..14],
            &[
                step(29, 30),
                step(30, 31),
                step(31, 32),
                step(32, 41),
                step(41, 42),
                step(42, 51),
            ],
        ]
        .concat();
        let short = played(&world, &delivered_one);
        assert_eq!(
            short.balance(&AccountId::Success, &Asset::AtExit).to_u64(),
            1
        );
        assert!(!short.is_applicable(&exchange(&short, RateId::Finish)));
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
    fn two_circuits_compose_and_the_solved_stress_warehouse_latches_both() {
        let final_world = played(&initial_stress(), &STRESS_SOLUTION);
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
