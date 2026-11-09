#![doc = include_str!("../README.md")]

mod build;
mod node;

use build::{Builder, rebuild};
pub use node::{Bytes, Hash};
use node::{Child, Decoded, Entry, Items, Node, decode, digest, ends};
use std::{cmp::Ordering, fmt, iter::Peekable, sync::Arc};
use thiserror::Error;

/// A persistent ordered map from byte keys to byte values whose shape depends
/// only on its keys: the same keys give the same tree, and so the same
/// [`hash`](Tree::hash), however the tree was built.
///
/// Cloning a tree copies a pointer. An edit copies the nodes from the root to
/// the entries it changes and shares the rest with every other version.
#[derive(Clone)]
pub struct Tree {
    root: Arc<Node>,
}

/// One key whose value differs between two trees.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Change<'a> {
    pub key: &'a [u8],
    pub before: Option<&'a [u8]>,
    pub after: Option<&'a [u8]>,
}

/// Why a tree could not be loaded.
#[derive(Debug, Error)]
pub enum LoadError<E> {
    #[error("a node could not be read")]
    Fetch(#[source] E),
    #[error("node {0} is missing")]
    Missing(Hash),
    #[error("node {node} is malformed: {reason}")]
    Malformed { node: Hash, reason: &'static str },
}

impl Tree {
    pub fn new() -> Self {
        Self {
            root: Node::leaf(Vec::new()),
        }
    }

    pub fn len(&self) -> u64 {
        self.root.count
    }

    pub fn is_empty(&self) -> bool {
        self.root.count == 0
    }

    pub fn get(&self, key: &[u8]) -> Option<&[u8]> {
        let mut node = &*self.root;
        loop {
            match &node.items {
                Items::Leaf(entries) => {
                    return entries
                        .binary_search_by(|entry| entry.key[..].cmp(key))
                        .ok()
                        .map(|index| &*entries[index].value);
                }
                Items::Branch(children) => {
                    let index = children.partition_point(|child| child.key[..] < *key);
                    node = &children.get(index)?.node;
                }
            }
        }
    }

    pub fn insert(&mut self, key: impl Into<Bytes>, value: impl Into<Bytes>) {
        self.update([(key.into(), Some(value.into()))]);
    }

    pub fn remove(&mut self, key: &[u8]) {
        self.update([(Bytes::from(key), None::<Bytes>)]);
    }

    /// Applies a batch of edits: a value sets its key, `None` removes it, and
    /// the last edit of a key wins.
    pub fn update<K: Into<Bytes>, V: Into<Bytes>>(
        &mut self,
        edits: impl IntoIterator<Item = (K, Option<V>)>,
    ) {
        let edits = last_wins(
            edits
                .into_iter()
                .map(|(key, value)| (key.into(), value.map(Into::into))),
        );
        if !edits.is_empty() {
            self.root = rebuild(&self.root, &edits);
        }
    }

    /// Entries in key order.
    pub fn iter(&self) -> Iter<'_> {
        Iter {
            stack: vec![(&self.root, 0)],
        }
    }

    /// Entries in key order, from the first key at or after `key`.
    pub fn iter_from(&self, key: &[u8]) -> Iter<'_> {
        let mut stack = Vec::new();
        let mut node = &*self.root;
        loop {
            match &node.items {
                Items::Leaf(entries) => {
                    stack.push((node, entries.partition_point(|entry| entry.key[..] < *key)));
                    return Iter { stack };
                }
                Items::Branch(children) => {
                    let index = children.partition_point(|child| child.key[..] < *key);
                    stack.push((node, index + 1));
                    match children.get(index) {
                        Some(child) => node = &child.node,
                        None => return Iter { stack },
                    }
                }
            }
        }
    }

