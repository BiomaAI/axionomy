//! Nodes, the rule that decides where they end, and their encoding.

use std::sync::{Arc, LazyLock, OnceLock};

pub use blake3::Hash;

/// Shared, immutable bytes, cloned by reference count.
pub type Bytes = Arc<[u8]>;

/// A key ends a node with probability 2^-5 at each level, so nodes hold 32
/// items on average.
const FANOUT_BITS: u32 = 5;
const FANOUT_MASK: u64 = (1 << FANOUT_BITS) - 1;
/// Levels whose boundaries a key's 64-bit mark decides. Above them only
/// `MAX_ITEMS` ends a node; no tree of fewer than 2^60 entries gets there.
const MARKED_LEVELS: u8 = (u64::BITS / FANOUT_BITS) as u8;
/// The most items a node holds. Keys chosen so that none of them ends a node
/// still produce nodes of at most this many items.
pub(crate) const MAX_ITEMS: usize = 256;

static MARK_KEY: LazyLock<[u8; 32]> =
    LazyLock::new(|| blake3::derive_key("axionomy-tree 2026-11-09 boundary", b""));
static NODE_KEY: LazyLock<[u8; 32]> =
    LazyLock::new(|| blake3::derive_key("axionomy-tree 2026-11-09 node", b""));

/// The bits of a key that decide whether it ends a node, at every level.
pub(crate) fn mark(key: &[u8]) -> u64 {
    let hash = blake3::keyed_hash(&MARK_KEY, key);
    let mut bytes = [0; 8];
    bytes.copy_from_slice(&hash.as_bytes()[..8]);
    u64::from_le_bytes(bytes)
}

/// Whether an item whose key has `mark`, placed `len`th in a node at `level`,
/// ends that node. The key alone decides, so changing a value never moves a
/// boundary.
pub(crate) fn ends(mark: u64, level: u8, len: usize) -> bool {
    len >= MAX_ITEMS
        || (level < MARKED_LEVELS && (mark >> (FANOUT_BITS * u32::from(level))) & FANOUT_MASK == 0)
}

/// The content hash of a node's encoding.
pub(crate) fn digest(encoding: &[u8]) -> Hash {
    blake3::keyed_hash(&NODE_KEY, encoding)
}

#[derive(Clone)]
pub(crate) struct Entry {
    pub key: Bytes,
    pub value: Bytes,
    pub mark: u64,
}

impl Entry {
    pub fn new(key: Bytes, value: Bytes) -> Self {
        let mark = mark(&key);
        Self { key, value, mark }
    }
}

/// A subtree as an item of its parent, under the subtree's last key.
#[derive(Clone)]
pub(crate) struct Child {
    pub key: Bytes,
    pub mark: u64,
    pub node: Arc<Node>,
}

impl Child {
    pub fn of(node: Arc<Node>) -> Self {
        let (key, mark) = match &node.items {
            Items::Leaf(entries) => entries.last().map(|entry| (&entry.key, entry.mark)),
            Items::Branch(children) => children.last().map(|child| (&child.key, child.mark)),
        }
        .expect("only an empty tree's root has no items");
        Self {
            key: Arc::clone(key),
            mark,
            node,
        }
    }
}

pub(crate) enum Items {
    Leaf(Vec<Entry>),
    Branch(Vec<Child>),
}

pub(crate) struct Node {
    pub level: u8,
    /// Entries in this subtree.
    pub count: u64,
    pub items: Items,
    hash: OnceLock<Hash>,
}

impl Node {
    pub fn leaf(entries: Vec<Entry>) -> Arc<Self> {
        Arc::new(Self {
            level: 0,
            count: entries.len() as u64,
            items: Items::Leaf(entries),
            hash: OnceLock::new(),
        })
    }

    pub fn branch(level: u8, children: Vec<Child>) -> Arc<Self> {
        Arc::new(Self {
            level,
            count: children.iter().map(|child| child.node.count).sum(),
            items: Items::Branch(children),
            hash: OnceLock::new(),
        })
    }

    pub fn len(&self) -> usize {
        match &self.items {
            Items::Leaf(entries) => entries.len(),
            Items::Branch(children) => children.len(),
        }
    }

    /// The mark of the last item, which decides whether the node ends where
    /// it does.
    pub fn last_mark(&self) -> Option<u64> {
        match &self.items {
            Items::Leaf(entries) => entries.last().map(|entry| entry.mark),
            Items::Branch(children) => children.last().map(|child| child.mark),
        }
    }

    pub fn last_key(&self) -> Option<&[u8]> {
        match &self.items {
            Items::Leaf(entries) => entries.last().map(|entry| &*entry.key),
            Items::Branch(children) => children.last().map(|child| &*child.key),
        }
    }

