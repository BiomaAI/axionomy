//! Axionomy, explained without sound: an economy's accounts, rates and
//! exchanges, then what that gives an agent (value, utility, simulation,
//! training), then who can drive it and why its runs can be trusted.
//!
//! Every number on screen comes from [`facts::Facts`], read from the engine
//! when the video is made. Check the video with the CLI in `main.rs`
//! (`cargo run --release -- --help`).

pub mod facts;

use facts::{Facts, Holding, Law, Outcome};
use fframes::{
    AnimateRuntimeInput, AudioMap, Color, Duration, FFramesContext, Frame, Overlap, Scene, Scenes,
    Svgr, Transform, Video,
    animation::{AnimationRuntime, Easing},
    include_media_dir,
};
use std::sync::LazyLock;

include_media_dir!(pub struct AxionomyVideoMedia, "media");

pub const WIDTH: usize = 1920;
pub const HEIGHT: usize = 1080;

// Studio's palette, with the colors it gives consumed, preserved and
// produced amounts, and the logo's cyan and gold.
const BG: &str = "#101012";
const SURFACE: &str = "#161618";
const LINE: &str = "#2b2a2d";
const MUTED: &str = "#918c82";
const TEXT: &str = "#eae6dc";
const CYAN: &str = "#2db7c8";
const GOLD: &str = "#f2c14e";
const CONSUME: &str = "#c4625a";
const PRESERVE: &str = "#bd8443";
const PRODUCE: &str = "#74a184";

const SANS: &str = "DM Sans";
const MONO: &str = "JetBrains Mono";
const WEIGHT: u16 = 500;
/// A monospaced character's advance, as a share of the font size.
const ADVANCE: f32 = 0.6;

const LEFT: f32 = 160.0;
const RIGHT: f32 = 1760.0;
/// How long one scene fades into the next.
const CROSS: f32 = 0.4;

static ENTER: LazyLock<AnimationRuntime> =
    LazyLock::new(|| AnimationRuntime::new(0.5, &Easing::EaseOut));
static EXIT: LazyLock<AnimationRuntime> =
    LazyLock::new(|| AnimationRuntime::new(0.25, &Easing::EaseIn));
static OUTRO: LazyLock<AnimationRuntime> =
    LazyLock::new(|| AnimationRuntime::new(CROSS, &Easing::EaseIn));
static DRAW: LazyLock<AnimationRuntime> =
    LazyLock::new(|| AnimationRuntime::new(1.1, &Easing::EaseInOut));
static COUNT: LazyLock<AnimationRuntime> =
    LazyLock::new(|| AnimationRuntime::new(0.9, &Easing::EaseOut));
static SPRING: LazyLock<AnimationRuntime> = LazyLock::new(|| {
    AnimationRuntime::new(
        2.0,
        &Easing::Spring {
            mass: 1.0,
            stiffness: 170.0,
            damping: 19.0,
        },
    )
});

fn run(frame: &Frame, runtime: &'static AnimationRuntime, at: f32, from: f32, to: f32) -> f32 {
    frame.animate_runtime(AnimateRuntimeInput {
        on_second: at,
        from,
        to,
        animation_runtime: runtime,
    })
}

/// 0 to 1, entering at `at`.
fn enter(frame: &Frame, at: f32) -> f32 {
    run(frame, &ENTER, at, 0.0, 1.0)
}

/// Entering at `from`, leaving at `to`.
fn shown(frame: &Frame, from: f32, to: f32) -> f32 {
    enter(frame, from) * run(frame, &EXIT, to, 1.0, 0.0)
}

/// A scene `length` seconds long fading as the next one comes in.
fn outro(frame: &Frame, length: f32) -> f32 {
    run(frame, &OUTRO, length - CROSS, 1.0, 0.0)
}

/// 0 to 1 over a slow ease, for strokes drawing on and values moving.
fn draw(frame: &Frame, at: f32) -> f32 {
    run(frame, &DRAW, at, 0.0, 1.0)
}

/// 0 to `to`, counted up from `at`.
fn counted(frame: &Frame, at: f32, to: f32) -> f32 {
    run(frame, &COUNT, at, 0.0, to)
}

/// A distance that springs to zero at `at`.
fn rise(frame: &Frame, at: f32, from: f32) -> f32 {
    run(frame, &SPRING, at, from, 0.0)
}

fn lerp(from: f32, to: f32, t: f32) -> f32 {
    from + (to - from) * t
}

fn mono_width(text: &str, size: f32) -> f32 {
    text.chars().count() as f32 * size * ADVANCE
}

/// The caption: one or two lines at the foot of the frame, rising in.
fn caption<'a>(frame: &Frame, lines: &[&'a str], from: f32, to: f32) -> Svgr<'a> {
    let opacity = shown(frame, from, to);
    let lift = rise(frame, from, 24.0);
    let texts: Vec<Svgr> = lines
        .iter()
        .enumerate()
        .map(|(index, line)| {
            let y = 950.0 + index as f32 * 62.0;
            fframes::svgr!(<text x={LEFT} y={y} font-family={SANS} font-weight={WEIGHT} font-size="48" fill={TEXT}>{*line}</text>)
        })
        .collect();
    fframes::svgr!(<g opacity={opacity} transform={Transform::translate(0.0, lift)}>{texts}</g>)
}

/// Where the numbers come from: a path at the top of the frame.
fn source<'a>(frame: &Frame, text: String, from: f32) -> Svgr<'a> {
    fframes::svgr!(
        <g opacity={enter(frame, from)}>
            <circle cx={LEFT + 8.0} cy="112" r="8" fill={CYAN} />
            <text x={LEFT + 32.0} y="122" font-family={MONO} font-weight={WEIGHT} font-size="28" fill={MUTED}>{text}</text>
        </g>
    )
}

/// `text` in the mono face over a patch of the background, so it reads
/// over the lines it sits on.
fn tag<'a>(
    x: f32,
    y: f32,
    text: String,
    size: f32,
    fill: &'static str,
    anchor: &'static str,
) -> Svgr<'a> {
    let width = mono_width(&text, size) + 16.0;
    let left = match anchor {
        "end" => x - width + 8.0,
        "middle" => x - width / 2.0,
        _ => x - 8.0,
    };
    fframes::svgr!(
        <g>
            <rect x={left} y={y - size} width={width} height={size * 1.4} rx="4" fill={BG} fill-opacity="0.9" />
            <text x={x} y={y} text-anchor={anchor} font-family={MONO} font-weight={WEIGHT} font-size={size} fill={fill}>{text}</text>
        </g>
    )
}

// ---------------------------------------------------------------------------
// The model: accounts, a rate, an exchange, a refusal.

/// The times the model scene's phases start at.
const RATE_AT: f32 = 5.5;
const APPLY_AT: f32 = 12.0;
const REFUSE_AT: f32 = 18.5;
const MODEL_SECONDS: f32 = 25.5;

