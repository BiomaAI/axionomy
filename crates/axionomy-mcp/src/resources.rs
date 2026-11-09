//! Resource addresses. Everything an economy, assessment or run holds is read
//! as an `axionomy://` resource, and every list is read one page at a time with
//! the MCP pagination conventions: an opaque `cursor` chosen by the server,
//! returned as `nextCursor` while more items remain. The named snapshots never
//! change, so a page's content never goes stale.

use rmcp::{
    ErrorData as McpError,
    model::{Resource, ResourceTemplate},
};
use serde::Serialize;
use std::fmt::Write as _;

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
        economy_id: String,
    },
    Accounts {
        economy_id: String,
    },
    Balances {
        economy_id: String,
        account: String,
        prefix: Option<String>,
    },
    Rates {
        economy_id: String,
        asset: Option<String>,
    },
    Rate {
        economy_id: String,
        rate: String,
    },
    RateAmounts {
        economy_id: String,
        rate: String,
    },
    RateConditions {
        economy_id: String,
        rate: String,
    },
    Invariants {
        economy_id: String,
    },
    InvariantWeights {
        economy_id: String,
        invariant: String,
    },
    Changes {
        economy_id: String,
    },
    Exchanges {
        economy_id: String,
    },
    Assessment {
        assessment_id: String,
    },
    AssessmentLines {
        assessment_id: String,
    },
    Artifact {
        artifact_id: String,
    },
    Document {
        artifact_id: String,
        document_id: String,
    },
    Frames {
        artifact_id: String,
        document_id: String,
    },
}

/// A parsed resource URI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Address {
    pub target: Target,
    pub cursor: Option<String>,
}

