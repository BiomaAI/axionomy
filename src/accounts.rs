//! The accounts of an economy, shared between forks.

use crate::Account;
use std::{
    fmt,
    hash::{Hash, Hasher},
    sync::Arc,
};

/// Accounts in the order they were created.
///
/// Every part is persistent and shared between forks: identifiers and
/// holdings live in 32-way trees indexed by creation slot, and a persistent
/// hash trie maps identifiers to slots. Cloning an economy copies a few
/// pointers; replacing, opening or closing an account copies one root-to-leaf
/// path, in place when nothing else shares it. A closed account leaves an empty
/// slot until more slots are empty than full, when the accounts are compacted.
#[derive(Clone)]
pub(crate) struct Accounts<AccountId, A> {
    index: Index<AccountId>,
    ids: Slots<Option<AccountId>>,
    holdings: Slots<Option<Arc<Account<A>>>>,
    live: usize,
}

impl<AccountId, A> Accounts<AccountId, A>
where
    AccountId: Eq + Hash,
{
    pub(crate) fn get(&self, id: &AccountId) -> Option<&Arc<Account<A>>> {
        self.holdings.get(self.index.get(id)?)?.as_ref()
    }

    pub(crate) fn contains(&self, id: &AccountId) -> bool {
        self.index.get(id).is_some()
    }

    /// Replaces an existing account's holdings.
    ///
    /// # Panics
    ///
    /// If the economy has no such account; the kernel replaces only accounts it
    /// has validated.
    pub(crate) fn replace(&mut self, id: &AccountId, holder: Arc<Account<A>>) {
        let slot = self
            .index
            .get(id)
            .expect("only existing accounts are replaced");
        self.holdings.set(slot, Some(holder));
    }
}

impl<AccountId, A> Accounts<AccountId, A>
where
    AccountId: Clone + Eq + Hash,
{
    /// Accounts with distinct identifiers, in order.
    ///
    /// # Panics
    ///
    /// If an identifier repeats; the builder rejects that first.
    pub(crate) fn new(accounts: impl IntoIterator<Item = (AccountId, Arc<Account<A>>)>) -> Self {
        let (ids, holdings): (Vec<_>, Vec<_>) = accounts
            .into_iter()
            .map(|(id, holder)| (Some(id), Some(holder)))
            .unzip();
        let mut index = Index::default();
        for (slot, id) in ids.iter().enumerate() {
            let id = id.as_ref().expect("every new slot holds an account");
            assert!(
                index.insert(id.clone(), slot).is_none(),
                "account identifiers must be distinct"
            );
        }
        Self {
            index,
            live: ids.len(),
            ids: Slots::new(ids),
            holdings: Slots::new(holdings),
        }
    }

    /// Adds an account after every existing one.
    ///
    /// # Panics
    ///
    /// If the account exists; the kernel opens only accounts it has checked
    /// are absent.
    pub(crate) fn open(&mut self, id: AccountId, holder: Arc<Account<A>>) {
        let slot = self.ids.len();
        assert!(
            self.index.insert(id.clone(), slot).is_none(),
            "only absent accounts are opened"
        );
        self.ids.push(Some(id));
        self.holdings.push(Some(holder));
        self.live += 1;
    }

    /// Removes an account.
    ///
    /// # Panics
    ///
    /// If the economy has no such account; the kernel closes only accounts it
    /// has validated.
    pub(crate) fn close(&mut self, id: &AccountId) {
        let slot = self
            .index
            .remove(id)
            .expect("only existing accounts are closed");
        self.ids.set(slot, None);
        self.holdings.set(slot, None);
        self.live -= 1;
        let empty = self.ids.len() - self.live;
        if empty >= WIDTH && empty > self.live {
            *self = Self::new(
                self.iter()
                    .map(|(id, holder)| (id.clone(), Arc::clone(holder)))
                    .collect::<Vec<_>>(),
            );
        }
    }
}

impl<AccountId, A> Accounts<AccountId, A> {
    pub(crate) fn iter(&self) -> impl Iterator<Item = (&AccountId, &Arc<Account<A>>)> {
        self.ids
            .iter()
            .zip(self.holdings.iter())
            .filter_map(|(id, holder)| Some((id.as_ref()?, holder.as_ref()?)))
    }