    /// The node's content hash, computed once and remembered.
    pub fn hash(&self) -> Hash {
        *self.hash.get_or_init(|| digest(&self.encode()))
    }

    /// The node's deterministic encoding: its level, its item count, then
    /// each entry's key and value, or each child's last key, entry count and
    /// hash, with lengths and counts as LEB128.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = vec![self.level];
        put_varint(&mut out, self.len() as u64);
        match &self.items {
            Items::Leaf(entries) => {
                for entry in entries {
                    put_bytes(&mut out, &entry.key);
                    put_bytes(&mut out, &entry.value);
                }
            }
            Items::Branch(children) => {
                for child in children {
                    put_bytes(&mut out, &child.key);
                    put_varint(&mut out, child.node.count);
                    out.extend_from_slice(child.node.hash().as_bytes());
                }
            }
        }
        out
    }

    /// A node decoded from bytes whose hash the caller has already checked.
    pub fn with_hash(level: u8, count: u64, items: Items, hash: Hash) -> Arc<Self> {
        Arc::new(Self {
            level,
            count,
            items,
            hash: OnceLock::from(hash),
        })
    }
}

/// A node as its encoding states it, before its children are resolved.
pub(crate) enum Decoded {
    Leaf(Vec<(Bytes, Bytes)>),
    Branch {
        level: u8,
        children: Vec<(Bytes, u64, Hash)>,
    },
}

/// Decodes exactly what [`Node::encode`] produces, rejecting anything else.
pub(crate) fn decode(mut input: &[u8]) -> Result<Decoded, &'static str> {
    let input = &mut input;
    let level = take(input, 1)?[0];
    let len = take_varint(input)?;
    if len > MAX_ITEMS as u64 {
        return Err("a node holds more items than any node can");
    }
    let decoded = if level == 0 {
        let mut entries = Vec::with_capacity(len as usize);
        for _ in 0..len {
            let key = take_bytes(input)?;
            let value = take_bytes(input)?;
            entries.push((key, value));
        }
        Decoded::Leaf(entries)
    } else {
        let mut children = Vec::with_capacity(len as usize);
        for _ in 0..len {
            let key = take_bytes(input)?;
            let count = take_varint(input)?;
            let mut hash = [0; 32];
            hash.copy_from_slice(take(input, 32)?);
            children.push((key, count, Hash::from_bytes(hash)));
        }
        Decoded::Branch { level, children }
    };
    if input.is_empty() {
        Ok(decoded)
    } else {
        Err("bytes follow the node's last item")
    }
}

fn put_varint(out: &mut Vec<u8>, mut value: u64) {
    while value >= 0x80 {
        out.push(value as u8 | 0x80);
        value >>= 7;
    }
    out.push(value as u8);
}

fn put_bytes(out: &mut Vec<u8>, bytes: &[u8]) {
    put_varint(out, bytes.len() as u64);
    out.extend_from_slice(bytes);
}

fn take<'a>(input: &mut &'a [u8], len: usize) -> Result<&'a [u8], &'static str> {
    if input.len() < len {
        return Err("the node ends early");
    }
    let (taken, rest) = input.split_at(len);
    *input = rest;
    Ok(taken)
}

/// Reads LEB128, accepting only the shortest encoding of each value.
fn take_varint(input: &mut &[u8]) -> Result<u64, &'static str> {
    let mut value = 0u64;
    for index in 0..10 {
        let byte = take(input, 1)?[0];
        let bits = u64::from(byte & 0x7f);
        if index == 9 && bits > 1 {
            return Err("a length overflows 64 bits");
        }
        value |= bits << (7 * index);
        if byte & 0x80 == 0 {
            if index > 0 && byte == 0 {
                return Err("a length is not in its shortest form");
            }
            return Ok(value);
        }
    }
    Err("a length overflows 64 bits")
}

fn take_bytes(input: &mut &[u8]) -> Result<Bytes, &'static str> {
    let len = usize::try_from(take_varint(input)?).map_err(|_| "a length overflows memory")?;
    Ok(take(input, len)?.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lengths_have_one_encoding() {
        for value in [0, 1, 127, 128, 300, u64::from(u32::MAX), u64::MAX] {
            let mut out = Vec::new();
            put_varint(&mut out, value);
            assert_eq!(take_varint(&mut out.as_slice()), Ok(value));
        }
        for overlong in [&[0x80, 0x00][..], &[0xff, 0x80, 0x00]] {
            assert!(take_varint(&mut &overlong[..]).is_err());
        }
        let mut overflow = [0xff; 10];
        overflow[9] = 0x02;
        assert!(take_varint(&mut &overflow[..]).is_err());
    }
}