/// When the exchange's amounts land, and when the refusal's row comes in.
fn applied(frame: &Frame) -> f32 {
    draw(frame, APPLY_AT + 0.6)
}

fn refused(frame: &Frame) -> f32 {
    draw(frame, REFUSE_AT + 0.8)
}

/// A balance row of an account panel through the scene: before the
/// exchange, after it, what the refused exchange lacked of it, and its
/// place among the rows shown before, after, and at the refusal.
struct Row {
    asset: String,
    before: u64,
    after: u64,
    short: u64,
    slots: [Option<usize>; 3],
}

fn rows(before: &Holding, after: &Holding, shortfalls: &[facts::Short]) -> Vec<Row> {
    let mut assets: Vec<&str> = before
        .balances
        .iter()
        .chain(&after.balances)
        .map(|balance| balance.asset.as_str())
        .collect();
    for short in shortfalls
        .iter()
        .filter(|short| short.account == before.account)
    {
        assets.push(&short.asset);
    }
    assets.sort_unstable();
    assets.dedup();
    let held = |holding: &Holding, asset: &str| {
        holding
            .balances
            .iter()
            .find(|balance| balance.asset == asset)
            .map_or(0, |balance| balance.quantity)
    };
    let mut rows: Vec<Row> = assets
        .into_iter()
        .map(|asset| Row {
            asset: asset.into(),
            before: held(before, asset),
            after: held(after, asset),
            short: shortfalls
                .iter()
                .find(|short| short.account == before.account && short.asset == asset)
                .map_or(0, |short| short.missing),
            slots: [None; 3],
        })
        .collect();
    let shows: [fn(&Row) -> bool; 3] = [
        |row| row.before > 0,
        |row| row.after > 0,
        |row| row.after > 0 || row.short > 0,
    ];
    for (phase, shows) in shows.iter().enumerate() {
        let mut slot = 0;
        for row in &mut rows {
            if shows(row) {
                row.slots[phase] = Some(slot);
                slot += 1;
            }
        }
    }
    rows
}

const ROW: f32 = 40.0;
const PANEL_TOP: f32 = 190.0;
const PANEL_WIDTH: f32 = 420.0;

/// A row's y and how much of it shows, moving between its places as the
/// scene goes from one phase to the next.
fn place(frame: &Frame, row: &Row) -> (f32, f32) {
    let [first, second, third] = row.slots;
    let slot = |slot: Option<usize>, fallback: [Option<usize>; 2]| {
        slot.or(fallback[0]).or(fallback[1]).unwrap_or(0) as f32
    };
    let (a, b, c) = (
        slot(first, [second, third]),
        slot(second, [first, third]),
        slot(third, [second, first]),
    );
    let (t1, t2) = (applied(frame), refused(frame));
    let index = lerp(lerp(a, b, t1), c, t2);
    let seen = |slot: Option<usize>| if slot.is_some() { 1.0 } else { 0.0 };
    let shows = lerp(lerp(seen(first), seen(second), t1), seen(third), t2);
    (PANEL_TOP + 92.0 + index * ROW, shows)
}

fn panel_rows(frame: &Frame, rows: &[Row]) -> f32 {
    let count = |phase: usize| rows.iter().filter(|row| row.slots[phase].is_some()).count() as f32;
    lerp(
        lerp(count(0), count(1), applied(frame)),
        count(2),
        refused(frame),
    )
}

fn panel<'a>(frame: &Frame, x: f32, account: &str, rows: &[Row], at: f32) -> Svgr<'a> {
    let height = 92.0 + panel_rows(frame, rows) * ROW + 16.0;
    let landed = applied(frame);
    let lines: Vec<Svgr> = rows
        .iter()
        .enumerate()
        .map(|(index, row)| {
            let (y, shows) = place(frame, row);
            let appears = enter(frame, at + 0.25 + index as f32 * 0.04) * shows;
            let value = lerp(row.before as f32, row.after as f32, landed);
            let delta = row.after as i64 - row.before as i64;
            let badge = if delta == 0 {
                Svgr::empty()
            } else {
                let (text, tint) = if delta < 0 {
                    (format!("{delta}"), CONSUME)
                } else {
                    (format!("+{delta}"), PRODUCE)
                };
                fframes::svgr!(
                    <text x={x + PANEL_WIDTH - 84.0} y={y} text-anchor="end" font-family={MONO} font-weight={WEIGHT} font-size="26" fill={tint}
                        opacity={shown(frame, APPLY_AT + 0.5, REFUSE_AT)}>{text}</text>
                )
            };
            let short = if row.short > 0 {
                fframes::svgr!(
                    <g opacity={enter(frame, REFUSE_AT + 1.4)}>
                        <rect x={x + 10.0} y={y - 29.0} width={PANEL_WIDTH - 20.0} height="38" rx="6" fill="none" stroke={CONSUME} stroke-width="2" />
                        <text x={x + PANEL_WIDTH - 84.0} y={y} text-anchor="end" font-family={MONO} font-weight={WEIGHT} font-size="26" fill={CONSUME}>{format!("short {}", row.short)}</text>
                    </g>
                )
            } else {
                Svgr::empty()
            };
            fframes::svgr!(
                <g opacity={appears}>
                    <text x={x + 24.0} y={y} font-family={MONO} font-weight={WEIGHT} font-size="28" fill={TEXT}>{row.asset.clone()}</text>
                    <text x={x + PANEL_WIDTH - 24.0} y={y} text-anchor="end" font-family={MONO} font-weight={WEIGHT} font-size="28" fill={TEXT}>{format!("{value:.0}")}</text>
                    {badge}
                    {short}
                </g>
            )
        })
        .collect();
    fframes::svgr!(
        <g opacity={enter(frame, at)} transform={Transform::translate(0.0, rise(frame, at, 40.0))}>
            <rect x={x} y={PANEL_TOP} width={PANEL_WIDTH} height={height} rx="14" fill={SURFACE} stroke={LINE} stroke-width="2" />
            <text x={x + 24.0} y={PANEL_TOP + 52.0} font-family={MONO} font-weight={WEIGHT} font-size="34" fill={CYAN}>{account.to_owned()}</text>
            <line x1={x + 24.0} y1={PANEL_TOP + 70.0} x2={x + PANEL_WIDTH - 24.0} y2={PANEL_TOP + 70.0} stroke={LINE} stroke-width="2" />
            {lines}
        </g>
    )
}

/// A line of a rate card: what one role does with one asset.
struct Entry {
    kind: &'static str,
    color: &'static str,
    account: String,
    asset: String,
    quantity: u64,
}

fn entries(law: &Law) -> Vec<(String, Vec<Entry>)> {
    law.roles
        .iter()
        .map(|role| {
            let mut entries = Vec::new();
            for (kind, color, balances) in [
                ("consumes", CONSUME, &role.consumes),
                ("preserves", PRESERVE, &role.preserves),
                ("produces", PRODUCE, &role.produces),
            ] {
                for balance in balances {
                    entries.push(Entry {
                        kind,
                        color,
                        account: role.account.clone(),
                        asset: balance.asset.clone(),
                        quantity: balance.quantity,
                    });
                }
            }
            (format!("{} \u{2192} {}", role.role, role.account), entries)
        })
        .collect()
}

