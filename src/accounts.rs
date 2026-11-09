//! The accounts of an economy, shared between forks.

use crate::Account;
use indexmap::IndexSet;
use std::{fmt, hash::Hash, sync::Arc};

/// Accounts in the order they were declared.
///
/// An economy's set of accounts is fixed when it is built; exchanges only
/// replace holdings. Identifiers are therefore shared by every fork, and
/// holdings live in a persistent 32-way tree, so cloning an economy copies two
/// pointers and replacing an account copies one root-to-leaf path.
#[derive(Clone)]
pub(crate) struct Accounts<AccountId, A> {
    ids: Arc<IndexSet<AccountId>>,
    holdings: Slots<Arc<Account<A>>>,
}

impl<AccountId, A> Accounts<AccountId, A>
where
    AccountId: Eq + Hash,
{
    /// Accounts with distinct identifiers, in order.
    pub(crate) fn new(accounts: impl IntoIterator<Item = (AccountId, Arc<Account<A>>)>) -> Self {
        let (ids, holdings): (IndexSet<_>, Vec<_>) = accounts.into_iter().unzip();
        assert_eq!(
            ids.len(),
            holdings.len(),
            "account identifiers must be distinct"
        );
        Self {
            ids: Arc::new(ids),
            holdings: Slots::new(holdings),
        }
    }

    pub(crate) fn get(&self, id: &AccountId) -> Option<&Arc<Account<A>>> {
        self.holdings.get(self.ids.get_index_of(id)?)
    }

    pub(crate) fn contains(&self, id: &AccountId) -> bool {
        self.ids.contains(id)
    }

    /// Replaces an existing account's holdings.
    ///
    /// # Panics
    ///
    /// If the economy has no such account; the kernel replaces only accounts it
    /// has validated.
    pub(crate) fn replace(&mut self, id: &AccountId, holder: Arc<Account<A>>) {
        let index = self
            .ids
            .get_index_of(id)
            .expect("only existing accounts are replaced");
        self.holdings.set(index, holder);
    }
}

impl<AccountId, A> Accounts<AccountId, A> {
    pub(crate) fn iter(&self) -> impl Iterator<Item = (&AccountId, &Arc<Account<A>>)> {
        self.ids.iter().zip(self.holdings.iter())
    }

    pub(crate) fn values(&self) -> impl Iterator<Item = &Arc<Account<A>>> {
        self.holdings.iter()
    }
}

impl<AccountId: fmt::Debug, A: fmt::Debug> fmt::Debug for Accounts<AccountId, A> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_map().entries(self.iter()).finish()
    }
}

const BITS: usize = 5;
const WIDTH: usize = 1 << BITS;
const MASK: usize = WIDTH - 1;

/// A fixed-length persistent vector: a complete, left-packed tree of 32-way
/// nodes. Unshared nodes are updated in place; shared ones are copied first.
#[derive(Clone)]
struct Slots<T> {
    len: usize,
    /// Bit shift of the root's child index; zero when the root is a leaf.
    shift: usize,
    root: Arc<Node<T>>,
}

#[derive(Clone)]
enum Node<T> {
    Leaf(Vec<T>),
    Branch(Vec<Arc<Node<T>>>),
}

impl<T: Clone> Slots<T> {
    fn new(items: Vec<T>) -> Self {
        let len = items.len();
        let mut level: Vec<Arc<Node<T>>> = items
            .chunks(WIDTH)
            .map(|chunk| Arc::new(Node::Leaf(chunk.to_vec())))
            .collect();
        let mut shift = 0;
        while level.len() > 1 {
            level = level
                .chunks(WIDTH)
                .map(|chunk| Arc::new(Node::Branch(chunk.to_vec())))
                .collect();
            shift += BITS;
        }
        let root = level
            .pop()
            .unwrap_or_else(|| Arc::new(Node::Leaf(Vec::new())));
        Self { len, shift, root }
    }

    fn get(&self, index: usize) -> Option<&T> {
        if index >= self.len {
            return None;
        }
        let mut node = &*self.root;
        let mut shift = self.shift;
        loop {
            match node {
                Node::Branch(children) => {
                    node = &children[(index >> shift) & MASK];
                    shift -= BITS;
                }
                Node::Leaf(items) => return items.get(index & MASK),
            }
        }
    }

    fn set(&mut self, index: usize, value: T) {
        assert!(index < self.len, "slot {index} is out of bounds");
        let mut node = Arc::make_mut(&mut self.root);
        let mut shift = self.shift;
        loop {
            node = match node {
                Node::Branch(children) => {
                    let child = &mut children[(index >> shift) & MASK];
                    shift -= BITS;
                    Arc::make_mut(child)
                }
                Node::Leaf(items) => {
                    items[index & MASK] = value;
                    return;
                }
            };
        }
    }

    fn iter(&self) -> SlotIter<'_, T> {
        match &*self.root {
            Node::Leaf(items) => SlotIter {
                branches: Vec::new(),
                leaf: items.iter(),
            },
            Node::Branch(children) => SlotIter {
                branches: vec![children.iter()],
                leaf: [].iter(),
            },
        }
    }
}

struct SlotIter<'a, T> {
    branches: Vec<std::slice::Iter<'a, Arc<Node<T>>>>,
    leaf: std::slice::Iter<'a, T>,
}

impl<'a, T> Iterator for SlotIter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<&'a T> {
        loop {
            if let Some(item) = self.leaf.next() {
                return Some(item);
            }
            let child = loop {
                let branch = self.branches.last_mut()?;
                match branch.next() {
                    Some(child) => break child,
                    None => {
                        self.branches.pop();
                    }
                }
            };
            match &**child {
                Node::Leaf(items) => self.leaf = items.iter(),
                Node::Branch(children) => self.branches.push(children.iter()),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slots_read_write_and_iterate_like_a_vector_at_every_depth() {
        for len in [0, 1, 31, 32, 33, 1_024, 1_025, 40_000] {
            let mut slots = Slots::new((0..len).collect());
            let mut expected: Vec<usize> = (0..len).collect();
            let fork = slots.clone();
            for index in (0..len).step_by(7) {
                slots.set(index, index * 10);
                expected[index] = index * 10;
            }
            assert_eq!(
                slots.iter().copied().collect::<Vec<_>>(),
                expected,
                "len {len}"
            );
            assert!((0..len).all(|index| slots.get(index) == Some(&expected[index])));
            assert_eq!(slots.get(len), None);
            assert!(
                fork.iter().copied().eq(0..len),
                "a fork is unchanged by writes to its source (len {len})"
            );
        }
    }
}
