//! Read-only board reasoning. Every suggested move is still a core exchange.
use super::*;
use axionomy_search::sampling::{SeededSampler, TicketSource};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TacticalEvidence {
    pub player: Option<Player>,
    pub winning_columns: Vec<u8>,
    pub opponent_winning_columns: Vec<u8>,
    pub safe_columns: Vec<u8>,
    pub fork_columns: Vec<u8>,
}

#[derive(Clone, Copy)]
struct Board {
    pieces: [u64; 2],
    heights: [u8; 7],
    width: u8,
    height: u8,
}

impl Board {
    fn read(world: &World) -> Self {
        let (width, height) = board_dimensions(world);
        let mut board = Self {
            pieces: [0; 2],
            heights: [0; 7],
            width,
            height,
        };
        for column in 0..width {
            for row in 0..height {
                for player in [Player::Red, Player::Yellow] {
                    if !world
                        .balance(&AccountId::Cell { column, row }, &Asset::Piece(player))
                        .is_zero()
                    {
                        board.pieces[player.index()] |= 1 << (column * 7 + row);
                        board.heights[column as usize] = row + 1;
                    }
                }
            }
        }
        board
    }
    fn columns(self) -> Vec<u8> {
        (0..self.width)
            .filter(|column| self.heights[*column as usize] < self.height)
            .collect()
    }
    fn played(mut self, player: Player, column: u8) -> Self {
        let row = self.heights[column as usize];
        self.pieces[player.index()] |= 1 << (column * 7 + row);
        self.heights[column as usize] += 1;
        self
    }
    fn won(self, player: Player) -> bool {
        let bits = self.pieces[player.index()];
        [1, 7, 6, 8].into_iter().any(|shift| {
            let pairs = bits & (bits >> shift);
            pairs & (pairs >> (shift * 2)) != 0
        })
    }
    fn wins(self, player: Player) -> Vec<u8> {
        self.columns()
            .into_iter()
            .filter(|column| self.played(player, *column).won(player))
            .collect()
    }
    fn evidence(self, player: Player) -> TacticalEvidence {
        let winning_columns = self.wins(player);
        let opponent_winning_columns = self.wins(player.other());
        let safe_columns: Vec<_> = self
            .columns()
            .into_iter()
            .filter(|column| {
                let next = self.played(player, *column);
                next.won(player) || next.wins(player.other()).is_empty()
            })
            .collect();
        let fork_columns = safe_columns
            .iter()
            .copied()
            .filter(|column| {
                let next = self.played(player, *column);
                !next.won(player) && next.wins(player).len() >= 2
            })
            .collect();
        TacticalEvidence {
            player: Some(player),
            winning_columns,
            opponent_winning_columns,
            safe_columns,
            fork_columns,
        }
    }
    fn score(self, player: Player) -> i32 {
        if self.won(player) {
            return 100_000;
        }
        if self.won(player.other()) {
            return -100_000;
        }
        let mut score = 0;
        // Count open four-cell windows, including gravity-inaccessible future threats.
        for column in 0..self.width as i32 {
            for row in 0..self.height as i32 {
                for (dx, dy) in [(1, 0), (0, 1), (1, 1), (1, -1)] {
                    let end = (column + 3 * dx, row + 3 * dy);
                    if end.0 < 0
                        || end.0 >= self.width as i32
                        || end.1 < 0
                        || end.1 >= self.height as i32
                    {
                        continue;
                    }
                    let mask = (0..4).fold(0, |mask, i| {
                        mask | 1 << ((column + i * dx) * 7 + row + i * dy)
                    });
                    let own = (self.pieces[player.index()] & mask).count_ones();
                    let enemy = (self.pieces[player.other().index()] & mask).count_ones();
                    if enemy == 0 {
                        score += [0, 1, 8, 60, 100_000][own as usize];
                    }
                    if own == 0 {
                        score -= [0, 1, 8, 60, 100_000][enemy as usize];
                    }
                }
            }
        }
        score
    }
}