    pub(crate) fn values(&self) -> impl Iterator<Item = &Arc<Account<A>>> {
        self.holdings.iter().flatten()
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

/// A persistent vector: a complete, left-packed tree of 32-way nodes. Unshared
/// nodes are updated in place; shared ones are copied first.
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

impl<T> Slots<T> {
    const fn len(&self) -> usize {
        self.len
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

    fn push(&mut self, value: T) {
        let index = self.len;
        if index == 1 << (self.shift + BITS) {
            let full = Arc::clone(&self.root);
            self.root = Arc::new(Node::Branch(vec![full]));
            self.shift += BITS;
        }
        let mut node = Arc::make_mut(&mut self.root);
        let mut shift = self.shift;
        loop {
            node = match node {
                Node::Branch(children) => {
                    let position = (index >> shift) & MASK;
                    if position == children.len() {
                        children.push(Arc::new(if shift == BITS {
                            Node::Leaf(Vec::with_capacity(WIDTH))
                        } else {
                            Node::Branch(Vec::with_capacity(WIDTH))
                        }));
                    }
                    shift -= BITS;
                    Arc::make_mut(&mut children[position])
                }
                Node::Leaf(items) => {
                    items.push(value);
                    self.len += 1;
                    return;
                }
            };
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

/// A persistent map from keys to slots: a hash array mapped trie of 32-way
/// nodes, path-copied on change like [`Slots`].
#[derive(Clone)]
struct Index<K> {
    root: Arc<Trie<K>>,
}

impl<K> Default for Index<K> {
    fn default() -> Self {
        Self {
            root: Arc::new(Trie::default()),
        }
    }
}

#[derive(Clone)]
struct Trie<K> {
    /// Bit `i` is set when an entry occupies hash chunk `i` at this level.
    bitmap: u32,
    entries: Vec<Entry<K>>,
}

impl<K> Default for Trie<K> {
    fn default() -> Self {
        Self {
            bitmap: 0,
            entries: Vec::new(),
        }
    }
}

#[derive(Clone)]
enum Entry<K> {
    Key {
        hash: u64,
        key: K,
        slot: usize,
    },
    Trie(Arc<Trie<K>>),
    /// Keys whose whole hashes are equal.
    Collision {
        hash: u64,
        keys: Vec<(K, usize)>,
    },
}

/// FNV-1a: a fixed, fast hash, so a trie's shape depends only on its keys.
struct Fnv(u64);

impl Hasher for Fnv {
    fn write(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 = (self.0 ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3);
        }
    }

    fn finish(&self) -> u64 {
        self.0
    }
}

fn hash_of<K: Hash>(key: &K) -> u64 {
    let mut hasher = Fnv(0xcbf2_9ce4_8422_2325);
    key.hash(&mut hasher);
    hasher.finish()
}

/// The bit and entry position of `hash` at `shift` in `trie`.
fn position<K>(trie: &Trie<K>, hash: u64, shift: u32) -> (u32, usize) {
    let bit = 1 << ((hash >> shift) & 31);
    (bit, (trie.bitmap & (bit - 1)).count_ones() as usize)
}

impl<K: Eq + Hash> Index<K> {
    fn get(&self, key: &K) -> Option<usize> {
        let hash = hash_of(key);
        let mut trie = &*self.root;
        let mut shift = 0;
        loop {
            let (bit, at) = position(trie, hash, shift);
            if trie.bitmap & bit == 0 {
                return None;
            }
            match &trie.entries[at] {
                Entry::Key {
                    hash: stored,
                    key: other,
                    slot,
                } => return (*stored == hash && other == key).then_some(*slot),
                Entry::Trie(child) => {
                    trie = child;
                    shift += BITS as u32;
                }
                Entry::Collision { hash: stored, keys } => {
                    return (*stored == hash)
                        .then(|| {
                            keys.iter()
                                .find(|(other, _)| other == key)
                                .map(|(_, slot)| *slot)
                        })
                        .flatten();
                }
            }
        }
    }
}

impl<K: Clone + Eq + Hash> Index<K> {
    /// Maps `key` to `slot`, returning the slot it replaced.
    fn insert(&mut self, key: K, slot: usize) -> Option<usize> {
        let hash = hash_of(&key);
        Self::insert_into(Arc::make_mut(&mut self.root), hash, key, slot, 0)
    }

    fn insert_into(
        trie: &mut Trie<K>,
        hash: u64,
        key: K,
        slot: usize,
        shift: u32,
    ) -> Option<usize> {
        let (bit, at) = position(trie, hash, shift);
        if trie.bitmap & bit == 0 {
            trie.bitmap |= bit;
            trie.entries.insert(at, Entry::Key { hash, key, slot });
            return None;
        }
        match &mut trie.entries[at] {
            Entry::Trie(child) => {
                Self::insert_into(Arc::make_mut(child), hash, key, slot, shift + BITS as u32)
            }
            Entry::Key {
                hash: stored,
                key: other,
                slot: previous,
            } if *stored == hash && *other == key => Some(std::mem::replace(previous, slot)),
            Entry::Collision { hash: stored, keys } if *stored == hash => {
                match keys.iter_mut().find(|(other, _)| *other == key) {
                    Some((_, previous)) => Some(std::mem::replace(previous, slot)),
                    None => {
                        keys.push((key, slot));
                        None
                    }
                }
            }
            existing => {
                let displaced = std::mem::replace(existing, Entry::Trie(Arc::default()));
                let Entry::Trie(child) = existing else {
                    unreachable!("the entry was just replaced by a trie")
                };
                let child = Arc::make_mut(child);
                let new = Entry::Key { hash, key, slot };
                Self::place(child, displaced, shift + BITS as u32);
                Self::place(child, new, shift + BITS as u32);
                None
            }
        }
    }

    /// Places an entry whose hash differs from every other entry in `trie`'s
    /// subtree, or equals one key's, splitting into deeper tries as needed.
    fn place(trie: &mut Trie<K>, entry: Entry<K>, shift: u32) {
        let hash = match &entry {
            Entry::Key { hash, .. } | Entry::Collision { hash, .. } => *hash,
            Entry::Trie(_) => unreachable!("only keys and collisions are placed"),
        };
        let (bit, at) = position(trie, hash, shift);
        if trie.bitmap & bit == 0 {
            trie.bitmap |= bit;
            trie.entries.insert(at, entry);
            return;
        }
        let existing = &mut trie.entries[at];
        let existing_hash = match existing {
            Entry::Key { hash, .. } | Entry::Collision { hash, .. } => Some(*hash),
            Entry::Trie(_) => None,
        };
        match (existing_hash, entry) {
            (Some(stored), Entry::Key { key, slot, .. }) if stored == hash => {
                let collided = std::mem::replace(existing, Entry::Trie(Arc::default()));
                *existing = match collided {
                    Entry::Key {
                        key: other,
                        slot: other_slot,
                        ..
                    } => Entry::Collision {
                        hash,
                        keys: vec![(other, other_slot), (key, slot)],
                    },
                    Entry::Collision { mut keys, .. } => {
                        keys.push((key, slot));
                        Entry::Collision { hash, keys }
                    }
                    Entry::Trie(_) => unreachable!("a trie has no hash"),
                };
            }
            (_, entry) => {
                if existing_hash.is_some() {
                    let displaced = std::mem::replace(existing, Entry::Trie(Arc::default()));
                    let Entry::Trie(child) = existing else {
                        unreachable!("the entry was just replaced by a trie")
                    };
                    Self::place(Arc::make_mut(child), displaced, shift + BITS as u32);
                }
                let Entry::Trie(child) = existing else {
                    unreachable!("the entry is a trie")
                };
                Self::place(Arc::make_mut(child), entry, shift + BITS as u32);
            }
        }
    }

    /// Removes `key`, returning its slot.
    fn remove(&mut self, key: &K) -> Option<usize> {
        let hash = hash_of(key);
        Self::remove_from(Arc::make_mut(&mut self.root), hash, key, 0)
    }

    fn remove_from(trie: &mut Trie<K>, hash: u64, key: &K, shift: u32) -> Option<usize> {
        let (bit, at) = position(trie, hash, shift);
        if trie.bitmap & bit == 0 {
            return None;
        }
        match &mut trie.entries[at] {
            Entry::Key {
                hash: stored,
                key: other,
                slot,
            } if *stored == hash && other == key => {
                let slot = *slot;
                trie.bitmap &= !bit;
                trie.entries.remove(at);
                Some(slot)
            }
            Entry::Key { .. } => None,
            Entry::Trie(child) => {
                let removed =
                    Self::remove_from(Arc::make_mut(child), hash, key, shift + BITS as u32);
                if child.entries.is_empty() {
                    trie.bitmap &= !bit;
                    trie.entries.remove(at);
                }
                removed
            }
            Entry::Collision { hash: stored, keys } => {
                if *stored != hash {
                    return None;
                }
                let found = keys.iter().position(|(other, _)| other == key)?;
                let (_, slot) = keys.remove(found);
                if let [(only, only_slot)] = keys.as_slice() {
                    trie.entries[at] = Entry::Key {
                        hash,
                        key: only.clone(),
                        slot: *only_slot,
                    };
                }
                Some(slot)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn slots_read_write_grow_and_iterate_like_a_vector_at_every_depth() {
        for len in [0, 1, 31, 32, 33, 1_024, 1_025, 40_000] {
            let mut slots = Slots::new((0..len).collect());
            let mut expected: Vec<usize> = (0..len).collect();
            let fork = slots.clone();
            for index in (0..len).step_by(7) {
                slots.set(index, index * 10);
                expected[index] = index * 10;
            }
            for extra in 0..70 {
                slots.push(extra);
                expected.push(extra);
            }
            assert_eq!(slots.len(), expected.len());
            assert_eq!(
                slots.iter().copied().collect::<Vec<_>>(),
                expected,
                "len {len}"
            );
            assert!((0..expected.len()).all(|index| slots.get(index) == Some(&expected[index])));
            assert_eq!(slots.get(expected.len()), None);
            assert!(
                fork.iter().copied().eq(0..len),
                "a fork is unchanged by writes to its source (len {len})"
            );
        }
    }

    /// Every key hashes alike, so every operation goes through collisions.
    #[derive(Clone, PartialEq, Eq)]
    struct Colliding(u32);

    impl Hash for Colliding {
        fn hash<H: Hasher>(&self, state: &mut H) {
            state.write_u8(0);
        }
    }

    fn agree<K: Clone + Eq + Hash + std::fmt::Debug>(keys: impl Fn(u32) -> K) {
        let mut index = Index::default();
        let mut expected = HashMap::new();
        let mut state = 0x2545_f491_u32;
        let mut snapshots = Vec::new();
        for step in 0..4_000_u32 {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            let key = keys(state % 600);
            if state.is_multiple_of(3) {
                assert_eq!(index.remove(&key), expected.remove(&key), "step {step}");
            } else {
                let slot = step as usize;
                assert_eq!(
                    index.insert(key.clone(), slot),
                    expected.insert(key, slot),
                    "step {step}"
                );
            }
            if step % 500 == 0 {
                snapshots.push((index.clone(), expected.clone()));
            }
        }
        for candidate in 0..600 {
            let key = keys(candidate);
            assert_eq!(index.get(&key), expected.get(&key).copied());
        }
        for (snapshot, expected) in snapshots {
            for candidate in 0..600 {
                let key = keys(candidate);
                assert_eq!(
                    snapshot.get(&key),
                    expected.get(&key).copied(),
                    "a fork is unchanged"
                );
            }
        }
    }

    #[test]
    fn index_agrees_with_a_hash_map_and_keeps_forks_intact() {
        agree(|key| key);
        agree(|key| format!("account/{key}"));
    }

    impl std::fmt::Debug for Colliding {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(formatter, "Colliding({})", self.0)
        }
    }

    #[test]
    fn index_handles_keys_whose_hashes_collide() {
        agree(Colliding);
    }

    #[test]
    fn accounts_open_close_and_reopen_in_creation_order() {
        let holder = |amount| {
            Arc::new(Account::<u8>::new(
                [(0, crate::Quantity::new(amount))].into(),
            ))
        };
        let mut accounts = Accounts::new([(1_u32, holder(1)), (2, holder(2))]);
        let fork = accounts.clone();
        accounts.open(3, holder(3));
        accounts.close(&1);
        assert!(!accounts.contains(&1));
        accounts.open(1, holder(4));
        let order: Vec<_> = accounts.iter().map(|(id, _)| *id).collect();
        assert_eq!(order, [2, 3, 1]);
        assert_eq!(
            accounts.get(&1).unwrap().balance(&0),
            crate::Quantity::new(4)
        );
        assert_eq!(fork.iter().map(|(id, _)| *id).collect::<Vec<_>>(), [1, 2]);
        for id in 4..200 {
            accounts.open(id, holder(u64::from(id)));
        }
        for id in 4..190 {
            accounts.close(&id);
        }
        let order: Vec<_> = accounts.iter().map(|(id, _)| *id).collect();
        assert_eq!(
            order,
            [2, 3, 1, 190, 191, 192, 193, 194, 195, 196, 197, 198, 199]
        );
        assert!(accounts.ids.len() < 64, "closed slots are compacted away");
    }
}