    /// Entries in key order, from the one at `position`.
    pub fn iter_at(&self, mut position: u64) -> Iter<'_> {
        let mut stack = Vec::new();
        let mut node = &*self.root;
        loop {
            match &node.items {
                Items::Leaf(entries) => {
                    let index = usize::try_from(position)
                        .map_or(entries.len(), |index| index.min(entries.len()));
                    stack.push((node, index));
                    return Iter { stack };
                }
                Items::Branch(children) => {
                    let mut index = 0;
                    while let Some(child) = children.get(index) {
                        if position < child.node.count {
                            break;
                        }
                        position -= child.node.count;
                        index += 1;
                    }
                    stack.push((node, index + 1));
                    match children.get(index) {
                        Some(child) => node = &child.node,
                        None => return Iter { stack },
                    }
                }
            }
        }
    }

    /// Entries whose keys start with `prefix`, in key order.
    pub fn prefix<'a>(&'a self, prefix: &'a [u8]) -> impl Iterator<Item = (&'a [u8], &'a [u8])> {
        self.iter_from(prefix)
            .take_while(move |(key, _)| key.starts_with(prefix))
    }

    /// How many keys sort before `key`: the position `key` has, or would have.
    pub fn rank(&self, key: &[u8]) -> u64 {
        let mut rank = 0;
        let mut node = &*self.root;
        loop {
            match &node.items {
                Items::Leaf(entries) => {
                    return rank + entries.partition_point(|entry| entry.key[..] < *key) as u64;
                }
                Items::Branch(children) => {
                    let index = children.partition_point(|child| child.key[..] < *key);
                    rank += children[..index]
                        .iter()
                        .map(|child| child.node.count)
                        .sum::<u64>();
                    match children.get(index) {
                        Some(child) => node = &child.node,
                        None => return rank,
                    }
                }
            }
        }
    }

    /// The tree's content hash: equal for equal entries, whatever their
    /// history. Computed for each node the first time it is asked for, and
    /// remembered by nodes shared with other versions.
    pub fn hash(&self) -> Hash {
        self.root.hash()
    }

    /// The keys whose values differ from `self` to `after`, in key order.
    /// Subtrees with equal hashes are skipped without being read.
    pub fn diff<'a>(&'a self, after: &'a Self) -> Vec<Change<'a>> {
        diff(&self.root, &after.root)
    }

    /// Each node `known` does not report, with its hash and encoding, parents
    /// before children. A subtree whose root is known is skipped: a store
    /// that holds a node holds everything under it.
    pub fn missing(&self, mut known: impl FnMut(&Hash) -> bool) -> Vec<(Hash, Vec<u8>)> {
        let mut missing = Vec::new();
        let mut stack = vec![&self.root];
        while let Some(node) = stack.pop() {
            let hash = node.hash();
            if known(&hash) {
                continue;
            }
            missing.push((hash, node.encode()));
            if let Items::Branch(children) = &node.items {
                stack.extend(children.iter().rev().map(|child| &child.node));
            }
        }
        missing
    }

    /// Loads the tree whose hash is `root`, reading each node's encoding with
    /// `fetch`. Every node is checked against its hash, and the tree against
    /// the shape its keys determine, so a loaded tree is the one tree those
    /// entries have.
    pub fn load<E>(
        root: &Hash,
        mut fetch: impl FnMut(&Hash) -> Result<Option<Vec<u8>>, E>,
    ) -> Result<Self, LoadError<E>> {
        let root = load(root, &mut fetch, None, None, true)?;
        if let Items::Branch(children) = &root.items
            && children.len() < 2
        {
            return Err(LoadError::Malformed {
                node: root.hash(),
                reason: "a root with one child is that child's tree",
            });
        }
        Ok(Self { root })
    }
}

impl Default for Tree {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for Tree {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_map().entries(self.iter()).finish()
    }
}

impl<K: Into<Bytes>, V: Into<Bytes>> FromIterator<(K, V)> for Tree {
    /// Builds a tree from entries in any order; the last value of a key wins.
    fn from_iter<I: IntoIterator<Item = (K, V)>>(entries: I) -> Self {
        let mut builder = Builder::default();
        for (key, value) in last_wins(
            entries
                .into_iter()
                .map(|(key, value)| (key.into(), value.into())),
        ) {
            builder.push_entry(Entry::new(key, value));
        }
        Self {
            root: builder.finish(),
        }
    }
}

impl<'a> IntoIterator for &'a Tree {
    type Item = (&'a [u8], &'a [u8]);
    type IntoIter = Iter<'a>;

    fn into_iter(self) -> Iter<'a> {
        self.iter()
    }
}