const CARD_X: f32 = 670.0;
const CARD_WIDTH: f32 = 600.0;

/// A rate card, its entries linked to the panel rows they act on; while
/// `flowing`, consumed amounts run from the accounts into the rate and
/// produced ones back out.
fn card<'a>(
    frame: &Frame,
    law: &Law,
    from: f32,
    to: f32,
    panels: &[(f32, &str, &[Row])],
    flowing: Option<(f32, f32)>,
) -> Svgr<'a> {
    let roles = entries(law);
    let mut y = PANEL_TOP + 120.0;
    let mut lines = Vec::new();
    let mut links = Vec::new();
    for (index, (heading, entries)) in roles.iter().enumerate() {
        let at = from + 0.3 + index as f32 * 0.25;
        lines.push(fframes::svgr!(
            <text x={CARD_X + 28.0} y={y} font-family={MONO} font-weight={WEIGHT} font-size="26" fill={MUTED} opacity={enter(frame, at)}>{heading.clone()}</text>
        ));
        y += 44.0;
        let mut last_kind = "";
        for (offset, entry) in entries.iter().enumerate() {
            let at = at + 0.1 + offset as f32 * 0.07;
            let label = if entry.kind == last_kind {
                ""
            } else {
                entry.kind
            };
            last_kind = entry.kind;
            lines.push(fframes::svgr!(
                <g opacity={enter(frame, at)}>
                    <text x={CARD_X + 28.0} y={y} font-family={MONO} font-weight={WEIGHT} font-size="26" fill={entry.color}>{label}</text>
                    <text x={CARD_X + 186.0} y={y} font-family={MONO} font-weight={WEIGHT} font-size="26" fill={TEXT}>{entry.asset.clone()}</text>
                    <text x={CARD_X + CARD_WIDTH - 28.0} y={y} text-anchor="end" font-family={MONO} font-weight={WEIGHT} font-size="26" fill={TEXT}>{entry.quantity.to_string()}</text>
                </g>
            ));
            if let Some((panel_x, _, rows)) =
                panels.iter().find(|(_, name, _)| *name == entry.account)
                && let Some(row) = rows.iter().find(|row| row.asset == entry.asset)
            {
                let (row_y, row_shows) = place(frame, row);
                let left = *panel_x < CARD_X;
                let (x1, y1, x2, y2) = if left {
                    (panel_x + PANEL_WIDTH, row_y - 9.0, CARD_X, y - 9.0)
                } else {
                    (CARD_X + CARD_WIDTH, y - 9.0, *panel_x, row_y - 9.0)
                };
                let middle = (x1 + x2) / 2.0;
                let path = format!("M{x1} {y1} C{middle} {y1} {middle} {y2} {x2} {y2}");
                let length = (x2 - x1).abs() + (y2 - y1).abs();
                let drawn = draw(frame, at + 0.3);
                let flow = flowing.filter(|(start, end)| {
                    entry.kind != "preserves" && frame.seconds() >= *start && frame.seconds() < *end
                });
                let (dash, offset) = match flow {
                    Some((start, _)) => {
                        let going = frame.seconds() - start;
                        let toward_card = (entry.kind == "consumes") == left;
                        let shift = if toward_card {
                            -going * 90.0
                        } else {
                            going * 90.0
                        };
                        ("14 10".to_owned(), shift)
                    }
                    None => (format!("{length} {length}"), length * (1.0 - drawn)),
                };
                links.push(fframes::svgr!(
                    <path d={path} fill="none" stroke={entry.color} stroke-width="2.5" stroke-opacity={0.75 * row_shows.max(0.3)}
                        stroke-dasharray={dash} stroke-dashoffset={offset} />
                ));
            }
            y += 38.0;
        }
        y += 18.0;
    }
    let height = y - PANEL_TOP - 40.0;
    fframes::svgr!(
        <g opacity={shown(frame, from, to)}>
            <g>{links}</g>
            <g transform={Transform::translate(0.0, rise(frame, from, 30.0))}>
                <rect x={CARD_X} y={PANEL_TOP} width={CARD_WIDTH} height={height} rx="14" fill={SURFACE} stroke={GOLD} stroke-width="2" />
                <text x={CARD_X + 28.0} y={PANEL_TOP + 52.0} font-family={MONO} font-weight={WEIGHT} font-size="28" fill={GOLD}>{law.rate.clone()}</text>
                {lines}
            </g>
        </g>
    )
}

#[derive(Debug)]
struct ModelScene<'a> {
    facts: &'a Facts,
}

impl Scene for ModelScene<'_> {
    fn duration(&self) -> Duration<'_> {
        Duration::Seconds(MODEL_SECONDS)
    }

    fn render_frame<'a>(&'a self, frame: Frame, _ctx: &FFramesContext<'a, '_>) -> Svgr<'a> {
        let model = &self.facts.model;
        let panels: Vec<(f32, &str, Vec<Row>)> = model
            .before
            .iter()
            .zip(&model.after)
            .enumerate()
            .map(|(index, (before, after))| {
                let x = if index == 0 {
                    LEFT
                } else {
                    RIGHT - PANEL_WIDTH
                };
                (
                    x,
                    before.account.as_str(),
                    rows(before, after, &model.shortfalls),
                )
            })
            .collect();
        let linked: Vec<(f32, &str, &[Row])> = panels
            .iter()
            .map(|(x, name, rows)| (*x, *name, rows.as_slice()))
            .collect();
        // The first panel is there from the first frame.
        let drawn: Vec<Svgr> = panels
            .iter()
            .enumerate()
            .map(|(index, (x, name, rows))| {
                panel(&frame, *x, name, rows, -0.3 + index as f32 * 0.5)
            })
            .collect();
        let refusal = model
            .shortfalls
            .iter()
            .map(|short| {
                format!(
                    "refused: {} is short {} {}",
                    short.account, short.asset, short.missing
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        let point = if frame.seconds() < APPLY_AT + 0.6 {
            format!("{} \u{b7} its start", model.point)
        } else {
            format!("{} \u{b7} after {}", model.point, model.rate.rate)
        };
        fframes::svgr!(
            <g opacity={outro(&frame, MODEL_SECONDS)}>
                {source(&frame, point, -0.3)}
                {drawn}
                {card(&frame, &model.rate, RATE_AT, REFUSE_AT, &linked, Some((APPLY_AT, APPLY_AT + 2.4)))}
                {card(&frame, &model.refused, REFUSE_AT + 0.3, MODEL_SECONDS + 1.0, &linked, None)}
                <text x={CARD_X} y="860" font-family={MONO} font-weight={WEIGHT} font-size="30" fill={CONSUME}
                    opacity={enter(&frame, REFUSE_AT + 1.6)}>{refusal}</text>
                {caption(&frame, &["An Axionomy economy is accounts", "and the assets they hold."], 0.3, RATE_AT)}
                {caption(&frame, &["A rate says what each role consumes,", "preserves and produces."], RATE_AT + 0.3, APPLY_AT)}
                {caption(&frame, &["An exchange applies the rate to accounts:", "all of it commits, or none."], APPLY_AT + 0.3, REFUSE_AT)}
                {caption(&frame, &["When an exchange can't apply,", "the kernel says what is missing."], REFUSE_AT + 0.3, MODEL_SECONDS + 1.0)}
            </g>
        )
    }
}

// ---------------------------------------------------------------------------
// Value: a pool's reserve ratio, moved by each swap.

const VALUE_SECONDS: f32 = 9.0;

#[derive(Debug)]
struct ValueScene<'a> {
    facts: &'a Facts,
}