pub fn tactical_evidence(world: &World) -> TacticalEvidence {
    if terminal_values(world).is_some() {
        return TacticalEvidence {
            player: None,
            winning_columns: vec![],
            opponent_winning_columns: vec![],
            safe_columns: vec![],
            fork_columns: vec![],
        };
    }
    let player = current_player(world);
    player
        .map(|player| Board::read(world).evidence(player))
        .unwrap_or(TacticalEvidence {
            player: None,
            winning_columns: vec![],
            opponent_winning_columns: vec![],
            safe_columns: vec![],
            fork_columns: vec![],
        })
}

/// Proven wins, double threats, and one-ply loss avoidance prune only the
/// policy's proposals. `candidates` continues to expose every legal move.
pub fn tactical_candidates(world: &World) -> Vec<Action> {
    let legal = candidates(world);
    if current_player(world).is_none() {
        return legal;
    }
    let evidence = tactical_evidence(world);
    let preferred = if !evidence.winning_columns.is_empty() {
        &evidence.winning_columns
    } else if !evidence.fork_columns.is_empty() {
        &evidence.fork_columns
    } else {
        &evidence.safe_columns
    };
    if preferred.is_empty() {
        return legal;
    }
    legal
        .into_iter()
        .filter(|action| column_of(action).is_none_or(|column| preferred.contains(&column)))
        .collect()
}

pub(super) fn cutoff(world: &World) -> Vec<f64> {
    if let Some(terminal) = terminal_values(world) {
        return terminal;
    }
    let score = Board::read(world).score(Player::Red) as f64;
    let red = 0.5 + 0.45 * score / (score.abs() + 120.0);
    vec![red, 1.0 - red]
}

