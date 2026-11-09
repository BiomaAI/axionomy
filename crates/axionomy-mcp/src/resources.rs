//! Resource addresses. Everything an economy, assessment or run holds is read
//! as an `axionomy://` resource, and every list is read one page at a time with
//! the MCP pagination conventions: an opaque `cursor` chosen by the server,
//! returned as `nextCursor` while more items remain. The named snapshots never
//! change, so a page's content never goes stale.

use axionomy::InvariantPath;

use crate::handles::{ArtifactId, AssessmentId, EconomyId};
use axionomy::{AccountPath, AssetPath, RatePath, path::Path};
use axionomy_view::DocumentPath;
use rmcp::{
    ErrorData as McpError,
    model::{Resource, ResourceTemplate},
};
use serde::Serialize;
use std::{
    fmt::{self, Write as _},
    str::FromStr,
};

pub(crate) const SCHEME: &str = "axionomy://";
/// Items per page of quantities, accounts, rates and other small rows.
pub(crate) const PAGE_ROWS: usize = 500;
/// Frames per page; one frame carries a whole rendered scene.
pub(crate) const PAGE_FRAMES: usize = 10;
/// How long a page stays fresh: its snapshot never changes, so a day is only a
/// bound on how long a client keeps it.
pub(crate) const IMMUTABLE_TTL_MS: u64 = 86_400_000;
/// How long the tool and template lists stay fresh within one server version.
pub(crate) const LIST_TTL_MS: u64 = 3_600_000;
const JSON: &str = "application/json";

/// What a resource URI names, without its cursor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Target {
    Economy {
        economy_id: EconomyId,
    },
    Accounts {
        economy_id: EconomyId,
        prefix: Option<String>,
    },
    Balances {
        economy_id: EconomyId,
        account: AccountPath,
        prefix: Option<String>,
    },
    Rates {
        economy_id: EconomyId,
        asset: Option<AssetPath>,
    },
    Rate {
        economy_id: EconomyId,
        rate: RatePath,
    },
    RateAmounts {
        economy_id: EconomyId,
        rate: RatePath,
    },
    RateConditions {
        economy_id: EconomyId,
        rate: RatePath,
    },
    Invariants {
        economy_id: EconomyId,
    },
    InvariantWeights {
        economy_id: EconomyId,
        invariant: InvariantPath,
    },
    Changes {
        economy_id: EconomyId,
    },
    Exchanges {
        economy_id: EconomyId,
    },
    Diff {
        economy_id: EconomyId,
        other_id: EconomyId,
    },
    Assessment {
        assessment_id: AssessmentId,
    },
    AssessmentLines {
        assessment_id: AssessmentId,
    },
    Artifact {
        artifact_id: ArtifactId,
    },
    Document {
        artifact_id: ArtifactId,
        document: DocumentPath,
    },
    Frames {
        artifact_id: ArtifactId,
        document: DocumentPath,
    },
}

/// A parsed resource URI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Address {
    pub target: Target,
    pub cursor: Option<Cursor>,
}

/// Where a listing resumes: the position of its next item, tied to the
/// listing it came from so a cursor from one listing is refused by another
/// instead of silently reading the wrong rows. Written `position.tag`, with
/// the tag in hexadecimal; clients treat it as opaque.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Cursor {
    position: usize,
    tag: u32,
}

impl Cursor {
    fn new(target: &Target, position: usize) -> Self {
        Self {
            position,
            tag: listing_tag(target),
        }
    }
}

impl fmt::Display for Cursor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}.{:08x}", self.position, self.tag)
    }
}

impl FromStr for Cursor {
    type Err = McpError;

    fn from_str(cursor: &str) -> Result<Self, Self::Err> {
        let invalid = || McpError::invalid_params(format!("invalid cursor `{cursor}`"), None);
        let (position, tag) = cursor.split_once('.').ok_or_else(invalid)?;
        Ok(Self {
            position: position.parse().map_err(|_| invalid())?,
            tag: u32::from_str_radix(tag, 16).map_err(|_| invalid())?,
        })
    }
}