impl Scene for ValueScene<'_> {
    fn duration(&self) -> Duration<'_> {
        Duration::Seconds(VALUE_SECONDS)
    }

    fn overlap(&self) -> Overlap {
        Overlap::Previous(CROSS)
    }

    fn render_frame<'a>(&'a self, frame: Frame, _ctx: &FFramesContext<'a, '_>) -> Svgr<'a> {
        let reserves = &self.facts.reserves;
        let ratios: Vec<(usize, f64)> = reserves
            .steps
            .iter()
            .map(|(step, energy, credit)| (*step, credit / energy))
            .collect();
        let (low, high) = ratios
            .iter()
            .fold((f64::MAX, f64::MIN), |(low, high), (_, ratio)| {
                (low.min(*ratio), high.max(*ratio))
            });
        let span = (high - low).max(high * 0.02);
        let (chart_x, chart_y, chart_w, chart_h) = (760.0_f32, 240.0_f32, 1000.0_f32, 500.0_f32);
        let at = |step: f32, ratio: f64| {
            let x = chart_x + step / reserves.exchanges.max(1) as f32 * chart_w;
            let y = chart_y + chart_h - ((ratio - low) / span) as f32 * chart_h;
            (x, y)
        };
        let progress = ((frame.seconds() - 0.9) / 6.0).clamp(0.0, 1.0);
        let reached = progress * reserves.exchanges as f32;
        let mut path = String::new();
        let mut current = (0, ratios[0].1);
        let mut dots = Vec::new();
        for (index, (step, ratio)) in ratios.iter().enumerate() {
            if *step as f32 > reached && index > 0 {
                break;
            }
            let (x, y) = at(*step as f32, *ratio);
            path.push_str(&format!("{}{x} {y} ", if index == 0 { "M" } else { "L" }));
            dots.push(fframes::svgr!(<circle cx={x} cy={y} r="3.5" fill={GOLD} />));
            current = (index, *ratio);
        }
        let (cx, cy) = at(ratios[current.0].0 as f32, current.1);
        let (_, energy, credit) = reserves.steps[current.0];
        let ticks: Vec<Svgr> = [low, low + span / 2.0, low + span]
            .iter()
            .map(|ratio| {
                let (_, y) = at(0.0, *ratio);
                fframes::svgr!(
                    <g>
                        <line x1={chart_x} y1={y} x2={chart_x + chart_w} y2={y} stroke={LINE} stroke-width="1.5" />
                        <text x={chart_x - 16.0} y={y + 9.0} text-anchor="end" font-family={MONO} font-weight={WEIGHT} font-size="24" fill={MUTED}>{format!("{ratio:.2}")}</text>
                    </g>
                )
            })
            .collect();
        let held = |name: &'static str, value: f64, y: f32, color: &'static str| {
            let first = if name == "energy" {
                reserves.steps[0].1
            } else {
                reserves.steps[0].2
            };
            let width = (value / first * 220.0) as f32;
            fframes::svgr!(
                <g>
                    <text x={LEFT + 24.0} y={y} font-family={MONO} font-weight={WEIGHT} font-size="28" fill={TEXT}>{name}</text>
                    <text x={LEFT + 396.0} y={y} text-anchor="end" font-family={MONO} font-weight={WEIGHT} font-size="28" fill={TEXT}>{format!("{value:.0}")}</text>
                    <rect x={LEFT + 24.0} y={y + 18.0} width={width.max(0.5)} height="18" rx="4" fill={color} />
                </g>
            )
        };
        fframes::svgr!(
            <g opacity={outro(&frame, VALUE_SECONDS + CROSS)}>
                {source(&frame, format!("amm/showcase \u{b7} the season {} played", reserves.run), 0.0)}
                <g opacity={enter(&frame, 0.2)} transform={Transform::translate(0.0, rise(&frame, 0.2, 40.0))}>
                    <rect x={LEFT} y="240" width="420" height="500" rx="14" fill={SURFACE} stroke={LINE} stroke-width="2" />
                    <text x={LEFT + 24.0} y="292" font-family={MONO} font-weight={WEIGHT} font-size="34" fill={CYAN}>{reserves.pool.clone()}</text>
                    <line x1={LEFT + 24.0} y1="310" x2={LEFT + 396.0} y2="310" stroke={LINE} stroke-width="2" />
                    {held("energy", energy, 370.0, CYAN)}
                    {held("credit", credit, 460.0, GOLD)}
                    <text x={LEFT + 24.0} y="600" font-family={MONO} font-weight={WEIGHT} font-size="26" fill={MUTED}>"credit per energy"</text>
                    <text x={LEFT + 24.0} y="676" font-family={MONO} font-weight={WEIGHT} font-size="60" fill={GOLD}>{format!("{:.3}", current.1)}</text>
                </g>
                <g opacity={enter(&frame, 0.5)}>
                    {ticks}
                    <text x={chart_x + chart_w} y={chart_y + chart_h + 52.0} text-anchor="end" font-family={MONO} font-weight={WEIGHT} font-size="24" fill={MUTED}>{format!("exchange {} of {}", reached as usize, reserves.exchanges)}</text>
                    <path d={path} fill="none" stroke={GOLD} stroke-width="3" stroke-linejoin="round" />
                    {dots}
                    <circle cx={cx} cy={cy} r="10" fill={GOLD} />
                </g>
                {caption(&frame, &["The pool's reserve ratio is the only price,", "and each swap moves it."], 0.6, VALUE_SECONDS + 1.0)}
            </g>
        )
    }
}

// ---------------------------------------------------------------------------
// Utility: objectives are balances; routes compared on both.

const SCATTER_X: f32 = 1040.0;
const SCATTER_Y: f32 = 220.0;
const SCATTER_W: f32 = 560.0;
const SCATTER_H: f32 = 560.0;