/// One URI template per kind of resource, in RFC 6570 syntax.
pub(crate) fn templates() -> Vec<ResourceTemplate> {
    [
        ("axionomy://economy/{economy_id}", "economy", "Economy snapshot", "How the snapshot was made and how many accounts, assets, rates and invariants it has."),
        ("axionomy://economy/{economy_id}/accounts{?cursor}", "accounts", "Accounts", "Accounts in insertion order, each with how many assets it holds."),
        ("axionomy://economy/{economy_id}/accounts/{account}/balances{?prefix,cursor}", "balances", "Account balances", "An account's nonzero balances, optionally only assets whose ID starts with `prefix`."),
        ("axionomy://economy/{economy_id}/rates{?asset,cursor}", "rates", "Rates", "Rates with their roles, optionally only rates that move `asset`."),
        ("axionomy://economy/{economy_id}/rates/{rate}", "rate", "Rate", "A rate's roles, distinct role pairs, parameters and how many amounts and conditions it has."),
        ("axionomy://economy/{economy_id}/rates/{rate}/amounts{?cursor}", "rate-amounts", "Rate amounts", "What one unit of the rate consumes, produces and preserves, by role: fixed quantities, then computed ones."),
        ("axionomy://economy/{economy_id}/rates/{rate}/conditions{?cursor}", "rate-conditions", "Rate conditions", "Comparisons every exchange of the rate must satisfy."),
        ("axionomy://economy/{economy_id}/invariants{?cursor}", "invariants", "Invariants", "Linear conservation laws, each with how many assets it weights."),
        ("axionomy://economy/{economy_id}/invariants/{invariant}/weights{?cursor}", "invariant-weights", "Invariant weights", "The integer weight of each asset in one conservation law."),
        ("axionomy://economy/{economy_id}/changes{?cursor}", "changes", "Changes", "The quantities moved by the exchanges that made this snapshot from its parent."),
        ("axionomy://economy/{economy_id}/exchanges{?cursor}", "exchanges", "Exchanges", "The exchanges that made this snapshot from its parent, in order; replaying them on the parent reproduces it."),
        ("axionomy://assessment/{assessment_id}", "assessment", "Assessment", "An exchange checked against a snapshot: its status, issues and how many lines explain it."),
        ("axionomy://assessment/{assessment_id}/lines{?cursor}", "assessment-lines", "Assessment lines", "What each account lacks for an infeasible exchange, or what an applicable one would move."),
        ("axionomy://artifact/{artifact_id}", "artifact", "Run artifact", "A finished problem run: its request and documents."),
        ("axionomy://artifact/{artifact_id}/documents/{document_id}", "document", "Run document", "One strategy's document without its frames: initial state, objectives and solver evidence."),
        ("axionomy://artifact/{artifact_id}/documents/{document_id}/frames{?cursor}", "frames", "Run frames", "A document's replay frames in order."),
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
            Self::Economy { economy_id } => (vec!["economy", economy_id], vec![]),
            Self::Accounts { economy_id } => (vec!["economy", economy_id, "accounts"], vec![]),
            Self::Balances {
                economy_id,
                account,
                prefix,
            } => (
                vec!["economy", economy_id, "accounts", account, "balances"],
                prefix
                    .iter()
                    .map(|prefix| ("prefix", prefix.as_str()))
                    .collect(),
            ),
            Self::Rates { economy_id, asset } => (
                vec!["economy", economy_id, "rates"],
                asset
                    .iter()
                    .map(|asset| ("asset", asset.as_str()))
                    .collect(),
            ),
            Self::Rate { economy_id, rate } => (vec!["economy", economy_id, "rates", rate], vec![]),
            Self::RateAmounts { economy_id, rate } => (
                vec!["economy", economy_id, "rates", rate, "amounts"],
                vec![],
            ),
            Self::RateConditions { economy_id, rate } => (
                vec!["economy", economy_id, "rates", rate, "conditions"],
                vec![],
            ),
            Self::Invariants { economy_id } => (vec!["economy", economy_id, "invariants"], vec![]),
            Self::InvariantWeights {
                economy_id,
                invariant,
            } => (
                vec!["economy", economy_id, "invariants", invariant, "weights"],
                vec![],
            ),
            Self::Changes { economy_id } => (vec!["economy", economy_id, "changes"], vec![]),
            Self::Exchanges { economy_id } => (vec!["economy", economy_id, "exchanges"], vec![]),
            Self::Assessment { assessment_id } => (vec!["assessment", assessment_id], vec![]),
            Self::AssessmentLines { assessment_id } => {
                (vec!["assessment", assessment_id, "lines"], vec![])
            }
            Self::Artifact { artifact_id } => (vec!["artifact", artifact_id], vec![]),
            Self::Document {
                artifact_id,
                document_id,
            } => (
                vec!["artifact", artifact_id, "documents", document_id],
                vec![],
            ),
            Self::Frames {
                artifact_id,
                document_id,
            } => (
                vec!["artifact", artifact_id, "documents", document_id, "frames"],
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
        let cursor = take("cursor");
        let segments: Vec<&str> = segments.iter().map(String::as_str).collect();
        let owned = |text: &str| text.to_owned();
        let target = match segments.as_slice() {
            ["economy", id] => Target::Economy {
                economy_id: owned(id),
            },
            ["economy", id, "accounts"] => Target::Accounts {
                economy_id: owned(id),
            },
            ["economy", id, "accounts", account, "balances"] => Target::Balances {
                economy_id: owned(id),
                account: owned(account),
                prefix: take("prefix"),
            },
            ["economy", id, "rates"] => Target::Rates {
                economy_id: owned(id),
                asset: take("asset"),
            },
            ["economy", id, "rates", rate] => Target::Rate {
                economy_id: owned(id),
                rate: owned(rate),
            },
            ["economy", id, "rates", rate, "amounts"] => Target::RateAmounts {
                economy_id: owned(id),
                rate: owned(rate),
            },
            ["economy", id, "rates", rate, "conditions"] => Target::RateConditions {
                economy_id: owned(id),
                rate: owned(rate),
            },
            ["economy", id, "invariants"] => Target::Invariants {
                economy_id: owned(id),
            },
            ["economy", id, "invariants", invariant, "weights"] => Target::InvariantWeights {
                economy_id: owned(id),
                invariant: owned(invariant),
            },
            ["economy", id, "changes"] => Target::Changes {
                economy_id: owned(id),
            },
            ["economy", id, "exchanges"] => Target::Exchanges {
                economy_id: owned(id),
            },
            ["assessment", id] => Target::Assessment {
                assessment_id: owned(id),
            },
            ["assessment", id, "lines"] => Target::AssessmentLines {
                assessment_id: owned(id),
            },
            ["artifact", id] => Target::Artifact {
                artifact_id: owned(id),
            },
            ["artifact", id, "documents", document] => Target::Document {
                artifact_id: owned(id),
                document_id: owned(document),
            },
            ["artifact", id, "documents", document, "frames"] => Target::Frames {
                artifact_id: owned(id),
                document_id: owned(document),
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
        let Some(cursor) = &self.cursor else {
            return Ok(0);
        };
        cursor
            .split_once('.')
            .filter(|(_, tag)| *tag == cursor_tag(&self.target))
            .and_then(|(position, _)| position.parse::<usize>().ok())
            .filter(|position| *position > 0)
            .ok_or_else(|| invalid_cursor(cursor))
    }
}

pub(crate) fn invalid_cursor(cursor: &str) -> McpError {
    McpError::invalid_params(format!("invalid cursor `{cursor}`"), None)
}

/// Ties a cursor to the listing it came from, so a cursor from one listing is
/// refused by another instead of silently reading the wrong rows.
fn cursor_tag(target: &Target) -> String {
    blake3::hash(target.uri().as_bytes()).to_hex()[..8].to_owned()
}

fn cursor(target: &Target, position: usize) -> String {
    format!("{position}.{}", cursor_tag(target))
}

/// One page of a listing as JSON.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Page<T> {
    pub items: Vec<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
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
            next_cursor = Some(cursor(&address.target, position));
            break;
        }
        page.push(item);
    }
    if page.is_empty()
        && let Some(cursor) = &address.cursor
    {
        return Err(invalid_cursor(cursor));
    }
    Ok(Page {
        items: page,
        next_cursor,
    })
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

    #[test]
    fn every_target_round_trips_through_its_uri() {
        let targets = [
            Target::Economy {
                economy_id: "eco_1".into(),
            },
            Target::Balances {
                economy_id: "eco_1".into(),
                account: "ware house/№1".into(),
                prefix: Some("sku:00".into()),
            },
            Target::Rates {
                economy_id: "eco_1".into(),
                asset: None,
            },
            Target::InvariantWeights {
                economy_id: "eco_1".into(),
                invariant: "supply & demand".into(),
            },
            Target::Frames {
                artifact_id: "run?1".into(),
                document_id: "a=b".into(),
            },
        ];
        for target in targets {
            let address = Address::parse(&target.uri()).unwrap();
            assert_eq!(address.target, target);
            assert_eq!(address.cursor, None);
        }
    }

    #[test]
    fn pages_resume_at_their_cursor_and_refuse_cursors_from_other_listings() {
        let target = Target::Accounts {
            economy_id: "eco_1".into(),
        };
        let first = Address::parse(&target.uri()).unwrap();
        let page_one = page(&first, (0..7).enumerate(), 3).unwrap();
        assert_eq!(page_one.items, [0, 1, 2]);
        let next = page_one.next_cursor.unwrap();

        let second = Address::parse(&target.page_uri(&next)).unwrap();
        let page_two = page(&second, (0..7).enumerate(), 3).unwrap();
        assert_eq!(page_two.items, [3, 4, 5]);
        let last = Address::parse(&target.page_uri(&page_two.next_cursor.unwrap())).unwrap();
        let page_three = page(&last, (0..7).enumerate(), 3).unwrap();
        assert_eq!(page_three.items, [6]);
        assert_eq!(page_three.next_cursor, None);

        let other = Target::Invariants {
            economy_id: "eco_1".into(),
        };
        let foreign = Address::parse(&other.page_uri(&next)).unwrap();
        assert!(page(&foreign, (0..7).enumerate(), 3).is_err());
        let beyond = Address::parse(&target.page_uri(&cursor(&target, 99))).unwrap();
        assert!(page(&beyond, (0..7).enumerate(), 3).is_err());
    }

    #[test]
    fn unpaged_targets_and_unknown_parameters_are_refused() {
        assert!(Address::parse("axionomy://economy/eco_1?cursor=1.x").is_err());
        assert!(Address::parse("axionomy://economy/eco_1/accounts?limit=5").is_err());
        assert!(Address::parse("axionomy://economy/eco_1/nothing").is_err());
        assert!(Address::parse("file:///economy/eco_1").is_err());
    }
}