impl Serialize for Cursor {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

/// One URI template per kind of resource, in RFC 6570 syntax.
pub(crate) fn templates() -> Vec<ResourceTemplate> {
    [
        ("axionomy://economy/{economy_id}", "economy", "Economy snapshot", "How the snapshot was made, its state root, and how many accounts, assets, rates and invariants it has."),
        ("axionomy://economy/{economy_id}/accounts{?prefix,cursor}", "accounts", "Accounts", "Accounts in path order, each with how many assets it holds, optionally only paths that start with `prefix`, such as `agents/7/`."),
        ("axionomy://economy/{economy_id}/accounts/{+account}{?prefix,cursor}", "balances", "Account balances", "An account's nonzero balances in asset path order, optionally only assets whose path starts with `prefix`. The account path appears as is, as in `accounts/agents/7/wallet`."),
        ("axionomy://economy/{economy_id}/rates{?asset,cursor}", "rates", "Rates", "Rates with their roles, optionally only rates that move `asset`."),
        ("axionomy://economy/{economy_id}/rates/{+rate}", "rate", "Rate", "A rate's roles, distinct role pairs, parameters and how many amounts and conditions it has."),
        ("axionomy://economy/{economy_id}/rate-amounts/{+rate}{?cursor}", "rate-amounts", "Rate amounts", "What one unit of the rate consumes, produces and preserves, by role: fixed quantities, then computed ones."),
        ("axionomy://economy/{economy_id}/rate-conditions/{+rate}{?cursor}", "rate-conditions", "Rate conditions", "Comparisons every exchange of the rate must satisfy."),
        ("axionomy://economy/{economy_id}/invariants{?cursor}", "invariants", "Invariants", "Linear conservation laws, each with how many assets it weights."),
        ("axionomy://economy/{economy_id}/invariants/{+invariant}{?cursor}", "invariant-weights", "Invariant weights", "The integer weight of each asset in one conservation law."),
        ("axionomy://economy/{economy_id}/changes{?cursor}", "changes", "Changes", "The quantities moved by the exchanges that made this snapshot from its parent."),
        ("axionomy://economy/{economy_id}/exchanges{?cursor}", "exchanges", "Exchanges", "The exchanges that made this snapshot from its parent, in order; replaying them on the parent reproduces it."),
        ("axionomy://economy/{economy_id}/diff/{other_id}{?cursor}", "diff", "Diff", "What differs from this snapshot to another, in path order: accounts opened and closed, balances with both quantities, rates added, removed or redefined, and whether the invariants differ. Equal snapshots have no rows."),
        ("axionomy://assessment/{assessment_id}", "assessment", "Assessment", "An exchange checked against a snapshot: its status, issues and how many lines explain it."),
        ("axionomy://assessment/{assessment_id}/lines{?cursor}", "assessment-lines", "Assessment lines", "What each account lacks for an infeasible exchange, or what an applicable one would move."),
        ("axionomy://artifact/{artifact_id}", "artifact", "Run artifact", "A finished problem run: its request and documents."),
        ("axionomy://artifact/{artifact_id}/documents/{document}", "document", "Run document", "One outcome's document without its frames: initial state, objectives and solver evidence."),
        ("axionomy://artifact/{artifact_id}/documents/{document}/frames{?cursor}", "frames", "Run frames", "A document's replay frames in order."),
    ]
    .into_iter()
    .map(|(template, name, title, description)| {
        ResourceTemplate::new(template, name)
            .with_title(title)
            .with_description(description)
            .with_mime_type(JSON)
    })
    .collect()
}

impl Target {
    /// The canonical URI of this target's first page.
    pub(crate) fn uri(&self) -> String {
        let mut uri = String::from(SCHEME);
        let (segments, query): (Vec<&str>, Vec<(&str, &str)>) = match self {
            Self::Economy { economy_id } => (vec!["economy", economy_id.as_str()], vec![]),
            Self::Accounts { economy_id, prefix } => (
                vec!["economy", economy_id.as_str(), "accounts"],
                prefix
                    .iter()
                    .map(|prefix| ("prefix", prefix.as_str()))
                    .collect(),
            ),
            Self::Balances {
                economy_id,
                account,
                prefix,
            } => (
                under(["economy", economy_id.as_str(), "accounts"], account),
                prefix
                    .iter()
                    .map(|prefix| ("prefix", prefix.as_str()))
                    .collect(),
            ),
            Self::Rates { economy_id, asset } => (
                vec!["economy", economy_id.as_str(), "rates"],
                asset
                    .iter()
                    .map(|asset| ("asset", asset.as_str()))
                    .collect(),
            ),
            Self::Rate { economy_id, rate } => (
                under(["economy", economy_id.as_str(), "rates"], rate),
                vec![],
            ),
            Self::RateAmounts { economy_id, rate } => (
                under(["economy", economy_id.as_str(), "rate-amounts"], rate),
                vec![],
            ),
            Self::RateConditions { economy_id, rate } => (
                under(["economy", economy_id.as_str(), "rate-conditions"], rate),
                vec![],
            ),
            Self::Invariants { economy_id } => {
                (vec!["economy", economy_id.as_str(), "invariants"], vec![])
            }
            Self::InvariantWeights {
                economy_id,
                invariant,
            } => (
                under(["economy", economy_id.as_str(), "invariants"], invariant),
                vec![],
            ),
            Self::Changes { economy_id } => {
                (vec!["economy", economy_id.as_str(), "changes"], vec![])
            }
            Self::Exchanges { economy_id } => {
                (vec!["economy", economy_id.as_str(), "exchanges"], vec![])
            }
            Self::Diff {
                economy_id,
                other_id,
            } => (
                vec!["economy", economy_id.as_str(), "diff", other_id.as_str()],
                vec![],
            ),
            Self::Assessment { assessment_id } => {
                (vec!["assessment", assessment_id.as_str()], vec![])
            }
            Self::AssessmentLines { assessment_id } => {
                (vec!["assessment", assessment_id.as_str(), "lines"], vec![])
            }
            Self::Artifact { artifact_id } => (vec!["artifact", artifact_id.as_str()], vec![]),
            Self::Document {
                artifact_id,
                document,
            } => (
                vec![
                    "artifact",
                    artifact_id.as_str(),
                    "documents",
                    document.as_str(),
                ],
                vec![],
            ),
            Self::Frames {
                artifact_id,
                document,
            } => (
                vec![
                    "artifact",
                    artifact_id.as_str(),
                    "documents",
                    document.as_str(),
                    "frames",
                ],
                vec![],
            ),
        };
        let path: Vec<String> = segments.into_iter().map(encode).collect();
        uri.push_str(&path.join("/"));
        push_query(&mut uri, &query);
        uri
    }