/// Sorts by key, keeping the last value given for each key.
fn last_wins<T>(items: impl IntoIterator<Item = (Bytes, T)>) -> Vec<(Bytes, T)> {
    let mut items: Vec<_> = items.into_iter().collect();
    items.sort_by(|left, right| left.0.cmp(&right.0));
    let mut kept: Vec<(Bytes, T)> = Vec::with_capacity(items.len());
    for (key, value) in items {
        match kept.last_mut() {
            Some((last, kept_value)) if *last == key => *kept_value = value,
            _ => kept.push((key, value)),
        }
    }
    kept
}

/// An iterator over a tree's entries in key order.
pub struct Iter<'a> {
    /// The path to the next entry: each node with the index of its next item.
    stack: Vec<(&'a Node, usize)>,
}

impl<'a> Iterator for Iter<'a> {
    type Item = (&'a [u8], &'a [u8]);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let (node, index) = self.stack.last_mut()?;
            match &node.items {
                Items::Leaf(entries) => {
                    if let Some(entry) = entries.get(*index) {
                        *index += 1;
                        return Some((&entry.key, &entry.value));
                    }
                }
                Items::Branch(children) => {
                    if let Some(child) = children.get(*index) {
                        *index += 1;
                        self.stack.push((&child.node, 0));
                        continue;
                    }
                }
            }
            self.stack.pop();
        }
    }
}

fn load<E>(
    hash: &Hash,
    fetch: &mut impl FnMut(&Hash) -> Result<Option<Vec<u8>>, E>,
    level: Option<u8>,
    after: Option<&[u8]>,
    last: bool,
) -> Result<Arc<Node>, LoadError<E>> {
    let malformed = |reason| LoadError::Malformed {
        node: *hash,
        reason,
    };
    let bytes = fetch(hash)
        .map_err(LoadError::Fetch)?
        .ok_or(LoadError::Missing(*hash))?;
    if digest(&bytes) != *hash {
        return Err(malformed("its encoding does not have its hash"));
    }
    let decoded = decode(&bytes).map_err(malformed)?;
    let node_level = match &decoded {
        Decoded::Leaf(_) => 0,
        Decoded::Branch { level, .. } => *level,
    };
    if level.is_some_and(|level| level != node_level) {
        return Err(malformed("it is not at the level below its parent"));
    }
    let keys: Vec<&[u8]> = match &decoded {
        Decoded::Leaf(entries) => entries.iter().map(|(key, _)| &**key).collect(),
        Decoded::Branch { children, .. } => children.iter().map(|(key, ..)| &**key).collect(),
    };
    if keys.is_empty() && (level.is_some() || node_level > 0) {
        return Err(malformed("only an empty tree's root is empty"));
    }
    let mut previous = after;
    for &key in &keys {
        if previous.is_some_and(|previous| previous >= key) {
            return Err(malformed("its keys are not in increasing order"));
        }
        previous = Some(key);
    }
    let marks: Vec<u64> = keys.iter().map(|key| node::mark(key)).collect();
    for (index, mark) in marks.iter().enumerate() {
        let is_last = index + 1 == marks.len();
        if ends(*mark, node_level, index + 1) != is_last && !(is_last && last) {
            return Err(malformed("it does not end where its keys end it"));
        }
    }
    let node = match decoded {
        Decoded::Leaf(entries) => {
            let count = entries.len() as u64;
            let entries = entries
                .into_iter()
                .zip(marks)
                .map(|((key, value), mark)| Entry { key, value, mark })
                .collect();
            Node::with_hash(0, count, Items::Leaf(entries), *hash)
        }
        Decoded::Branch { level, children } => {
            let final_index = children.len() - 1;
            let mut loaded = Vec::with_capacity(children.len());
            let mut count = 0u64;
            let mut after = after;
            for (index, ((key, child_count, child_hash), mark)) in
                children.iter().zip(marks).enumerate()
            {
                let child = load(
                    child_hash,
                    fetch,
                    Some(level - 1),
                    after,
                    last && index == final_index,
                )?;
                if child.last_key() != Some(&**key) || child.count != *child_count {
                    return Err(malformed("a child is not what its parent says"));
                }
                count = count
                    .checked_add(child.count)
                    .ok_or_else(|| malformed("it counts more entries than fit in 64 bits"))?;
                after = Some(key);
                loaded.push(Child {
                    key: Arc::clone(key),
                    mark,
                    node: child,
                });
            }
            Node::with_hash(level, count, Items::Branch(loaded), *hash)
        }
    };
    Ok(node)
}

