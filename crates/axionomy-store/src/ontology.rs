//! The ontologies a store can hold: any whose identifiers are written as
//! paths (D-044), which is how the store keys them, and serialize, which is
//! how its records carry them.

use axionomy::{Identifier, Ontology, path::Segments};
use serde::{Serialize, de::DeserializeOwned};

/// What a store needs of an identifier type: what the kernel needs, its path
/// for keys, its serialization for records, and being sendable to the writer
/// thread.
pub trait Id: Identifier + Segments + Serialize + DeserializeOwned + Send + Sync + 'static {}

impl<T> Id for T where
    T: Identifier + Segments + Serialize + DeserializeOwned + Send + Sync + 'static
{
}

/// An ontology whose economies a store can record and read back.
pub trait Recordable: Ontology<Account: Id, Asset: Id, Rate: Id, Role: Id> {}

impl<O> Recordable for O where O: Ontology<Account: Id, Asset: Id, Rate: Id, Role: Id> {}
