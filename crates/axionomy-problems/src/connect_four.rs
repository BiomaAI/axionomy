//! Compact adversarial Connect Four with encoded gravity and terminal truth.

use axionomy::{
    Account, Basket, Economy, EconomyBuilder, Exchange, LinearInvariant, Quantity, Rate, Trace,
    basket,
};
use axionomy_search::{
    action_source::eager_actions,
    mcts::{MctsConfig, MctsDecision, MctsError, MctsSession, MctsStatus, random_action, search},
    session::{Continue, WorkBudget},
};
use std::ops::ControlFlow;

pub const WIDTH: u8 = 4;
pub const HEIGHT: u8 = 4;
pub const STANDARD_WIDTH: u8 = 7;
pub const STANDARD_HEIGHT: u8 = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Player {
    Red,
    Yellow,
}

impl Player {
    pub const fn other(self) -> Self {
        match self {
            Self::Red => Self::Yellow,
            Self::Yellow => Self::Red,
        }
    }

    const fn index(self) -> usize {
        match self {
            Self::Red => 0,
            Self::Yellow => 1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Line {
    Row(u8),
    Column(u8),
    MainDiagonal,
    AntiDiagonal,
    Segment(u8),
}

pub const LINES: [Line; 10] = [
    Line::Row(0),
    Line::Row(1),
    Line::Row(2),
    Line::Row(3),
    Line::Column(0),
    Line::Column(1),
    Line::Column(2),
    Line::Column(3),
    Line::MainDiagonal,
    Line::AntiDiagonal,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AccountId {
    Game,
    Column(u8),
    Cell { column: u8, row: u8 },
    Result,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Asset {
    GameIdentity,
    ColumnIdentity(u8),
    CellIdentity { column: u8, row: u8 },
    ResultIdentity,
    Empty,
    Piece(Player),
    NextRow(u8),
    ColumnFull,
    Turn(Player),
    LineCount(Player, Line, u8),
    Winner(Player),
    Draw,
    BoardSize { width: u8, height: u8 },
    Pending(Player),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Role {
    Game,
    Column,
    Cell,
    Result,
    Column0,
    Column1,
    Column2,
    Column3,
    Column4,
    Column5,
    Column6,
    Winning0,
    Winning1,
    Winning2,
    Winning3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LineCounts {
    row: u8,
    column: u8,
    main_diagonal: Option<u8>,
    anti_diagonal: Option<u8>,
}

impl LineCounts {
    pub const fn row(self) -> u8 {
        self.row
    }

    pub const fn column(self) -> u8 {
        self.column
    }

    pub const fn main_diagonal(self) -> Option<u8> {
        self.main_diagonal
    }

    pub const fn anti_diagonal(self) -> Option<u8> {
        self.anti_diagonal
    }

    pub fn completes_line(self) -> bool {
        self.row == 3
            || self.column == 3
            || self.main_diagonal == Some(3)
            || self.anti_diagonal == Some(3)
    }

    fn entries(self, column: u8, row: u8) -> Vec<(Line, u8)> {
        let mut entries = vec![
            (Line::Row(row), self.row),
            (Line::Column(column), self.column),
        ];
        if let Some(count) = self.main_diagonal {
            entries.push((Line::MainDiagonal, count));
        }
        if let Some(count) = self.anti_diagonal {
            entries.push((Line::AntiDiagonal, count));
        }
        entries
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RateId {
    Move {
        player: Player,
        column: u8,
        row: u8,
        counts: LineCounts,
    },
    Draw(Player),
    StandardMove {
        player: Player,
        column: u8,
        row: u8,
    },
    ClaimWin {
        player: Player,
        segment: u8,
    },
    Continue(Player),
    StandardDraw(Player),
}

pub type World = Economy<AccountId, Asset, RateId, Role>;
pub type Action = Exchange<RateId, Role, AccountId>;
pub type Decision = MctsDecision<Action>;
pub type DecisionError = MctsError<RateId, Role, AccountId, Asset>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GameProgress {
    moves_completed: usize,
    maximum_moves: usize,
    iterations_completed: usize,
    iterations_per_move: usize,
    nodes: usize,
}

impl GameProgress {
    pub const fn moves_completed(self) -> usize {
        self.moves_completed
    }

    pub const fn maximum_moves(self) -> usize {
        self.maximum_moves
    }

    pub const fn iterations_completed(self) -> usize {
        self.iterations_completed
    }

    pub const fn iterations_per_move(self) -> usize {
        self.iterations_per_move
    }

    pub const fn nodes(self) -> usize {
        self.nodes
    }
}

pub fn initial() -> World {
    let mut game_assets = basket([(Asset::GameIdentity, 1), (Asset::Turn(Player::Red), 1)]);
    for player in [Player::Red, Player::Yellow] {
        for line in LINES {
            game_assets.insert(Asset::LineCount(player, line, 0), Quantity::new(1));
        }
    }

    let mut builder = EconomyBuilder::new()
        .account(AccountId::Game, Account::from(game_assets))
        .account(
            AccountId::Result,
            Account::from(basket([(Asset::ResultIdentity, 1)])),
        );

    for column in 0..WIDTH {
        builder = builder.account(
            AccountId::Column(column),
            Account::from(basket([
                (Asset::ColumnIdentity(column), 1),
                (Asset::NextRow(0), 1),
            ])),
        );
        for row in 0..HEIGHT {
            builder = builder.account(
                AccountId::Cell { column, row },
                Account::from(basket([
                    (Asset::CellIdentity { column, row }, 1),
                    (Asset::Empty, 1),
                ])),
            );
            for player in [Player::Red, Player::Yellow] {
                for counts in count_combinations(column, row) {
                    let rate = move_rate(player, column, row, counts);
                    builder = builder.rate(
                        RateId::Move {
                            player,
                            column,
                            row,
                            counts,
                        },
                        rate,
                    );
                }
            }
        }
    }

    for player in [Player::Red, Player::Yellow] {
        builder = builder.rate(
            RateId::Draw(player),
            Rate::new()
                .preserve(Role::Game, basket([(Asset::GameIdentity, 1)]))
                .consume(Role::Game, basket([(Asset::Turn(player), 1)]))
                .preserve(
                    Role::Column0,
                    basket([(Asset::ColumnIdentity(0), 1), (Asset::ColumnFull, 1)]),
                )
                .preserve(
                    Role::Column1,
                    basket([(Asset::ColumnIdentity(1), 1), (Asset::ColumnFull, 1)]),
                )
                .preserve(
                    Role::Column2,
                    basket([(Asset::ColumnIdentity(2), 1), (Asset::ColumnFull, 1)]),
                )
                .preserve(
                    Role::Column3,
                    basket([(Asset::ColumnIdentity(3), 1), (Asset::ColumnFull, 1)]),
                )
                .preserve(Role::Result, basket([(Asset::ResultIdentity, 1)]))
                .produce(Role::Result, basket([(Asset::Draw, 1)]))
                .distinct(Role::Game, Role::Column0)
                .distinct(Role::Game, Role::Column1)
                .distinct(Role::Game, Role::Column2)
                .distinct(Role::Game, Role::Column3)
                .distinct(Role::Game, Role::Result)
                .distinct(Role::Column0, Role::Column1)
                .distinct(Role::Column0, Role::Column2)
                .distinct(Role::Column0, Role::Column3)
                .distinct(Role::Column0, Role::Result)
                .distinct(Role::Column1, Role::Column2)
                .distinct(Role::Column1, Role::Column3)
                .distinct(Role::Column1, Role::Result)
                .distinct(Role::Column2, Role::Column3)
                .distinct(Role::Column2, Role::Result)
                .distinct(Role::Column3, Role::Result),
        );
    }

    builder
        .invariant(
            LinearInvariant::new("cell occupancy")
                .weight(Asset::Empty, 1)
                .weight(Asset::Piece(Player::Red), 1)
                .weight(Asset::Piece(Player::Yellow), 1),
        )
        .invariant((0..HEIGHT).fold(
            LinearInvariant::new("column progression").weight(Asset::ColumnFull, 1),
            |invariant, row| invariant.weight(Asset::NextRow(row), 1),
        ))
        .invariant(
            LinearInvariant::new("game phase")
                .weight(Asset::Turn(Player::Red), 1)
                .weight(Asset::Turn(Player::Yellow), 1)
                .weight(Asset::Winner(Player::Red), 1)
                .weight(Asset::Winner(Player::Yellow), 1)
                .weight(Asset::Draw, 1),
        )
        .invariant(
            [Player::Red, Player::Yellow]
                .into_iter()
                .flat_map(|player| LINES.into_iter().map(move |line| (player, line)))
                .fold(
                    LinearInvariant::new("line counter tokens"),
                    |invariant, (player, line)| {
                        (0..=4).fold(invariant, |invariant, count| {
                            invariant.weight(Asset::LineCount(player, line, count), 1)
                        })
                    },
                ),
        )
        .build()
        .expect("connect four model is valid")
}

/// A standard 7×6 board using compact move rates and explicit four-cell win
/// certificates. Candidate generation admits `Continue` only when no encoded
/// win certificate is applicable, avoiding the old counter cross-product.
pub fn initial_standard() -> World {
    let mut builder = EconomyBuilder::new()
        .account(
            AccountId::Game,
            Account::from(basket([
                (Asset::GameIdentity, 1),
                (
                    Asset::BoardSize {
                        width: STANDARD_WIDTH,
                        height: STANDARD_HEIGHT,
                    },
                    1,
                ),
                (Asset::Turn(Player::Red), 1),
            ])),
        )
        .account(
            AccountId::Result,
            Account::from(basket([(Asset::ResultIdentity, 1)])),
        );
    for column in 0..STANDARD_WIDTH {
        builder = builder.account(
            AccountId::Column(column),
            Account::from(basket([
                (Asset::ColumnIdentity(column), 1),
                (Asset::NextRow(0), 1),
            ])),
        );
        for row in 0..STANDARD_HEIGHT {
            builder = builder.account(
                AccountId::Cell { column, row },
                Account::from(basket([
                    (Asset::CellIdentity { column, row }, 1),
                    (Asset::Empty, 1),
                ])),
            );
            for player in [Player::Red, Player::Yellow] {
                builder = builder.rate(
                    RateId::StandardMove {
                        player,
                        column,
                        row,
                    },
                    standard_move_rate(player, column, row),
                );
            }
        }
    }
    for player in [Player::Red, Player::Yellow] {
        builder = builder
            .rate(
                RateId::Continue(player),
                Rate::new()
                    .preserve(Role::Game, basket([(Asset::GameIdentity, 1)]))
                    .consume(Role::Game, basket([(Asset::Pending(player), 1)]))
                    .produce(Role::Game, basket([(Asset::Turn(player.other()), 1)])),
            )
            .rate(RateId::StandardDraw(player), standard_draw_rate(player));
        for (segment, cells) in winning_segments().into_iter().enumerate() {
            builder = builder.rate(
                RateId::ClaimWin {
                    player,
                    segment: segment as u8,
                },
                claim_win_rate(player, cells),
            );
        }
    }
    builder
        .invariant(
            LinearInvariant::new("cell occupancy")
                .weight(Asset::Empty, 1)
                .weight(Asset::Piece(Player::Red), 1)
                .weight(Asset::Piece(Player::Yellow), 1),
        )
        .invariant((0..STANDARD_HEIGHT).fold(
            LinearInvariant::new("column progression").weight(Asset::ColumnFull, 1),
            |invariant, row| invariant.weight(Asset::NextRow(row), 1),
        ))
        .invariant(
            LinearInvariant::new("game phase")
                .weight(Asset::Turn(Player::Red), 1)
                .weight(Asset::Turn(Player::Yellow), 1)
                .weight(Asset::Pending(Player::Red), 1)
                .weight(Asset::Pending(Player::Yellow), 1)
                .weight(Asset::Winner(Player::Red), 1)
                .weight(Asset::Winner(Player::Yellow), 1)
                .weight(Asset::Draw, 1),
        )
        .build()
        .expect("standard connect four model is valid")
}

pub fn candidates(world: &World) -> Vec<Action> {
    let (width, height) = board_dimensions(world);
    if width == STANDARD_WIDTH && height == STANDARD_HEIGHT {
        return standard_candidates(world);
    }
    let Some(player) = current_player(world) else {
        return Vec::new();
    };

    let mut actions = Vec::new();
    for column in 0..WIDTH {
        if let Some(row) = next_row(world, column) {
            let counts = current_counts(world, player, column, row);
            actions.push(action(RateId::Move {
                player,
                column,
                row,
                counts,
            }));
        }
    }
    if actions.is_empty() {
        actions.push(action(RateId::Draw(player)));
    }
    world.applicable(actions)
}

fn standard_candidates(world: &World) -> Vec<Action> {
    if let Some(player) = pending_player(world) {
        let claims = world.applicable(winning_segments().into_iter().enumerate().map(
            |(segment, _)| {
                action(RateId::ClaimWin {
                    player,
                    segment: segment as u8,
                })
            },
        ));
        if !claims.is_empty() {
            return claims;
        }
        if (0..STANDARD_WIDTH).all(|column| {
            !world
                .balance(&AccountId::Column(column), &Asset::ColumnFull)
                .is_zero()
        }) {
            return world.applicable([action(RateId::StandardDraw(player))]);
        }
        return world.applicable([action(RateId::Continue(player))]);
    }
    let Some(player) = current_player(world) else {
        return Vec::new();
    };
    world.applicable((0..STANDARD_WIDTH).filter_map(|column| {
        next_row_bounded(world, column, STANDARD_HEIGHT).map(|row| {
            action(RateId::StandardMove {
                player,
                column,
                row,
            })
        })
    }))
}

pub fn terminal_values(world: &World) -> Option<Vec<f64>> {
    if !world
        .balance(&AccountId::Result, &Asset::Winner(Player::Red))
        .is_zero()
    {
        Some(vec![1.0, 0.0])
    } else if !world
        .balance(&AccountId::Result, &Asset::Winner(Player::Yellow))
        .is_zero()
    {
        Some(vec![0.0, 1.0])
    } else if !world.balance(&AccountId::Result, &Asset::Draw).is_zero() {
        Some(vec![0.5, 0.5])
    } else {
        None
    }
}

pub fn mcts(world: &World, iterations: usize, seed: u64) -> Result<Decision, DecisionError> {
    search(
        world,
        MctsConfig::new(iterations, 20).with_seed(seed),
        2,
        candidates,
        |_| Vec::new(),
        |world| current_player(world).map_or(0, Player::index),
        terminal_values,
        |_| vec![0.5, 0.5],
        random_action,
    )
}

pub fn play_game(iterations_per_move: usize, seed: u64) -> Trace<RateId, Role, AccountId> {
    play_game_with_progress(
        iterations_per_move,
        seed,
        iterations_per_move.max(1),
        |_| ControlFlow::Continue(()),
    )
    .expect("uninterrupted MCTS play produces a complete trace")
}

/// Plays a complete game through bounded MCTS advances.
///
/// The observer runs between deterministic iteration chunks. Returning
/// `Break` leaves the partial game disposable and returns `None`; every move
/// already accepted into the local trace remains independently replayable.
pub fn play_game_with_progress(
    iterations_per_move: usize,
    seed: u64,
    chunk_size: usize,
    observer: impl FnMut(GameProgress) -> ControlFlow<()>,
) -> Option<Trace<RateId, Role, AccountId>> {
    play_from_with_progress(
        initial(),
        usize::from(WIDTH) * usize::from(HEIGHT) + 1,
        iterations_per_move,
        seed,
        chunk_size,
        observer,
    )
}

pub fn play_standard_game_with_progress(
    iterations_per_action: usize,
    seed: u64,
    chunk_size: usize,
    observer: impl FnMut(GameProgress) -> ControlFlow<()>,
) -> Option<Trace<RateId, Role, AccountId>> {
    play_from_with_progress(
        initial_standard(),
        usize::from(STANDARD_WIDTH) * usize::from(STANDARD_HEIGHT) * 2 + 1,
        iterations_per_action,
        seed,
        chunk_size,
        observer,
    )
}

fn play_from_with_progress(
    mut world: World,
    maximum_moves: usize,
    iterations_per_move: usize,
    seed: u64,
    chunk_size: usize,
    mut observer: impl FnMut(GameProgress) -> ControlFlow<()>,
) -> Option<Trace<RateId, Role, AccountId>> {
    let mut trace = Trace::new();
    for turn in 0..maximum_moves {
        if terminal_values(&world).is_some() {
            break;
        }
        let config = MctsConfig::new(iterations_per_move, maximum_moves)
            .with_seed(seed + u64::try_from(turn).unwrap_or(u64::MAX));
        let mut session = MctsSession::new(
            &world,
            config,
            2,
            eager_actions(candidates),
            |_| Vec::new(),
            |world| current_player(world).map_or(0, Player::index),
            terminal_values,
            |_| vec![0.5, 0.5],
            random_action,
        )
        .ok()?;
        while session.status() == MctsStatus::Running {
            let report = session
                .advance(WorkBudget::new(chunk_size.max(1)), &mut Continue)
                .ok()?;
            let progress = *report.progress();
            if observer(GameProgress {
                moves_completed: turn,
                maximum_moves,
                iterations_completed: progress.iterations(),
                iterations_per_move,
                nodes: progress.nodes(),
            })
            .is_break()
            {
                return None;
            }
        }
        let decision = session.into_decision()?;
        let exchange = decision.action().clone();
        world
            .apply(exchange.clone())
            .expect("MCTS returns an applicable exchange");
        trace.push(exchange);
    }
    Some(trace)
}

pub fn column_of(action: &Action) -> Option<u8> {
    match action.rate() {
        RateId::Move { column, .. } | RateId::StandardMove { column, .. } => Some(*column),
        RateId::Draw(_)
        | RateId::ClaimWin { .. }
        | RateId::Continue(_)
        | RateId::StandardDraw(_) => None,
    }
}

fn move_rate(player: Player, column: u8, row: u8, counts: LineCounts) -> Rate<Role, Asset> {
    let mut consumed = basket([(Asset::Turn(player), 1)]);
    let mut produced = Basket::new();
    for (line, count) in counts.entries(column, row) {
        consumed.insert(Asset::LineCount(player, line, count), Quantity::new(1));
        produced.insert(Asset::LineCount(player, line, count + 1), Quantity::new(1));
    }

    let winning = counts.completes_line();
    if !winning {
        produced.insert(Asset::Turn(player.other()), Quantity::new(1));
    }

    let rate = Rate::new()
        .preserve(Role::Game, basket([(Asset::GameIdentity, 1)]))
        .consume(Role::Game, consumed)
        .produce(Role::Game, produced)
        .preserve(Role::Column, basket([(Asset::ColumnIdentity(column), 1)]))
        .consume(Role::Column, basket([(Asset::NextRow(row), 1)]))
        .produce(
            Role::Column,
            if row + 1 == HEIGHT {
                basket([(Asset::ColumnFull, 1)])
            } else {
                basket([(Asset::NextRow(row + 1), 1)])
            },
        )
        .preserve(
            Role::Cell,
            basket([(Asset::CellIdentity { column, row }, 1)]),
        )
        .consume(Role::Cell, basket([(Asset::Empty, 1)]))
        .produce(Role::Cell, basket([(Asset::Piece(player), 1)]))
        .distinct(Role::Game, Role::Column)
        .distinct(Role::Game, Role::Cell)
        .distinct(Role::Column, Role::Cell);

    if winning {
        rate.preserve(Role::Result, basket([(Asset::ResultIdentity, 1)]))
            .produce(Role::Result, basket([(Asset::Winner(player), 1)]))
            .distinct(Role::Game, Role::Result)
            .distinct(Role::Column, Role::Result)
            .distinct(Role::Cell, Role::Result)
    } else {
        rate
    }
}

fn standard_move_rate(player: Player, column: u8, row: u8) -> Rate<Role, Asset> {
    Rate::new()
        .preserve(Role::Game, basket([(Asset::GameIdentity, 1)]))
        .consume(Role::Game, basket([(Asset::Turn(player), 1)]))
        .produce(Role::Game, basket([(Asset::Pending(player), 1)]))
        .preserve(Role::Column, basket([(Asset::ColumnIdentity(column), 1)]))
        .consume(Role::Column, basket([(Asset::NextRow(row), 1)]))
        .produce(
            Role::Column,
            if row + 1 == STANDARD_HEIGHT {
                basket([(Asset::ColumnFull, 1)])
            } else {
                basket([(Asset::NextRow(row + 1), 1)])
            },
        )
        .preserve(
            Role::Cell,
            basket([(Asset::CellIdentity { column, row }, 1)]),
        )
        .consume(Role::Cell, basket([(Asset::Empty, 1)]))
        .produce(Role::Cell, basket([(Asset::Piece(player), 1)]))
        .distinct(Role::Game, Role::Column)
        .distinct(Role::Game, Role::Cell)
        .distinct(Role::Column, Role::Cell)
}

fn claim_win_rate(player: Player, cells: [(u8, u8); 4]) -> Rate<Role, Asset> {
    let roles = [
        Role::Winning0,
        Role::Winning1,
        Role::Winning2,
        Role::Winning3,
    ];
    let mut rate = Rate::new()
        .preserve(Role::Game, basket([(Asset::GameIdentity, 1)]))
        .consume(Role::Game, basket([(Asset::Pending(player), 1)]))
        .preserve(Role::Result, basket([(Asset::ResultIdentity, 1)]))
        .produce(Role::Result, basket([(Asset::Winner(player), 1)]))
        .distinct(Role::Game, Role::Result);
    for (role, (column, row)) in roles.into_iter().zip(cells) {
        rate = rate
            .preserve(
                role,
                basket([
                    (Asset::CellIdentity { column, row }, 1),
                    (Asset::Piece(player), 1),
                ]),
            )
            .distinct(Role::Game, role)
            .distinct(Role::Result, role);
    }
    for (index, left) in roles.iter().enumerate() {
        for right in &roles[index + 1..] {
            rate = rate.distinct(*left, *right);
        }
    }
    rate
}

fn standard_draw_rate(player: Player) -> Rate<Role, Asset> {
    let roles = [
        Role::Column0,
        Role::Column1,
        Role::Column2,
        Role::Column3,
        Role::Column4,
        Role::Column5,
        Role::Column6,
    ];
    let mut rate = Rate::new()
        .preserve(Role::Game, basket([(Asset::GameIdentity, 1)]))
        .consume(Role::Game, basket([(Asset::Pending(player), 1)]))
        .preserve(Role::Result, basket([(Asset::ResultIdentity, 1)]))
        .produce(Role::Result, basket([(Asset::Draw, 1)]))
        .distinct(Role::Game, Role::Result);
    for (column, role) in roles.into_iter().enumerate() {
        rate = rate
            .preserve(
                role,
                basket([
                    (Asset::ColumnIdentity(column as u8), 1),
                    (Asset::ColumnFull, 1),
                ]),
            )
            .distinct(Role::Game, role)
            .distinct(Role::Result, role);
    }
    for (index, left) in roles.iter().enumerate() {
        for right in &roles[index + 1..] {
            rate = rate.distinct(*left, *right);
        }
    }
    rate
}

fn winning_segments() -> Vec<[(u8, u8); 4]> {
    let mut segments = Vec::with_capacity(69);
    for row in 0..STANDARD_HEIGHT {
        for column in 0..=STANDARD_WIDTH - 4 {
            segments.push([
                (column, row),
                (column + 1, row),
                (column + 2, row),
                (column + 3, row),
            ]);
        }
    }
    for column in 0..STANDARD_WIDTH {
        for row in 0..=STANDARD_HEIGHT - 4 {
            segments.push([
                (column, row),
                (column, row + 1),
                (column, row + 2),
                (column, row + 3),
            ]);
        }
    }
    for column in 0..=STANDARD_WIDTH - 4 {
        for row in 0..=STANDARD_HEIGHT - 4 {
            segments.push([
                (column, row),
                (column + 1, row + 1),
                (column + 2, row + 2),
                (column + 3, row + 3),
            ]);
            segments.push([
                (column, row + 3),
                (column + 1, row + 2),
                (column + 2, row + 1),
                (column + 3, row),
            ]);
        }
    }
    segments
}

fn action(rate: RateId) -> Action {
    let exchange = Exchange::new(rate, Quantity::new(1));
    match rate {
        RateId::Move {
            column,
            row,
            counts,
            ..
        } => {
            let exchange = exchange
                .bind(Role::Game, AccountId::Game)
                .bind(Role::Column, AccountId::Column(column))
                .bind(Role::Cell, AccountId::Cell { column, row });
            if counts.completes_line() {
                exchange.bind(Role::Result, AccountId::Result)
            } else {
                exchange
            }
        }
        RateId::Draw(_) => exchange
            .bind(Role::Game, AccountId::Game)
            .bind(Role::Column0, AccountId::Column(0))
            .bind(Role::Column1, AccountId::Column(1))
            .bind(Role::Column2, AccountId::Column(2))
            .bind(Role::Column3, AccountId::Column(3))
            .bind(Role::Result, AccountId::Result),
        RateId::StandardMove { column, row, .. } => exchange
            .bind(Role::Game, AccountId::Game)
            .bind(Role::Column, AccountId::Column(column))
            .bind(Role::Cell, AccountId::Cell { column, row }),
        RateId::ClaimWin { segment, .. } => {
            let cells = winning_segments()[usize::from(segment)];
            [
                Role::Winning0,
                Role::Winning1,
                Role::Winning2,
                Role::Winning3,
            ]
            .into_iter()
            .zip(cells)
            .fold(
                exchange
                    .bind(Role::Game, AccountId::Game)
                    .bind(Role::Result, AccountId::Result),
                |exchange, (role, (column, row))| {
                    exchange.bind(role, AccountId::Cell { column, row })
                },
            )
        }
        RateId::Continue(_) => exchange.bind(Role::Game, AccountId::Game),
        RateId::StandardDraw(_) => [
            Role::Column0,
            Role::Column1,
            Role::Column2,
            Role::Column3,
            Role::Column4,
            Role::Column5,
            Role::Column6,
        ]
        .into_iter()
        .enumerate()
        .fold(
            exchange
                .bind(Role::Game, AccountId::Game)
                .bind(Role::Result, AccountId::Result),
            |exchange, (column, role)| exchange.bind(role, AccountId::Column(column as u8)),
        ),
    }
}

fn current_player(world: &World) -> Option<Player> {
    [Player::Red, Player::Yellow].into_iter().find(|player| {
        !world
            .balance(&AccountId::Game, &Asset::Turn(*player))
            .is_zero()
    })
}

fn pending_player(world: &World) -> Option<Player> {
    [Player::Red, Player::Yellow].into_iter().find(|player| {
        !world
            .balance(&AccountId::Game, &Asset::Pending(*player))
            .is_zero()
    })
}

fn next_row(world: &World, column: u8) -> Option<u8> {
    next_row_bounded(world, column, HEIGHT)
}

fn next_row_bounded(world: &World, column: u8, height: u8) -> Option<u8> {
    (0..height).find(|row| {
        !world
            .balance(&AccountId::Column(column), &Asset::NextRow(*row))
            .is_zero()
    })
}

pub fn board_dimensions(world: &World) -> (u8, u8) {
    if !world
        .balance(
            &AccountId::Game,
            &Asset::BoardSize {
                width: STANDARD_WIDTH,
                height: STANDARD_HEIGHT,
            },
        )
        .is_zero()
    {
        (STANDARD_WIDTH, STANDARD_HEIGHT)
    } else {
        (WIDTH, HEIGHT)
    }
}

fn current_counts(world: &World, player: Player, column: u8, row: u8) -> LineCounts {
    LineCounts {
        row: line_count(world, player, Line::Row(row)),
        column: line_count(world, player, Line::Column(column)),
        main_diagonal: (column == row).then(|| line_count(world, player, Line::MainDiagonal)),
        anti_diagonal: (column + row + 1 == WIDTH)
            .then(|| line_count(world, player, Line::AntiDiagonal)),
    }
}

fn line_count(world: &World, player: Player, line: Line) -> u8 {
    (0..=4)
        .find(|count| {
            !world
                .balance(&AccountId::Game, &Asset::LineCount(player, line, *count))
                .is_zero()
        })
        .expect("every line owns one encoded counter")
}

fn count_combinations(column: u8, row: u8) -> Vec<LineCounts> {
    let diagonal = if column == row {
        (0..4).map(Some).collect::<Vec<_>>()
    } else {
        vec![None]
    };
    let anti_diagonal = if column + row + 1 == WIDTH {
        (0..4).map(Some).collect::<Vec<_>>()
    } else {
        vec![None]
    };
    let mut combinations = Vec::new();
    for row_count in 0..4 {
        for column_count in 0..4 {
            for main_diagonal in &diagonal {
                for anti_diagonal in &anti_diagonal {
                    combinations.push(LineCounts {
                        row: row_count,
                        column: column_count,
                        main_diagonal: *main_diagonal,
                        anti_diagonal: *anti_diagonal,
                    });
                }
            }
        }
    }
    combinations
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    struct PlainBoard {
        cells: [[Option<Player>; HEIGHT as usize]; WIDTH as usize],
        heights: [u8; WIDTH as usize],
        turn: Player,
    }

    impl PlainBoard {
        const fn new() -> Self {
            Self {
                cells: [[None; HEIGHT as usize]; WIDTH as usize],
                heights: [0; WIDTH as usize],
                turn: Player::Red,
            }
        }

        fn legal_columns(self) -> Vec<u8> {
            if self.winner().is_some() {
                return Vec::new();
            }
            (0..WIDTH)
                .filter(|column| self.heights[usize::from(*column)] < HEIGHT)
                .collect()
        }

        fn played(mut self, column: u8) -> Self {
            let index = usize::from(column);
            let row = self.heights[index];
            self.cells[index][usize::from(row)] = Some(self.turn);
            self.heights[index] += 1;
            self.turn = self.turn.other();
            self
        }

        fn winner(self) -> Option<Player> {
            [Player::Red, Player::Yellow]
                .into_iter()
                .find(|player| plain_has_line(&self.cells, *player))
        }

        fn full(self) -> bool {
            self.heights.into_iter().all(|height| height == HEIGHT)
        }
    }

    fn plain_has_line(
        cells: &[[Option<Player>; HEIGHT as usize]; WIDTH as usize],
        player: Player,
    ) -> bool {
        (0..HEIGHT).any(|row| {
            (0..WIDTH).all(|column| cells[usize::from(column)][usize::from(row)] == Some(player))
        }) || (0..WIDTH).any(|column| {
            (0..HEIGHT).all(|row| cells[usize::from(column)][usize::from(row)] == Some(player))
        }) || (0..WIDTH).all(|index| cells[usize::from(index)][usize::from(index)] == Some(player))
            || (0..WIDTH).all(|column| {
                cells[usize::from(column)][usize::from(HEIGHT - 1 - column)] == Some(player)
            })
    }

    #[test]
    fn gravity_and_line_counts_are_core_validated() {
        let mut world = initial();
        let first = candidates(&world)
            .into_iter()
            .find(|action| column_of(action) == Some(0))
            .expect("column zero is open");
        world.apply(first).expect("first piece lands");

        assert_eq!(
            world.balance(
                &AccountId::Cell { column: 0, row: 0 },
                &Asset::Piece(Player::Red),
            ),
            Quantity::new(1)
        );
        assert_eq!(
            world.balance(&AccountId::Column(0), &Asset::NextRow(1)),
            Quantity::new(1)
        );
        assert_eq!(
            world.balance(
                &AccountId::Game,
                &Asset::LineCount(Player::Red, Line::Row(0), 1),
            ),
            Quantity::new(1)
        );
    }

    #[test]
    fn encoded_coordinates_reject_misbinding_an_otherwise_empty_cell() {
        let world = initial();
        let counts = current_counts(&world, Player::Red, 0, 0);
        let misplaced = Exchange::new(
            RateId::Move {
                player: Player::Red,
                column: 0,
                row: 0,
                counts,
            },
            Quantity::new(1),
        )
        .bind(Role::Game, AccountId::Game)
        .bind(Role::Column, AccountId::Column(3))
        .bind(Role::Cell, AccountId::Cell { column: 3, row: 3 });

        assert!(!world.is_applicable(&misplaced));
    }

    #[test]
    fn mcts_selects_an_immediate_encoded_win() {
        let mut world = initial();
        let mut plain = PlainBoard::new();
        for column in [0, 0, 1, 1, 2, 2] {
            let exchange = candidates(&world)
                .into_iter()
                .find(|action| column_of(action) == Some(column))
                .expect("scripted column is open");
            world.apply(exchange).expect("scripted move is valid");
            plain = plain.played(column);
        }

        assert_eq!(minimax_best_column(plain), Some(3));
        let decision = mcts(&world, 512, 19).expect("red can choose a move");
        assert_eq!(column_of(decision.action()), Some(3));
        let mut won = world.fork();
        won.apply(decision.action().clone())
            .expect("winning move applies");
        assert_eq!(terminal_values(&won), Some(vec![1.0, 0.0]));
    }

    #[test]
    fn complete_mcts_game_is_replayable() {
        let initial = initial();
        let trace = play_game(96, 5);
        let final_world = initial
            .replayed(&trace)
            .expect("the complete game must replay");

        assert!(terminal_values(&final_world).is_some());
        assert!(candidates(&final_world).is_empty());
        assert!(trace.exchanges().len() <= usize::from(WIDTH * HEIGHT + 1));
    }

    #[test]
    fn generated_prefixes_match_a_plain_board_oracle() {
        compare_prefixes(initial(), PlainBoard::new(), 0, 5);
    }

    #[test]
    fn standard_board_uses_compact_rates_and_core_validated_win_certificates() {
        let mut world = initial_standard();
        assert_eq!(board_dimensions(&world), (7, 6));
        assert!(world.rate_ids().count() < 300);

        for (index, column) in [0, 0, 1, 1, 2, 2, 3].into_iter().enumerate() {
            let movement = candidates(&world)
                .into_iter()
                .find(|candidate| column_of(candidate) == Some(column))
                .expect("scripted standard-board column is legal");
            world.apply(movement).expect("piece placement applies");
            let adjudication = candidates(&world);
            if index == 6 {
                assert!(matches!(
                    adjudication[0].rate(),
                    RateId::ClaimWin {
                        player: Player::Red,
                        ..
                    }
                ));
            }
            world
                .apply(adjudication[0].clone())
                .expect("continue or win certificate applies");
        }
        assert_eq!(terminal_values(&world), Some(vec![1.0, 0.0]));
        assert!(candidates(&world).is_empty());
    }

    fn compare_prefixes(world: World, plain: PlainBoard, depth: usize, limit: usize) {
        let mut encoded_columns = candidates(&world)
            .iter()
            .filter_map(column_of)
            .collect::<Vec<_>>();
        encoded_columns.sort_unstable();
        assert_eq!(encoded_columns, plain.legal_columns());
        assert_eq!(
            terminal_values(&world),
            plain.winner().map(|winner| match winner {
                Player::Red => vec![1.0, 0.0],
                Player::Yellow => vec![0.0, 1.0],
            })
        );
        if depth == limit || plain.winner().is_some() || plain.full() {
            return;
        }
        for column in encoded_columns {
            let exchange = candidates(&world)
                .into_iter()
                .find(|action| column_of(action) == Some(column))
                .expect("oracle column is encoded");
            let mut next = world.fork();
            next.apply(exchange).expect("generated prefix is valid");
            compare_prefixes(next, plain.played(column), depth + 1, limit);
        }
    }

    fn minimax_best_column(board: PlainBoard) -> Option<u8> {
        let player = board.turn;
        let mut cache = HashMap::new();
        board
            .legal_columns()
            .into_iter()
            .max_by_key(|column| minimax_score(board.played(*column), player, &mut cache))
    }

    fn minimax_score(
        board: PlainBoard,
        maximizing: Player,
        cache: &mut HashMap<PlainBoard, i8>,
    ) -> i8 {
        if let Some(winner) = board.winner() {
            return if winner == maximizing { 1 } else { -1 };
        }
        if board.full() {
            return 0;
        }
        if let Some(score) = cache.get(&board) {
            return *score;
        }
        let score = if board.turn == maximizing {
            board
                .legal_columns()
                .into_iter()
                .map(|column| minimax_score(board.played(column), maximizing, cache))
                .max()
                .expect("non-terminal board has a move")
        } else {
            board
                .legal_columns()
                .into_iter()
                .map(|column| minimax_score(board.played(column), maximizing, cache))
                .min()
                .expect("non-terminal board has a move")
        };
        cache.insert(board, score);
        score
    }
}
