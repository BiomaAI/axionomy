# axionomy-tree

`axionomy-tree` is the persistent ordered map Axionomy stores states in: a
prolly tree from byte keys to byte values whose shape depends only on its
keys.

- **Canonical.** A key's hash decides whether it ends a node, at each level,
  so the same keys give the same tree and the same root hash in any insertion
  order, and changing a value never moves a boundary. A node also ends at 256
  items, so chosen keys cannot make one larger.
- **Persistent.** Nodes are immutable and shared. Cloning a tree copies a
  pointer; an edit copies the nodes from the root to the entries it changes.
- **Hashes on request.** A node's hash is the keyed BLAKE3 hash of its
  encoding, computed the first time it is asked for and remembered, so a node
  shared by many versions is hashed once.
- **Counted.** Branches count the entries under each child, so positions,
  ranks and paging are logarithmic.

```rust
use axionomy_tree::Tree;

let mut before = Tree::new();
before.insert(&b"accounts/alice"[..], &b"10"[..]);
before.insert(&b"accounts/bob"[..], &b"5"[..]);

let mut after = before.clone();
after.insert(&b"accounts/bob"[..], &b"7"[..]);

let changes = after.diff(&before);
assert_eq!(changes.len(), 1);
assert_eq!(changes[0].key, b"accounts/bob");

let rebuilt: Tree = after.iter().collect();
assert_eq!(rebuilt.hash(), after.hash());
```

`Tree::missing` lists the nodes a store does not have yet, and `Tree::load`
reads a tree back by its root hash, checking every node against its hash and
the whole tree against the shape its keys determine. The crate does no I/O and
knows nothing about economies; `axionomy-store` builds on it.
