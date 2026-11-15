# axionomy-derive

`#[derive(Identifier)]`, re-exported as `axionomy::Identifier`: an identifier
type written as one path, its only written form, and ordered as that path.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
enum Room {
    KeyRoom,
    Gate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
enum Asset {
    At(Room),
    Edge(Room, Room),
    Key(Option<u8>),
}

assert_eq!(serde_json::json!(Asset::Edge(Room::KeyRoom, Room::Gate)), "edge/key-room/gate");
assert_eq!(serde_json::json!(Asset::Key(None)), "key/none");
assert_eq!(serde_json::json!(Asset::Key(Some(7))), "key/some/7");
```

- **Written.** A variant is its kebab-case name, then its fields in order; a
  struct is its fields in order (`JobId(3)` is `3`), and a unit struct its
  name. A field is another identifier, an unsigned integer, a `bool` (`true`,
  `false`) or an `Option` of one (`none`, or `some` and the value). Anything
  else, such as a signed integer, text, a float or a collection, has no
  written form and does not compile.
- **Read back.** `Serialize` and `Deserialize` write and read that path, so
  JSON, the store, Studio keys and MCP spell an identifier one way, and a
  typed economy written down is an `axionomy::paths` economy.
- **Ordered as its path.** `Ord` is the path's natural order, the order of
  `axionomy::path::Path`: segment by segment, numbers by value and before
  words (`job/2` before `job/10`), and a path before the longer paths it
  starts. Variants sort by name, not by where they are declared. Code that
  needs a domain order, such as time, states it instead of borrowing an
  identifier's.
- **Patterns.** Beside `Asset` the derive writes `AssetPattern`, the same
  variants with each field an `axionomy::Slot` of the field's type: a value
  (`Slot::Is`), a variable (`Slot::Var`, from an `axionomy::Param`), or a
  pattern of the field's own type (`Slot::Of`). It converts into
  `axionomy::Pattern<Asset>`, written as `Asset` writes a value, so a rate
  with variables names its identifier, assets and accounts with types, not
  text. A variable stands for one segment, so `Param<T>` takes a `T` written
  in one: an unsigned integer, a `bool`, or an enum whose variants have no
  fields, for which the derive also lists every value (`Param::every`).

```rust
use axionomy::{Param, Pattern, path};

// `Room` and `Asset` as above.
let from = Param::<Room>::new(path!("from"));
let at: Pattern<Asset> = AssetPattern::At(from.var()).into();
assert_eq!(at, "at/{from}".parse().unwrap());
assert_eq!(from.every(), axionomy::Variable::values(
    path!("from"),
    ["key-room", "gate"].map(|segment| segment.parse().unwrap()),
));
```

Derive `Debug`, `Clone`, `PartialEq`, `Eq` and `Hash` (and `Copy` when it
fits) as usual, but not `PartialOrd`, `Ord`, `Serialize` or `Deserialize`: the
derive writes those. `axionomy::path::write` and `axionomy::path::read` turn
an identifier into a typed path and back.