    /// The URI of the page that starts at `cursor`.
    #[cfg(test)]
    pub(crate) fn page_uri(&self, cursor: &str) -> String {
        let mut uri = self.uri();
        uri.push(if uri.contains('?') { '&' } else { '?' });
        uri.push_str("cursor=");
        uri.push_str(&encode(cursor));
        uri
    }

    #[cfg(test)]
    pub(crate) fn cursor(&self, position: usize) -> String {
        Cursor::new(self, position).to_string()
    }

    fn is_paged(&self) -> bool {
        !matches!(
            self,
            Self::Economy { .. }
                | Self::Rate { .. }
                | Self::Assessment { .. }
                | Self::Artifact { .. }
                | Self::Document { .. }
        )
    }

    /// A `resource_link` content block naming this target.
    pub(crate) fn link(&self, name: &str, description: impl Into<String>) -> Resource {
        Resource::new(self.uri(), name)
            .with_description(description)
            .with_mime_type(JSON)
    }
}

impl Address {
    pub(crate) fn parse(uri: &str) -> Result<Self, McpError> {
        let unknown = || McpError::invalid_params(format!("unknown resource URI `{uri}`"), None);
        let rest = uri.strip_prefix(SCHEME).ok_or_else(unknown)?;
        let (path, query) = rest.split_once('?').unwrap_or((rest, ""));
        let segments = path
            .split('/')
            .map(decode)
            .collect::<Option<Vec<String>>>()
            .ok_or_else(unknown)?;
        let mut parameters = Vec::new();
        for pair in query.split('&').filter(|pair| !pair.is_empty()) {
            let (key, value) = pair.split_once('=').ok_or_else(unknown)?;
            parameters.push((key.to_owned(), decode(value).ok_or_else(unknown)?));
        }
        let mut take = |name: &str| {
            parameters
                .iter()
                .position(|(key, _)| key == name)
                .map(|index| parameters.remove(index).1)
        };
        let cursor = take("cursor").map(|cursor| cursor.parse()).transpose()?;
        let segments: Vec<&str> = segments.iter().map(String::as_str).collect();
        let target = match segments.as_slice() {
            ["economy", id] => Target::Economy {
                economy_id: id.parse().map_err(|_| unknown())?,
            },
            ["economy", id, "accounts"] => Target::Accounts {
                economy_id: id.parse().map_err(|_| unknown())?,
                prefix: take("prefix"),
            },
            ["economy", id, "accounts", account @ ..] => Target::Balances {
                economy_id: id.parse().map_err(|_| unknown())?,
                account: joined(account).ok_or_else(unknown)?,
                prefix: take("prefix"),
            },
            ["economy", id, "rates"] => Target::Rates {
                economy_id: id.parse().map_err(|_| unknown())?,
                asset: take("asset")
                    .map(|asset| asset.parse())
                    .transpose()
                    .map_err(|_| unknown())?,
            },
            ["economy", id, "rates", rate @ ..] => Target::Rate {
                economy_id: id.parse().map_err(|_| unknown())?,
                rate: joined(rate).ok_or_else(unknown)?,
            },
            ["economy", id, "rate-amounts", rate @ ..] => Target::RateAmounts {
                economy_id: id.parse().map_err(|_| unknown())?,
                rate: joined(rate).ok_or_else(unknown)?,
            },
            ["economy", id, "rate-conditions", rate @ ..] => Target::RateConditions {
                economy_id: id.parse().map_err(|_| unknown())?,
                rate: joined(rate).ok_or_else(unknown)?,
            },
            ["economy", id, "invariants"] => Target::Invariants {
                economy_id: id.parse().map_err(|_| unknown())?,
            },
            ["economy", id, "invariants", invariant @ ..] => Target::InvariantWeights {
                economy_id: id.parse().map_err(|_| unknown())?,
                invariant: joined(invariant).ok_or_else(unknown)?,
            },
            ["economy", id, "changes"] => Target::Changes {
                economy_id: id.parse().map_err(|_| unknown())?,
            },
            ["economy", id, "exchanges"] => Target::Exchanges {
                economy_id: id.parse().map_err(|_| unknown())?,
            },
            ["economy", id, "diff", other] => Target::Diff {
                economy_id: id.parse().map_err(|_| unknown())?,
                other_id: other.parse().map_err(|_| unknown())?,
            },
            ["assessment", id] => Target::Assessment {
                assessment_id: id.parse().map_err(|_| unknown())?,
            },
            ["assessment", id, "lines"] => Target::AssessmentLines {
                assessment_id: id.parse().map_err(|_| unknown())?,
            },
            ["artifact", id] => Target::Artifact {
                artifact_id: id.parse().map_err(|_| unknown())?,
            },
            ["artifact", id, "documents", document] => Target::Document {
                artifact_id: id.parse().map_err(|_| unknown())?,
                document: document.parse().map_err(|_| unknown())?,
            },
            ["artifact", id, "documents", document, "frames"] => Target::Frames {
                artifact_id: id.parse().map_err(|_| unknown())?,
                document: document.parse().map_err(|_| unknown())?,
            },
            _ => return Err(unknown()),
        };
        if let Some((key, _)) = parameters.first() {
            return Err(McpError::invalid_params(
                format!("resource URI `{uri}` does not take the parameter `{key}`"),
                None,
            ));
        }
        if cursor.is_some() && !target.is_paged() {
            return Err(McpError::invalid_params(
                format!("resource `{uri}` is not paged and takes no cursor"),
                None,
            ));
        }
        Ok(Self { target, cursor })
    }