/// Where `(energy, time)` sits on the scatter.
fn on_scatter(facts: &Facts, energy: u64, time: u64) -> (f32, f32) {
    let max_energy = facts.searched.iter().map(|o| o.energy).max().unwrap_or(1) + 2;
    let max_time = facts.searched.iter().map(|o| o.time).max().unwrap_or(1) + 2;
    (
        SCATTER_X + energy as f32 / max_energy as f32 * SCATTER_W,
        SCATTER_Y + SCATTER_H - time as f32 / max_time as f32 * SCATTER_H,
    )
}

/// The searches' outcomes grouped by point, in order.
fn grouped(outcomes: &[Outcome]) -> Vec<((u64, u64), Vec<String>)> {
    let mut points: Vec<((u64, u64), Vec<String>)> = Vec::new();
    for outcome in outcomes {
        let key = (outcome.energy, outcome.time);
        match points.iter_mut().find(|(at, _)| *at == key) {
            Some((_, names)) => names.push(outcome.by.clone()),
            None => points.push((key, vec![outcome.by.clone()])),
        }
    }
    points
}

fn dominated(point: (u64, u64), others: &[(u64, u64)]) -> bool {
    others
        .iter()
        .any(|other| other.0 <= point.0 && other.1 <= point.1 && *other != point)
}

/// The maze Showcase's routes on its two objectives: the searches' points,
/// gold where no other beats them on both, and the learned routes landing
/// from `learned_at`.
fn scatter<'a>(frame: &Frame, facts: &Facts, from: f32, learned_at: Option<f32>) -> Svgr<'a> {
    let points = grouped(&facts.searched);
    let keys: Vec<(u64, u64)> = points.iter().map(|(key, _)| *key).collect();
    // Labels go right of their points, pushed apart where points are close.
    let mut order: Vec<usize> = (0..points.len()).collect();
    order.sort_by(|a, b| {
        on_scatter(facts, keys[*a].0, keys[*a].1)
            .1
            .total_cmp(&on_scatter(facts, keys[*b].0, keys[*b].1).1)
    });
    let mut label_y = vec![0.0_f32; points.len()];
    let mut bottom = f32::MIN;
    for index in order {
        let (_, y) = on_scatter(facts, keys[index].0, keys[index].1);
        let top = (y - 10.0).max(bottom + 14.0);
        label_y[index] = top;
        bottom = top + 30.0 * points[index].1.len() as f32;
    }
    let dots: Vec<Svgr> = points
        .iter()
        .enumerate()
        .map(|(index, (key, names))| {
            let (x, y) = on_scatter(facts, key.0, key.1);
            let at = from + 0.6 + index as f32 * 0.3;
            let front = !dominated(*key, &keys);
            let (fill, ring) = if front { (GOLD, GOLD) } else { (MUTED, LINE) };
            let names: Vec<Svgr> = names
                .iter()
                .enumerate()
                .map(|(line, name)| {
                    fframes::svgr!(<text x={x + 46.0} y={label_y[index] + 22.0 + line as f32 * 30.0} font-family={MONO} font-weight={WEIGHT} font-size="24" fill={MUTED}>{name.clone()}</text>)
                })
                .collect();
            let value = format!("({}, {})", key.0, key.1);
            fframes::svgr!(
                <g opacity={enter(frame, at)}>
                    <circle cx={x} cy={y} r={10.0 + 6.0 * (1.0 - enter(frame, at))} fill={fill} />
                    <circle cx={x} cy={y} r="18" fill="none" stroke={ring} stroke-width="2" />
                    <text x={x - 46.0} y={y + 8.0} text-anchor="end" font-family={MONO} font-weight={WEIGHT} font-size="26" fill={TEXT}>{value}</text>
                    {names}
                </g>
            )
        })
        .collect();
    let learned: Vec<Svgr> = learned_at
        .map(|at| {
            facts
                .learned
                .iter()
                .enumerate()
                .map(|(index, learned)| {
                    let at = at + index as f32 * 0.9;
                    let (x, y) = on_scatter(facts, learned.outcome.energy, learned.outcome.time);
                    let color = if index == 0 { CYAN } else { GOLD };
                    fframes::svgr!(
                        <g opacity={enter(frame, at)} transform={Transform::translate(0.0, rise(frame, at, -180.0))}>
                            <circle cx={x} cy={y} r="30" fill="none" stroke={color} stroke-width="4" />
                            {tag(x - 46.0, y + 46.0, learned.outcome.by.clone(), 22.0, color, "end")}
                        </g>
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    fframes::svgr!(
        <g opacity={enter(frame, from)}>
            <line x1={SCATTER_X} y1={SCATTER_Y + SCATTER_H} x2={SCATTER_X + SCATTER_W} y2={SCATTER_Y + SCATTER_H} stroke={MUTED} stroke-width="2" />
            <line x1={SCATTER_X} y1={SCATTER_Y} x2={SCATTER_X} y2={SCATTER_Y + SCATTER_H} stroke={MUTED} stroke-width="2" />
            <text x={SCATTER_X + SCATTER_W} y={SCATTER_Y + SCATTER_H + 50.0} text-anchor="end" font-family={MONO} font-weight={WEIGHT} font-size="26" fill={MUTED}>"maze/energy \u{2192}"</text>
            <text x={SCATTER_X} y={SCATTER_Y - 28.0} font-family={MONO} font-weight={WEIGHT} font-size="26" fill={MUTED}>"\u{2191} maze/time"</text>
            {dots}
            {learned}
        </g>
    )
}

const UTILITY_SECONDS: f32 = 9.0;

#[derive(Debug)]
struct UtilityScene<'a> {
    facts: &'a Facts,
}

impl Scene for UtilityScene<'_> {
    fn duration(&self) -> Duration<'_> {
        Duration::Seconds(UTILITY_SECONDS)
    }

    fn overlap(&self) -> Overlap {
        Overlap::Previous(CROSS)
    }

    fn render_frame<'a>(&'a self, frame: Frame, _ctx: &FFramesContext<'a, '_>) -> Svgr<'a> {
        let points = grouped(&self.facts.searched);
        let keys: Vec<(u64, u64)> = points.iter().map(|(key, _)| *key).collect();
        let ends: Vec<Svgr> = points
            .into_iter()
            .enumerate()
            .map(|(index, ((energy, time), names))| {
                let y = 280.0 + index as f32 * 170.0;
                let at = 0.6 + index as f32 * 0.3;
                let front = !dominated((energy, time), &keys);
                let tint = if front { GOLD } else { MUTED };
                fframes::svgr!(
                    <g opacity={enter(&frame, at)} transform={Transform::translate(rise(&frame, at, -30.0), 0.0)}>
                        <text x={LEFT} y={y} font-family={MONO} font-weight={WEIGHT} font-size="24" fill={MUTED}>{format!("{} \u{b7} at its end", names.join(", "))}</text>
                        <text x={LEFT} y={y + 50.0} font-family={MONO} font-weight={WEIGHT} font-size="30" fill={TEXT}>"agent"</text>
                        <text x={LEFT + 140.0} y={y + 50.0} font-family={MONO} font-weight={WEIGHT} font-size="30" fill={TEXT}>"spent-energy"</text>
                        <text x={LEFT + 400.0} y={y + 50.0} text-anchor="end" font-family={MONO} font-weight={WEIGHT} font-size="30" fill={tint}>{counted(&frame, at + 0.2, energy as f32).round().to_string()}</text>
                        <text x={LEFT + 450.0} y={y + 50.0} font-family={MONO} font-weight={WEIGHT} font-size="30" fill={TEXT}>"spent-time"</text>
                        <text x={LEFT + 680.0} y={y + 50.0} text-anchor="end" font-family={MONO} font-weight={WEIGHT} font-size="30" fill={tint}>{counted(&frame, at + 0.2, time as f32).round().to_string()}</text>
                    </g>
                )
            })
            .collect();
        fframes::svgr!(
            <g opacity={outro(&frame, UTILITY_SECONDS + CROSS)}>
                {source(&frame, "maze/showcase \u{b7} where its experiment's searches end".to_owned(), 0.0)}
                {ends}
                {scatter(&frame, self.facts, 0.3, None)}
                {caption(&frame, &["Objectives are balances too: each route", "is compared on energy and time at once."], 0.6, UTILITY_SECONDS + 1.0)}
            </g>
        )
    }
}

