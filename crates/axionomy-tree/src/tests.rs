use super::*;
use std::collections::{BTreeMap, HashMap};

type Model = BTreeMap<Vec<u8>, Vec<u8>>;

/// A small deterministic generator, so failures reproduce.
struct Random(u64);

impl Random {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, bound: u64) -> u64 {
        self.next() % bound
    }

    fn shuffle<T>(&mut self, items: &mut [T]) {
        for index in (1..items.len()).rev() {
            items.swap(index, self.below(index as u64 + 1) as usize);
        }
    }
}

fn key(n: u64) -> Vec<u8> {
    format!("accounts/{n:06}").into_bytes()
}

fn value(n: u64) -> Vec<u8> {
    n.to_be_bytes().to_vec()
}

fn built(model: &Model) -> Tree {
    model
        .iter()
        .map(|(key, value)| (key.as_slice(), value.as_slice()))
        .collect()
}

fn entries(tree: &Tree) -> Vec<(Vec<u8>, Vec<u8>)> {
    tree.iter()
        .map(|(key, value)| (key.to_vec(), value.to_vec()))
        .collect()
}

fn stored(tree: &Tree, store: &mut HashMap<Hash, Vec<u8>>) -> usize {
    let missing = tree.missing(|hash| store.contains_key(hash));
    let written = missing.len();
    store.extend(missing);
    written
}

fn reload(tree: &Tree, store: &HashMap<Hash, Vec<u8>>) -> Tree {
    Tree::load(&tree.hash(), |hash| {
        Ok::<_, std::convert::Infallible>(store.get(hash).cloned())
    })
    .unwrap()
}

/// The tree agrees with `model` entry for entry, is the tree a from-scratch
/// build gives, and survives storing and loading.
fn check(tree: &Tree, model: &Model) {
    assert_eq!(tree.len(), model.len() as u64);
    assert_eq!(
        entries(tree),
        model
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect::<Vec<_>>()
    );
    assert_eq!(tree.hash(), built(model).hash());
    let mut store = HashMap::new();
    stored(tree, &mut store);
    assert_eq!(reload(tree, &store).hash(), tree.hash());
}

#[test]
fn the_same_keys_give_the_same_tree_in_any_order() {
    let mut random = Random(0x5eed);
    let mut numbers: Vec<u64> = (0..5_000).collect();
    let model: Model = numbers.iter().map(|&n| (key(n), value(n))).collect();
    let expected = built(&model);
    assert!(
        expected.root.level >= 2,
        "the test needs a tree several levels deep"
    );
    for _ in 0..3 {
        random.shuffle(&mut numbers);
        let mut tree = Tree::new();
        for &n in &numbers {
            tree.insert(key(n), value(n));
            // Keys that come and go leave no trace.
            tree.insert(key(n + 1_000_000), value(n));
            tree.remove(&key(n + 1_000_000));
        }
        assert_eq!(tree.hash(), expected.hash());
    }
    let mut tree = expected.clone();
    for &n in &numbers {
        tree.insert(key(n), value(n + 1));
    }
    for &n in &numbers {
        tree.insert(key(n), value(n));
    }
    assert_eq!(tree.hash(), expected.hash());
}

#[test]
fn edits_give_the_tree_a_build_from_scratch_gives() {
    let mut random = Random(0xed17);
    let mut model = Model::new();
    let mut tree = Tree::new();
    for round in 0..300 {
        let size = if round % 50 == 0 {
            2_000
        } else {
            1 + random.below(40)
        };
        let edits: Vec<(Vec<u8>, Option<Vec<u8>>)> = (0..size)
            .map(|_| {
                let n = random.below(4_000);
                let value = (random.below(4) != 0).then(|| value(random.next()));
                (key(n), value)
            })
            .collect();
        for (key, value) in &edits {
            match value {
                Some(value) => model.insert(key.clone(), value.clone()),
                None => model.remove(key),
            };
        }
        tree.update(edits);
        if round % 10 == 0 {
            check(&tree, &model);
        } else {
            assert_eq!(tree.hash(), built(&model).hash(), "round {round}");
        }
    }
    let keys: Vec<Vec<u8>> = model.keys().cloned().collect();
    tree.update(keys.into_iter().map(|key| (key, None::<Vec<u8>>)));
    check(&tree, &Model::new());
    assert_eq!(tree.hash(), Tree::new().hash());
}

#[test]
fn cursors_agree_with_a_sorted_map() {
    let mut random = Random(0xc0de);
    let model: Model = (0..3_000)
        .map(|_| random.below(10_000))
        .map(|n| (key(n), value(n)))
        .collect();
    let tree = built(&model);
    let ordered: Vec<_> = model.iter().collect();
    for _ in 0..200 {
        let probe = key(random.below(10_500));
        let from = model.range(probe.clone()..).map(|(key, _)| key.as_slice());
        assert!(tree.iter_from(&probe).map(|(key, _)| key).eq(from));
        let rank = model.range(..probe.clone()).count() as u64;
        assert_eq!(tree.rank(&probe), rank);
        assert_eq!(tree.get(&probe), model.get(&probe).map(Vec::as_slice));

        let position = random.below(model.len() as u64 + 10);
        let at = ordered
            .iter()
            .skip(position as usize)
            .map(|(key, _)| key.as_slice());
        assert!(tree.iter_at(position).map(|(key, _)| key).eq(at));
    }
    for prefix in [
        "accounts/0001",
        "accounts/00",
        "accounts/1",
        "accounts/2",
        "loans",
        "",
    ] {
        let expected = model
            .keys()
            .filter(|key| key.starts_with(prefix.as_bytes()))
            .map(Vec::as_slice);
        assert!(
            tree.prefix(prefix.as_bytes())
                .map(|(key, _)| key)
                .eq(expected),
            "{prefix}"
        );
    }
}