    /// The position the cursor resumes from, zero without one.
    pub(crate) fn start(&self) -> Result<usize, McpError> {
        match self.cursor {
            None => Ok(0),
            Some(cursor) if cursor.tag == listing_tag(&self.target) && cursor.position > 0 => {
                Ok(cursor.position)
            }
            Some(cursor) => Err(invalid_cursor(cursor)),
        }
    }
}

/// Path segments after a fixed prefix, as the segment list of a URI.
fn under<'a, K>(prefix: [&'a str; 3], path: &'a Path<K>) -> Vec<&'a str> {
    prefix.into_iter().chain(path.segments()).collect()
}

/// The path the remaining URI segments spell, if they spell one.
fn joined<K>(segments: &[&str]) -> Option<Path<K>> {
    segments.join("/").parse().ok()
}

fn invalid_cursor(cursor: Cursor) -> McpError {
    McpError::invalid_params(format!("invalid cursor `{cursor}`"), None)
}

/// The first four bytes of the listing URI's hash.
fn listing_tag(target: &Target) -> u32 {
    let digest = blake3::hash(target.uri().as_bytes());
    u32::from_be_bytes(
        *digest
            .as_bytes()
            .first_chunk()
            .expect("a digest has 32 bytes"),
    )
}

/// One page of a listing as JSON.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Page<T> {
    pub items: Vec<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<Cursor>,
}