// ---------------------------------------------------------------------------
// Simulation: forks from a point, and a plan committed.

const FORK_SECONDS: f32 = 9.0;

#[derive(Debug)]
struct ForkScene<'a> {
    facts: &'a Facts,
}

impl Scene for ForkScene<'_> {
    fn duration(&self) -> Duration<'_> {
        Duration::Seconds(FORK_SECONDS)
    }

    fn overlap(&self) -> Overlap {
        Overlap::Previous(CROSS)
    }

    fn render_frame<'a>(&'a self, frame: Frame, _ctx: &FFramesContext<'a, '_>) -> Svgr<'a> {
        let forks = &self.facts.forks;
        let top = 230.0_f32;
        let height = 600.0_f32;
        let spacing = 360.0_f32;
        let place = |level: usize, index: usize| {
            let count = forks.levels[level].len().max(1);
            let x = LEFT + 40.0 + level as f32 * spacing;
            let y = if count == 1 {
                top + height / 2.0
            } else {
                top + (index as f32 + 0.5) / count as f32 * height
            };
            (x, y)
        };
        let lit_at = |level: usize| 4.2 + level as f32 * 0.35;
        let mut edges = Vec::new();
        let mut nodes = Vec::new();
        for (level, parents) in forks.levels.iter().enumerate() {
            let at = 0.5 + level as f32 * 0.9;
            for (index, parent) in parents.iter().enumerate() {
                let (x, y) = place(level, index);
                let on_plan = forks.plan.get(level) == Some(&index);
                let lit = on_plan && frame.seconds() > lit_at(level);
                if level > 0 {
                    let (px, py) = place(level - 1, *parent);
                    let grow = draw(&frame, at);
                    let (stroke, width) = if lit { (GOLD, 4.0) } else { (LINE, 1.5) };
                    edges.push(fframes::svgr!(
                        <line x1={px} y1={py} x2={lerp(px, x, grow)} y2={lerp(py, y, grow)} stroke={stroke} stroke-width={width} />
                    ));
                }
                let r = if level == 0 {
                    12.0
                } else if lit {
                    9.0
                } else {
                    4.5
                };
                let fill = if lit || level == 0 { GOLD } else { MUTED };
                nodes.push(fframes::svgr!(<circle cx={x} cy={y} r={r} fill={fill} opacity={enter(&frame, at + 0.6)} />));
            }
        }
        // The plan's exchanges, each under its edge, and the rest of it on
        // to its goal.
        let last = forks.levels.len() - 1;
        let labels: Vec<Svgr> = forks
            .rates
            .iter()
            .take(last)
            .enumerate()
            .map(|(level, rate)| {
                let (x1, y1) = place(level, forks.plan[level]);
                let (x2, y2) = place(level + 1, forks.plan[level + 1]);
                let y = (y1 + y2) / 2.0 + if level % 2 == 0 { 44.0 } else { -24.0 };
                fframes::svgr!(
                    <g opacity={enter(&frame, lit_at(level + 1))}>
                        {tag((x1 + x2) / 2.0, y, rate.clone(), 22.0, GOLD, "middle")}
                    </g>
                )
            })
            .collect();
        let (lx, ly) = place(last, forks.plan[last]);
        let goal_x = RIGHT - 30.0;
        let tail = draw(&frame, lit_at(last) + 0.4);
        let rest = forks.rates.len().saturating_sub(last);
        let counts: Vec<String> = forks
            .levels
            .iter()
            .map(|level| level.len().to_string())
            .collect();
        fframes::svgr!(
            <g opacity={outro(&frame, FORK_SECONDS + CROSS)}>
                {source(&frame, format!("maze/showcase \u{b7} points forked from its start: {}", counts.join(", ")), 0.0)}
                {edges}
                {nodes}
                <line x1={lx} y1={ly} x2={lerp(lx, goal_x, tail)} y2={ly} stroke={GOLD} stroke-width="4" stroke-dasharray="12 10" />
                {labels}
                <g opacity={enter(&frame, lit_at(last) + 1.4)}>
                    <circle cx={goal_x} cy={ly} r="16" fill="none" stroke={GOLD} stroke-width="4" />
                    {tag(goal_x, ly - 36.0, format!("{rest} more exchanges to maze/solved"), 24.0, GOLD, "end")}
                </g>
                {caption(&frame, &["A planner tries exchanges on forks under the", "same rates, then commits its plan: here, A*'s."], 0.6, FORK_SECONDS + 1.0)}
            </g>
        )
    }
}

// ---------------------------------------------------------------------------
// Training: a DQN in Burn, rewarded with the objectives.

const TRAINING_SECONDS: f32 = 10.0;

#[derive(Debug)]
struct TrainingScene<'a> {
    facts: &'a Facts,
}