pub(super) fn rollout(
    world: &World,
    actions: &[Action],
    random: &mut SeededSampler,
) -> Option<Action> {
    if actions.is_empty() {
        return None;
    }
    let player = current_player(world);
    if actions.len() == 1 || player.is_none() {
        return Some(actions[0].clone());
    }
    // Retain exploration, with positional preference on the remaining moves.
    if random.ticket(4) == 0 {
        return Some(actions[random.ticket(actions.len() as u64) as usize].clone());
    }
    let player = player.unwrap();
    let board = Board::read(world);
    actions
        .iter()
        .max_by_key(|action| {
            column_of(action).map_or(0, |column| {
                board.played(player, column).score(player) * 8
                    - (column as i32 * 2 - board.width as i32 + 1).abs()
            })
        })
        .cloned()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Opponent {
    CenterFirst,
    AlphaBeta,
}

fn negamax(board: Board, player: Player, depth: u8, mut alpha: i32, beta: i32) -> i32 {
    if board.won(player.other()) {
        return -100_000 - depth as i32;
    }
    let mut columns = board.columns();
    if columns.is_empty() {
        return 0;
    }
    if depth == 0 {
        return board.score(player);
    }
    columns.sort_by_key(|column| (*column as i32 * 2 - board.width as i32 + 1).abs());
    let mut best = i32::MIN;
    for column in columns {
        let value = -negamax(
            board.played(player, column),
            player.other(),
            depth - 1,
            -beta,
            -alpha,
        );
        best = best.max(value);
        alpha = alpha.max(value);
        if alpha >= beta {
            break;
        }
    }
    best
}

/// Distinct opponents share the exact same starting board and legal exchanges.
/// Red uses tactical MCTS; Yellow uses a positional baseline or depth-five search.
pub fn play_opponent_game(
    iterations: usize,
    seed: u64,
    opponent: Opponent,
) -> Option<Trace<Ontology>> {
    play_opponent_game_with_progress(iterations, seed, opponent, |_| ControlFlow::Continue(()))
}

pub fn play_opponent_game_with_progress(
    iterations: usize,
    seed: u64,
    opponent: Opponent,
    observer: impl FnMut(usize) -> ControlFlow<()>,
) -> Option<Trace<Ontology>> {
    play_opponent_game_from_with_progress(initial_standard(), iterations, seed, opponent, observer)
}

pub fn play_opponent_game_from_with_progress(
    mut world: World,
    iterations: usize,
    seed: u64,
    opponent: Opponent,
    mut observer: impl FnMut(usize) -> ControlFlow<()>,
) -> Option<Trace<Ontology>> {
    let mut trace = Trace::new();
    for turn in 0..85 {
        if observer(turn as usize).is_break() {
            return None;
        }
        if terminal_values(&world).is_some() {
            return Some(trace);
        }
        let legal = candidates(&world);
        let exchange = if current_player(&world) == Some(Player::Red) {
            mcts(&world, iterations, seed.wrapping_add(turn))
                .ok()?
                .action()
                .clone()
        } else if let Some(player) = current_player(&world) {
            let board = Board::read(&world);
            legal.into_iter().max_by_key(|action| {
                let column = column_of(action).unwrap();
                match opponent {
                    Opponent::CenterFirst => -(column as i32 * 2 - board.width as i32 + 1).abs(),
                    Opponent::AlphaBeta => -negamax(
                        board.played(player, column),
                        player.other(),
                        4,
                        -200_000,
                        200_000,
                    ),
                }
            })?
        } else {
            legal.first()?.clone()
        };
        world.apply(exchange.clone()).ok()?;
        trace.push(exchange);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    fn position(columns: &[u8]) -> World {
        let mut world = initial_standard();
        for &column in columns {
            let drop = candidates(&world)
                .into_iter()
                .find(|action| column_of(action) == Some(column))
                .unwrap();
            world.apply(drop).unwrap();
            world.apply(candidates(&world).remove(0)).unwrap();
        }
        world
    }
    #[test]
    fn tactics_prove_win_block_and_double_threat_even_at_one_iteration() {
        for (columns, expected) in [
            (&[0, 6, 1, 6, 2, 5][..], 3),
            (&[0, 1, 0, 1, 2, 1][..], 1),
            (&[1, 6, 2, 6][..], 3),
        ] {
            let world = position(columns);
            assert_eq!(
                column_of(mcts(&world, 1, 17).unwrap().action()),
                Some(expected)
            );
        }
        let fork = tactical_evidence(&position(&[1, 6, 2, 6]));
        assert!(fork.winning_columns.is_empty());
        assert_eq!(fork.fork_columns, [3]);
    }
    #[test]
    fn tactical_forecasts_agree_with_core_win_certificates() {
        for columns in [
            &[0, 6, 1, 6, 2, 5][..],
            &[1, 6, 2, 6][..],
            &[0, 1, 0, 1, 2, 1][..],
        ] {
            let world = position(columns);
            let evidence = tactical_evidence(&world);
            for action in candidates(&world) {
                let column = column_of(&action).unwrap();
                let mut next = world.clone();
                next.apply(action).unwrap();
                let wins = candidates(&next)
                    .iter()
                    .any(|action| matches!(action.rate(), RateId::ClaimWin { .. }));
                assert_eq!(evidence.winning_columns.contains(&column), wins);
            }
        }
    }
    #[test]
    fn distinct_opponents_produce_deterministic_terminal_replays() {
        let mut traces = Vec::new();
        for opponent in [Opponent::CenterFirst, Opponent::AlphaBeta] {
            let trace = play_opponent_game(2, 17, opponent).unwrap();
            let replay = initial_standard().replayed(&trace).unwrap();
            assert!(terminal_values(&replay).is_some());
            assert_eq!(
                trace.exchanges(),
                play_opponent_game(2, 17, opponent).unwrap().exchanges()
            );
            traces.push(trace);
        }
        assert_ne!(traces[0].exchanges(), traces[1].exchanges());
        assert!(
            play_opponent_game_with_progress(
                2,
                17,
                Opponent::AlphaBeta,
                |_| ControlFlow::Break(())
            )
            .is_none()
        );
    }
}
