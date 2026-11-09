//! The ontologies a store can hold: any whose identifiers serialize, which is
//! how the store writes them.

use axionomy::{Identifier, Ontology};
use serde::{Serialize, de::DeserializeOwned};

/// What a store needs of an identifier type: what the kernel needs, a JSON
/// encoding, and being sendable to the writer thread.
pub trait Id: Identifier + Serialize + DeserializeOwned + Send + Sync + 'static {}

impl<T> Id for T where T: Identifier + Serialize + DeserializeOwned + Send + Sync + 'static {}

/// An ontology whose economies a store can record and read back.
pub trait Recordable: Ontology<Account: Id, Asset: Id, Rate: Id, Role: Id> {}

impl<O> Recordable for O where O: Ontology<Account: Id, Asset: Id, Rate: Id, Role: Id> {}