#[test]
fn diff_lists_exactly_the_changed_keys() {
    let mut random = Random(0xd1ff);
    let before_model: Model = (0..4_000).map(|n| (key(n), value(n))).collect();
    let before = built(&before_model);
    for edits in [0, 1, 3, 50, 2_000] {
        let mut after_model = before_model.clone();
        let mut after = before.clone();
        for _ in 0..edits {
            let n = random.below(5_000);
            if random.below(3) == 0 {
                after_model.remove(&key(n));
                after.remove(&key(n));
            } else {
                after_model.insert(key(n), value(n + 7));
                after.insert(key(n), value(n + 7));
            }
        }
        let expected: Vec<_> = before_model
            .keys()
            .chain(after_model.keys())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .filter_map(|key| {
                let (before, after) = (before_model.get(key), after_model.get(key));
                (before != after).then(|| Change {
                    key,
                    before: before.map(Vec::as_slice),
                    after: after.map(Vec::as_slice),
                })
            })
            .collect();
        assert_eq!(before.diff(&after), expected);
        // Trees that share no nodes compare by hash.
        assert_eq!(built(&before_model).diff(&built(&after_model)), expected);
    }
    let empty = Tree::new();
    assert_eq!(empty.diff(&before).len(), before_model.len());
    assert!(
        before
            .diff(&empty)
            .iter()
            .all(|change| change.after.is_none())
    );
    assert!(empty.diff(&Tree::new()).is_empty());
}

#[test]
fn storing_writes_only_new_nodes_and_loading_checks_every_node() {
    let model: Model = (0..10_000).map(|n| (key(n), value(n))).collect();
    let mut tree = built(&model);
    let mut store = HashMap::new();
    let nodes = stored(&tree, &mut store);
    assert!(nodes > 100);
    let height = usize::from(tree.root.level) + 1;
    tree.insert(key(4_321), value(0));
    assert!(stored(&tree, &mut store) <= 3 * height);
    assert_eq!(reload(&tree, &store).get(&key(4_321)), Some(&value(0)[..]));

    let root = tree.hash();
    let mut tampered = store.clone();
    tampered.get_mut(&root).unwrap()[3] ^= 1;
    let fetch = |store: &HashMap<Hash, Vec<u8>>| {
        let store = store.clone();
        move |hash: &Hash| Ok::<_, std::convert::Infallible>(store.get(hash).cloned())
    };
    assert!(matches!(
        Tree::load(&root, fetch(&tampered)),
        Err(LoadError::Malformed { .. })
    ));
    let Items::Branch(children) = &tree.root.items else {
        panic!("a tree of ten thousand entries has branches");
    };
    let mut pruned = store.clone();
    pruned.remove(&children[0].node.hash());
    assert!(matches!(
        Tree::load(&root, fetch(&pruned)),
        Err(LoadError::Missing(_))
    ));

    // A node that holds what canonical chunking splits is rejected, though
    // its hash matches its bytes.
    let leaves: Vec<Entry> = tree
        .iter()
        .take(200)
        .map(|(key, value)| Entry::new(key.into(), value.into()))
        .collect();
    let merged = Node::leaf(leaves);
    let mut forged = HashMap::new();
    forged.insert(merged.hash(), merged.encode());
    assert!(matches!(
        Tree::load(&merged.hash(), fetch(&forged)),
        Err(LoadError::Malformed { .. })
    ));
    // So is a root over a single child.
    let only = Node::branch(
        1,
        vec![Child::of(Node::leaf(vec![Entry::new(
            Bytes::from(&b"a"[..]),
            Bytes::from(&b"1"[..]),
        )]))],
    );
    let mut forged = HashMap::new();
    stored(
        &Tree {
            root: Arc::clone(&only),
        },
        &mut forged,
    );
    assert!(Tree::load(&only.hash(), fetch(&forged)).is_err());
}

#[test]
fn keys_that_never_end_a_node_still_bound_its_size() {
    let mut keys: Vec<Vec<u8>> = (0..)
        .map(key)
        .filter(|key| !ends(node::mark(key), 0, 1))
        .take(700)
        .collect();
    keys.sort();
    let mut model: Model = keys.iter().map(|key| (key.clone(), vec![1])).collect();
    let mut tree = built(&model);
    let leaves = |tree: &Tree| -> Vec<usize> {
        let mut sizes = Vec::new();
        let mut stack = vec![&tree.root];
        while let Some(node) = stack.pop() {
            match &node.items {
                Items::Leaf(entries) => sizes.push(entries.len()),
                Items::Branch(children) => {
                    stack.extend(children.iter().rev().map(|child| &child.node));
                }
            }
        }
        sizes
    };
    assert_eq!(leaves(&tree), [256, 256, 188]);
    // Removing an early key moves every forced boundary after it.
    model.remove(&keys[3]);
    tree.remove(&keys[3]);
    assert_eq!(leaves(&tree), [256, 256, 187]);
    check(&tree, &model);
}