fn diff<'a>(before: &'a Arc<Node>, after: &'a Arc<Node>) -> Vec<Change<'a>> {
    let frontier = |root: &'a Arc<Node>| -> Vec<&'a Arc<Node>> {
        if root.count == 0 {
            Vec::new()
        } else {
            vec![root]
        }
    };
    let (mut left, mut right) = (frontier(before), frontier(after));
    let mut left_level = before.level;
    let mut right_level = after.level;
    if left.is_empty() {
        left_level = right_level;
    }
    if right.is_empty() {
        right_level = left_level;
    }
    while left_level > right_level {
        left = expand(&left);
        left_level -= 1;
    }
    while right_level > left_level {
        right = expand(&right);
        right_level -= 1;
    }
    loop {
        (left, right) = without_shared(&left, &right);
        if left_level == 0 {
            return changes(&left, &right);
        }
        left = expand(&left);
        right = expand(&right);
        left_level -= 1;
    }
}

fn expand<'a>(nodes: &[&'a Arc<Node>]) -> Vec<&'a Arc<Node>> {
    nodes
        .iter()
        .flat_map(|node| match &node.items {
            Items::Branch(children) => children.iter().map(|child| &child.node).collect(),
            Items::Leaf(_) => Vec::new(),
        })
        .collect()
}

/// Drops the subtrees present on both sides. Equal subtrees end at the same
/// key, so they meet when both sides are walked by last key.
fn without_shared<'a>(
    left: &[&'a Arc<Node>],
    right: &[&'a Arc<Node>],
) -> (Vec<&'a Arc<Node>>, Vec<&'a Arc<Node>>) {
    let (mut kept_left, mut kept_right) = (Vec::new(), Vec::new());
    let mut left = left.iter().copied().peekable();
    let mut right = right.iter().copied().peekable();
    loop {
        match (left.peek().copied(), right.peek().copied()) {
            (Some(a), Some(b)) => match a.last_key().cmp(&b.last_key()) {
                Ordering::Less => kept_left.extend(left.next()),
                Ordering::Greater => kept_right.extend(right.next()),
                Ordering::Equal => {
                    if !(Arc::ptr_eq(a, b) || a.hash() == b.hash()) {
                        kept_left.push(a);
                        kept_right.push(b);
                    }
                    left.next();
                    right.next();
                }
            },
            _ => {
                kept_left.extend(left);
                kept_right.extend(right);
                return (kept_left, kept_right);
            }
        }
    }
}

fn changes<'a>(left: &[&'a Arc<Node>], right: &[&'a Arc<Node>]) -> Vec<Change<'a>> {
    fn entries<'a>(leaves: &[&'a Arc<Node>]) -> Peekable<impl Iterator<Item = &'a Entry>> {
        leaves
            .iter()
            .flat_map(|leaf| -> &'a [Entry] {
                match &leaf.items {
                    Items::Leaf(entries) => entries,
                    Items::Branch(_) => &[],
                }
            })
            .peekable()
    }
    let (mut left, mut right) = (entries(left), entries(right));
    let mut changes = Vec::new();
    loop {
        let order = match (left.peek(), right.peek()) {
            (None, None) => return changes,
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (Some(a), Some(b)) => a.key.cmp(&b.key),
        };
        let (before, after) = match order {
            Ordering::Less => (left.next(), None),
            Ordering::Greater => (None, right.next()),
            Ordering::Equal => (left.next(), right.next()),
        };
        if before.map(|entry| &entry.value) != after.map(|entry| &entry.value) {
            let key = before
                .or(after)
                .map(|entry| &*entry.key)
                .unwrap_or_default();
            changes.push(Change {
                key,
                before: before.map(|entry| &*entry.value),
                after: after.map(|entry| &*entry.value),
            });
        }
    }
}

#[cfg(test)]
mod tests;