/// Up to `size` items from position `start` of a listing whose items carry
/// their position in the unfiltered sequence. A cursor names the position of
/// the next item, so it stays valid whatever a filter skips.
pub(crate) fn page<T>(
    address: &Address,
    items: impl Iterator<Item = (usize, T)>,
    size: usize,
) -> Result<Page<T>, McpError> {
    let start = address.start()?;
    let mut page = Vec::with_capacity(size.min(64));
    let mut next_cursor = None;
    for (position, item) in items.skip_while(|(position, _)| *position < start) {
        if page.len() == size {
            next_cursor = Some(Cursor::new(&address.target, position));
            break;
        }
        page.push(item);
    }
    if page.is_empty()
        && let Some(cursor) = address.cursor
    {
        return Err(invalid_cursor(cursor));
    }
    Ok(Page {
        items: page,
        next_cursor,
    })
}

/// One page of a listing that occupies the positions `span`: the positions to
/// read for this page, and the cursor of the next one. Positions are those of
/// the unfiltered sequence, so a cursor locates its item directly.
pub(crate) fn span_page(
    address: &Address,
    span: std::ops::Range<usize>,
    size: usize,
) -> Result<(std::ops::Range<usize>, Option<Cursor>), McpError> {
    let start = match address.cursor {
        None => span.start,
        Some(cursor) => {
            let position = address.start()?;
            if !span.contains(&position) {
                return Err(invalid_cursor(cursor));
            }
            position
        }
    };
    let end = span.end.min(start.saturating_add(size));
    let next_cursor = (end < span.end).then(|| Cursor::new(&address.target, end));
    Ok((start..end, next_cursor))
}

fn push_query(uri: &mut String, query: &[(&str, &str)]) {
    for (index, (key, value)) in query.iter().enumerate() {
        uri.push(if index == 0 { '?' } else { '&' });
        uri.push_str(key);
        uri.push('=');
        uri.push_str(&encode(value));
    }
}

/// Percent-encodes everything but RFC 3986 unreserved characters, as RFC 6570
/// simple and query expansion do.
fn encode(text: &str) -> String {
    let mut encoded = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            encoded.push(char::from(byte));
        } else {
            let _ = write!(encoded, "%{byte:02X}");
        }
    }
    encoded
}