impl Scene for TrainingScene<'_> {
    fn duration(&self) -> Duration<'_> {
        Duration::Seconds(TRAINING_SECONDS)
    }

    fn overlap(&self) -> Overlap {
        Overlap::Previous(CROSS)
    }

    fn render_frame<'a>(&'a self, frame: Frame, _ctx: &FFramesContext<'a, '_>) -> Svgr<'a> {
        let learned = &self.facts.learned;
        let (low, high) = learned
            .iter()
            .flat_map(|learned| learned.curve.iter())
            .fold((f64::MAX, f64::MIN), |(low, high), value| {
                (low.min(*value), high.max(*value))
            });
        // The caption says the learned routes are the searches' only where
        // every one of them is.
        let found = if learned.iter().all(|learned| {
            learned.reached
                && self.facts.searched.iter().any(|searched| {
                    (searched.energy, searched.time)
                        == (learned.outcome.energy, learned.outcome.time)
                })
        }) {
            "learns the routes the searches find."
        } else {
            "learns its own routes to the goal."
        };
        let (cx, cy, cw, ch) = (LEFT + 70.0, 240.0_f32, 640.0_f32, 470.0_f32);
        let progress = ((frame.seconds() - 0.6) / 3.8).clamp(0.0, 1.0);
        let curves: Vec<Svgr> = learned
            .iter()
            .enumerate()
            .map(|(index, learned)| {
                let color = if index == 0 { CYAN } else { GOLD };
                let count = learned.curve.len().max(2);
                let upto = (count as f32 * progress) as usize;
                let path: String = learned
                    .curve
                    .iter()
                    .take(upto.max(1))
                    .enumerate()
                    .map(|(step, value)| {
                        let x = cx + step as f32 / (count - 1) as f32 * cw;
                        let y = cy + ch - ((value - low) / (high - low).max(1e-9)) as f32 * ch;
                        format!("{}{x} {y} ", if step == 0 { "M" } else { "L" })
                    })
                    .collect();
                let label_y = cy + ch + 60.0 + index as f32 * 40.0;
                fframes::svgr!(
                    <g>
                        <path d={path} fill="none" stroke={color} stroke-width="3" stroke-linejoin="round" />
                        <text x={cx} y={label_y} font-family={MONO} font-weight={WEIGHT} font-size="24" fill={color} opacity={enter(&frame, 0.8)}>
                            {format!("weighing {}: {} episodes", learned.weighing, learned.episodes)}
                        </text>
                    </g>
                )
            })
            .collect();
        let ticks: Vec<Svgr> = [low, high]
            .iter()
            .map(|value| {
                let y = cy + ch - ((value - low) / (high - low).max(1e-9)) as f32 * ch;
                fframes::svgr!(<text x={cx - 14.0} y={y + 8.0} text-anchor="end" font-family={MONO} font-weight={WEIGHT} font-size="22" fill={MUTED}>{format!("{value:.0}")}</text>)
            })
            .collect();
        fframes::svgr!(
            <g opacity={outro(&frame, TRAINING_SECONDS + CROSS)}>
                {source(&frame, "crates/axionomy-burn \u{b7} maze/showcase, reward per episode".to_owned(), 0.0)}
                <g opacity={enter(&frame, 0.3)}>
                    <line x1={cx} y1={cy + ch} x2={cx + cw} y2={cy + ch} stroke={MUTED} stroke-width="2" />
                    <line x1={cx} y1={cy} x2={cx} y2={cy + ch} stroke={MUTED} stroke-width="2" />
                    {ticks}
                    {curves}
                </g>
                {scatter(&frame, self.facts, 0.2, Some(4.8))}
                {caption(&frame, &["A DQN in Burn, rewarded with these objectives,", found], 0.6, TRAINING_SECONDS + 1.0)}
            </g>
        )
    }
}

// ---------------------------------------------------------------------------
// Harnesses: whoever proposes, the kernel decides.

const HARNESS_SECONDS: f32 = 9.0;

#[derive(Debug)]
struct HarnessScene<'a> {
    facts: &'a Facts,
}

impl Scene for HarnessScene<'_> {
    fn duration(&self) -> Duration<'_> {
        Duration::Seconds(HARNESS_SECONDS)
    }

    fn overlap(&self) -> Overlap {
        Overlap::Previous(CROSS)
    }

    fn render_frame<'a>(&'a self, frame: Frame, _ctx: &FFramesContext<'a, '_>) -> Svgr<'a> {
        let kind = |name: &str| match name {
            "connect-four" => "a game, against minimax or MCTS",
            "work-league" => "agents competing for the same work",
            "logistics" => "deliveries under fuel and time windows",
            "perishables" => "a cold chain",
            "amm" => "a market with no price feed",
            "lending" => "collateral, debt and liquidation",
            _ => "",
        };
        let count = |at: f32, value: usize| counted(&frame, at, value as f32).round().to_string();
        let mut rows: Vec<Svgr> = self
            .facts
            .packages
            .iter()
            .enumerate()
            .map(|(index, package)| {
                let y = 240.0 + index as f32 * 88.0;
                let at = 0.4 + index as f32 * 0.2;
                fframes::svgr!(
                    <g opacity={enter(&frame, at)} transform={Transform::translate(rise(&frame, at, -40.0), 0.0)}>
                        <text x={LEFT} y={y} font-family={MONO} font-weight={WEIGHT} font-size="34" fill={CYAN}>{package.name.clone()}</text>
                        <text x={LEFT + 330.0} y={y} font-family={SANS} font-weight={WEIGHT} font-size="32" fill={TEXT}>{kind(&package.name)}</text>
                        <g font-family={MONO} font-weight={WEIGHT} font-size="28" text-anchor="end">
                            <text x={RIGHT - 520.0} y={y} fill={TEXT}>{count(at + 0.2, package.accounts)}</text>
                            <text x={RIGHT - 520.0 + 16.0} y={y} text-anchor="start" fill={MUTED}>"accounts"</text>
                            <text x={RIGHT - 250.0} y={y} fill={TEXT}>{count(at + 0.3, package.instances)}</text>
                            <text x={RIGHT - 250.0 + 16.0} y={y} text-anchor="start" fill={MUTED}>"rate instances"</text>
                        </g>
                    </g>
                )
            })
            .collect();
        let y = 240.0 + self.facts.packages.len() as f32 * 88.0;
        let at = 0.4 + self.facts.packages.len() as f32 * 0.2;
        rows.push(fframes::svgr!(
            <g opacity={enter(&frame, at)} transform={Transform::translate(rise(&frame, at, -40.0), 0.0)}>
                <text x={LEFT} y={y} font-family={MONO} font-weight={WEIGHT} font-size="34" fill={CYAN}>"axionomy mcp"</text>
                <text x={LEFT + 330.0} y={y} font-family={SANS} font-weight={WEIGHT} font-size="32" fill={TEXT}>"LLM agents, through the MCP server"</text>
            </g>
        ));
        fframes::svgr!(
            <g opacity={outro(&frame, HARNESS_SECONDS + CROSS)}>
                {source(&frame, "sample packages \u{b7} at their showcase starts".to_owned(), 0.0)}
                {rows}
                {caption(&frame, &["Game AIs, planners, learners and LLM agents", "propose exchanges; the kernel checks each one."], 0.8, HARNESS_SECONDS + 1.0)}
            </g>
        )
    }
}

// ---------------------------------------------------------------------------
// Trust: replay, and the specification.

const TRUST_SECONDS: f32 = 9.0;

#[derive(Debug)]
struct TrustScene<'a> {
    facts: &'a Facts,
}

