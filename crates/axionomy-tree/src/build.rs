//! Building canonical trees, from scratch or by editing an existing one.
//!
//! A builder chunks a sorted stream of entries into leaves, the leaves into
//! branches, and so on up, ending each node where [`ends`] says. Editing walks
//! the old tree in key order and feeds the builder the edited entries, but
//! hands it a whole unchanged subtree wherever chunking would reproduce that
//! subtree exactly. The result is the tree a from-scratch build of the new
//! entries gives, at a cost proportional to the edits.

use crate::node::{Bytes, Child, Entry, Items, Node, ends};
use std::{mem, sync::Arc};

/// Key-sorted edits with distinct keys; `None` removes.
pub(crate) type Edits = [(Bytes, Option<Bytes>)];

#[derive(Default)]
pub(crate) struct Builder {
    leaves: Vec<Entry>,
    /// `branches[i]` holds the children of the open node at level `i + 1`.
    branches: Vec<Vec<Child>>,
}

impl Builder {
    pub fn push_entry(&mut self, entry: Entry) {
        let mark = entry.mark;
        self.leaves.push(entry);
        if ends(mark, 0, self.leaves.len()) {
            let leaf = Node::leaf(mem::take(&mut self.leaves));
            self.push_child(1, Child::of(leaf));
        }
    }

    fn push_child(&mut self, mut level: u8, mut child: Child) {
        loop {
            let index = usize::from(level) - 1;
            if self.branches.len() <= index {
                self.branches.resize_with(index + 1, Vec::new);
            }
            let open = &mut self.branches[index];
            let mark = child.mark;
            open.push(child);
            if !ends(mark, level, open.len()) {
                return;
            }
            child = Child::of(Node::branch(level, mem::take(open)));
            level += 1;
        }
    }

    /// Whether no node is open at `level` or below.
    fn clear_through(&self, level: u8) -> bool {
        self.leaves.is_empty()
            && self
                .branches
                .iter()
                .take(usize::from(level))
                .all(Vec::is_empty)
    }

    /// Takes `node` whole if chunking its entries one by one would rebuild
    /// it: nothing is open at its level or below, and its last item ends it,
    /// or nothing will follow it.
    fn reuse(&mut self, node: &Arc<Node>, last: bool) -> bool {
        let Some(mark) = node.last_mark() else {
            return false;
        };
        if !self.clear_through(node.level) || !(last || ends(mark, node.level, node.len())) {
            return false;
        }
        self.push_child(node.level + 1, Child::of(Arc::clone(node)));
        true
    }

    /// Closes the open nodes from the bottom up. The root is the first level
    /// that ends up as a single node.
    pub fn finish(mut self) -> Arc<Node> {
        let mut level = 0u8;
        loop {
            let above_open = self.branches[usize::from(level)..]
                .iter()
                .any(|open| !open.is_empty());
            if level == 0 {
                if !self.leaves.is_empty() {
                    let leaf = Node::leaf(mem::take(&mut self.leaves));
                    if !above_open {
                        return leaf;
                    }
                    self.push_child(1, Child::of(leaf));
                } else if !above_open {
                    return Node::leaf(Vec::new());
                }
            } else {
                let open = mem::take(&mut self.branches[usize::from(level) - 1]);
                if !above_open {
                    match <[Child; 1]>::try_from(open) {
                        Ok([only]) => return only.node,
                        Err(open) if !open.is_empty() => return Node::branch(level, open),
                        Err(_) => unreachable!("an open level lies above this one"),
                    }
                }
                if !open.is_empty() {
                    self.push_child(level + 1, Child::of(Node::branch(level, open)));
                }
            }
            level += 1;
        }
    }
}

/// The tree `root` becomes after `edits`.
pub(crate) fn rebuild(root: &Arc<Node>, edits: &Edits) -> Arc<Node> {
    let mut builder = Builder::default();
    walk(&mut builder, root, edits, true);
    builder.finish()
}

/// Feeds `node`'s entries, with `edits` applied, to the builder. `last` says
/// whether `node` is the last at its level, so that nothing follows it once
/// its edits are done.
fn walk(builder: &mut Builder, node: &Arc<Node>, edits: &Edits, last: bool) {
    if edits.is_empty() && builder.reuse(node, last) {
        return;
    }
    match &node.items {
        Items::Leaf(entries) => merge(builder, entries, edits),
        Items::Branch(children) => {
            let mut rest = edits;
            let final_index = children.len() - 1;
            for (index, child) in children.iter().enumerate() {
                // A child holds the keys after its predecessor's last key, up
                // to and including its own; the last child holds every key
                // after that.
                let split = if index == final_index {
                    rest.len()
                } else {
                    rest.partition_point(|(key, _)| key[..] <= child.key[..])
                };
                let (inside, after) = rest.split_at(split);
                walk(builder, &child.node, inside, last && index == final_index);
                rest = after;
            }
        }
    }
}

fn merge(builder: &mut Builder, entries: &[Entry], edits: &Edits) {
    let mut entries = entries.iter().peekable();
    for (key, value) in edits {
        while let Some(entry) = entries.next_if(|entry| entry.key[..] < key[..]) {
            builder.push_entry(entry.clone());
        }
        match (entries.next_if(|entry| entry.key == *key), value) {
            (Some(entry), Some(value)) => builder.push_entry(Entry {
                value: Arc::clone(value),
                ..entry.clone()
            }),
            (None, Some(value)) => {
                builder.push_entry(Entry::new(Arc::clone(key), Arc::clone(value)))
            }
            (_, None) => {}
        }
    }
    for entry in entries {
        builder.push_entry(entry.clone());
    }
}