fn decode(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let hex = text.get(index + 1..index + 3)?;
            decoded.push(u8::from_str_radix(hex, 16).ok()?);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(decoded).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn economy() -> EconomyId {
        EconomyId::from_digest(&blake3::hash(b"economy"))
    }

    #[test]
    fn every_target_round_trips_through_its_uri() {
        let targets = [
            Target::Economy {
                economy_id: economy(),
            },
            Target::Accounts {
                economy_id: economy(),
                prefix: Some("agents/7/".into()),
            },
            Target::Balances {
                economy_id: economy(),
                account: "agents/7/wallet".parse().unwrap(),
                prefix: Some("sku/00".into()),
            },
            Target::Rates {
                economy_id: economy(),
                asset: Some("sku/42".parse().unwrap()),
            },
            Target::RateAmounts {
                economy_id: economy(),
                rate: "market/swap".parse().unwrap(),
            },
            Target::InvariantWeights {
                economy_id: economy(),
                invariant: "supply/cash".parse().unwrap(),
            },
            Target::Frames {
                artifact_id: format!("art-{}", "0".repeat(64)).parse().unwrap(),
                document: "outcomes/a-star".parse().unwrap(),
            },
        ];
        for target in targets {
            let address = Address::parse(&target.uri()).unwrap();
            assert_eq!(address.target, target);
            assert_eq!(address.cursor, None);
        }
    }

    #[test]
    fn paths_compose_into_uris_unescaped() {
        let target = Target::Balances {
            economy_id: economy(),
            account: "agents/7/wallet".parse().unwrap(),
            prefix: None,
        };
        assert_eq!(
            target.uri(),
            format!("axionomy://economy/{}/accounts/agents/7/wallet", economy())
        );
    }

    #[test]
    fn pages_resume_at_their_cursor_and_refuse_cursors_from_other_listings() {
        let target = Target::Accounts {
            economy_id: economy(),
            prefix: None,
        };
        let first = Address::parse(&target.uri()).unwrap();
        let page_one = page(&first, (0..7).enumerate(), 3).unwrap();
        assert_eq!(page_one.items, [0, 1, 2]);
        let next = page_one.next_cursor.unwrap().to_string();

        let second = Address::parse(&target.page_uri(&next)).unwrap();
        let page_two = page(&second, (0..7).enumerate(), 3).unwrap();
        assert_eq!(page_two.items, [3, 4, 5]);
        let last = page_two.next_cursor.unwrap().to_string();
        let page_three = page(
            &Address::parse(&target.page_uri(&last)).unwrap(),
            (0..7).enumerate(),
            3,
        )
        .unwrap();
        assert_eq!(page_three.items, [6]);
        assert_eq!(page_three.next_cursor, None);

        let other = Target::Invariants {
            economy_id: economy(),
        };
        let foreign = Address::parse(&other.page_uri(&next)).unwrap();
        assert!(page(&foreign, (0..7).enumerate(), 3).is_err());
        let beyond = Address::parse(&target.page_uri(&target.cursor(99))).unwrap();
        assert!(page(&beyond, (0..7).enumerate(), 3).is_err());
        assert!(Address::parse(&target.page_uri("not-a-cursor")).is_err());
    }

    #[test]
    fn unpaged_targets_unknown_parameters_and_forged_handles_are_refused() {
        let economy = economy();
        assert!(Address::parse(&format!("axionomy://economy/{economy}?cursor=1.0")).is_err());
        assert!(Address::parse(&format!("axionomy://economy/{economy}/accounts?limit=5")).is_err());
        assert!(Address::parse(&format!("axionomy://economy/{economy}/nothing")).is_err());
        assert!(
            Address::parse(&format!("axionomy://economy/{economy}/accounts/Bad_Path")).is_err()
        );
        assert!(Address::parse("axionomy://economy/eco-1").is_err());
        assert!(Address::parse(&format!("file:///economy/{economy}")).is_err());
    }
}