impl Scene for TrustScene<'_> {
    fn duration(&self) -> Duration<'_> {
        Duration::Seconds(TRUST_SECONDS)
    }

    fn overlap(&self) -> Overlap {
        Overlap::Previous(CROSS)
    }

    fn render_frame<'a>(&'a self, frame: Frame, _ctx: &FFramesContext<'a, '_>) -> Svgr<'a> {
        let replay = &self.facts.replay;
        let typed = |text: String, at: f32, seconds: f32| {
            let count = text.chars().count();
            let shown =
                (((frame.seconds() - at) / seconds).clamp(0.0, 1.0) * count as f32) as usize;
            text.chars().take(shown).collect::<String>()
        };
        let x = LEFT + 260.0;
        // The replay steps through the route's exchanges, one dot each.
        let steps: Vec<Svgr> = (0..replay.exchanges)
            .map(|step| {
                let at = 1.2 + step as f32 * 0.09;
                let dx = x + step as f32 * 40.0;
                fframes::svgr!(<circle cx={dx} cy="372" r={4.0 + 3.0 * enter(&frame, at)} fill={if frame.seconds() >= at { GOLD } else { LINE }} />)
            })
            .collect();
        let replayed_at = 1.2 + replay.exchanges as f32 * 0.09 + 0.2;
        let same = replay.recorded == replay.replayed;
        let theorems: Vec<Svgr> = self
            .facts
            .theorems
            .iter()
            .enumerate()
            .map(|(index, theorem)| {
                let y = 620.0 + index as f32 * 52.0;
                let at = 3.6 + index as f32 * 0.35;
                fframes::svgr!(
                    <g font-family={MONO} font-weight={WEIGHT} font-size="30">
                        <text x={LEFT} y={y} fill={MUTED} opacity={enter(&frame, at)}>{theorem.file}</text>
                        <text x={x} y={y} fill={TEXT}>{typed(format!("theorem {}", theorem.name), at, 0.5)}</text>
                    </g>
                )
            })
            .collect();
        fframes::svgr!(
            <g opacity={outro(&frame, TRUST_SECONDS + CROSS)}>
                {source(&frame, format!("maze/showcase \u{b7} {}'s route, {} exchanges, recorded and replayed", replay.run, replay.exchanges), 0.0)}
                <g font-family={MONO} font-weight={WEIGHT}>
                    <text x={LEFT} y="300" font-size="30" fill={MUTED} opacity={enter(&frame, 0.3)}>"recorded"</text>
                    <text x={x} y="300" font-size="44" fill={TEXT}>{typed(format!("{:016x}", replay.recorded), 0.4, 0.6)}</text>
                    {steps}
                    <text x={LEFT} y="460" font-size="30" fill={MUTED} opacity={enter(&frame, replayed_at)}>"replayed"</text>
                    <text x={x} y="460" font-size="44" fill={TEXT}>{typed(format!("{:016x}", replay.replayed), replayed_at, 0.6)}</text>
                    <text x={x + mono_width("0000000000000000", 44.0) + 40.0} y="460" font-size="34" fill={if same { PRODUCE } else { CONSUME }}
                        opacity={enter(&frame, replayed_at + 0.8)}>{if same { "the same state" } else { "a different state" }}</text>
                    <text x={LEFT} y="560" font-size="26" fill={MUTED} opacity={enter(&frame, 3.4)}>"verification/lean/Axionomy"</text>
                </g>
                {theorems}
                {caption(&frame, &["Every run replays to the same state.", "The kernel's rules are specified in Lean."], 0.6, TRUST_SECONDS + 1.0)}
            </g>
        )
    }
}

// ---------------------------------------------------------------------------
// The end.

const END_SECONDS: f32 = 5.0;

#[derive(Debug)]
struct EndScene;

impl Scene for EndScene {
    fn duration(&self) -> Duration<'_> {
        Duration::Seconds(END_SECONDS)
    }

    fn overlap(&self) -> Overlap {
        Overlap::Previous(CROSS)
    }

    fn render_frame<'a>(&'a self, frame: Frame, ctx: &FFramesContext<'a, '_>) -> Svgr<'a> {
        let Some(logo) = ctx.get_image("logo.png") else {
            return Svgr::empty();
        };
        // It settles in, then keeps drifting a little larger.
        let scale = 0.94 + 0.06 * enter(&frame, 0.2) + 0.012 * frame.seconds() / END_SECONDS;
        let (w, h) = (1080.0_f32, 192.0_f32);
        let (x, y) = (
            (WIDTH as f32 - w * scale) / 2.0,
            330.0 + h * (1.0 - scale) / 2.0,
        );
        fframes::svgr!(
            <g>
                <image href={logo.href()} x={x} y={y} width={w * scale} height={h * scale} opacity={enter(&frame, 0.2)} />
                <g font-family={MONO} font-weight={WEIGHT} font-size="34" text-anchor="middle">
                    <text x="960" y="700" fill={TEXT} opacity={enter(&frame, 0.9)}>"github.com/BiomaAI/axionomy"</text>
                    <text x="960" y="760" fill={MUTED} opacity={enter(&frame, 1.1)}>"biomaai.github.io/axionomy"</text>
                </g>
            </g>
        )
    }
}

// ---------------------------------------------------------------------------

pub struct AxionomyVideo<'a> {
    model: ModelScene<'a>,
    value: ValueScene<'a>,
    utility: UtilityScene<'a>,
    forks: ForkScene<'a>,
    training: TrainingScene<'a>,
    harness: HarnessScene<'a>,
    trust: TrustScene<'a>,
    end: EndScene,
}

impl<'a> AxionomyVideo<'a> {
    pub fn new(facts: &'a Facts) -> Self {
        Self {
            model: ModelScene { facts },
            value: ValueScene { facts },
            utility: UtilityScene { facts },
            forks: ForkScene { facts },
            training: TrainingScene { facts },
            harness: HarnessScene { facts },
            trust: TrustScene { facts },
            end: EndScene,
        }
    }
}

impl std::fmt::Debug for AxionomyVideo<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AxionomyVideo").finish()
    }
}

impl Video for AxionomyVideo<'_> {
    const FPS: usize = 30;
    const WIDTH: usize = WIDTH;
    const HEIGHT: usize = HEIGHT;
    const BACKGROUND_COLOR: Color = Color::BLACK;

    fn duration(&self) -> Duration<'_> {
        Duration::Auto
    }

    fn audio(&self) -> AudioMap<'_> {
        AudioMap::none()
    }

    fn define_scenes(&self) -> Scenes<'_> {
        Scenes::from(vec![
            &self.model as &dyn Scene,
            &self.value,
            &self.utility,
            &self.forks,
            &self.training,
            &self.harness,
            &self.trust,
            &self.end,
        ])
    }

    fn render_frame<'a>(&'a self, frame: Frame, ctx: &FFramesContext<'a, '_>) -> Svgr<'a> {
        fframes::svgr!(
            <svg xmlns="http://www.w3.org/2000/svg" width={WIDTH} height={HEIGHT} viewBox="0 0 1920 1080">
                <rect width="1920" height="1080" fill={BG} />
                {ctx.render_scenes(&frame)}
            </svg>
        )
    }
}
