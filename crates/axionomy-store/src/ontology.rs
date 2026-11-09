//! The ontologies a store can hold: any whose identifiers serialize, which is
//! how the store writes them.

use crate::{OntologyPath, StoreError};
use axionomy::Ontology;
use serde::{Serialize, de::DeserializeOwned};
use std::{fmt::Debug, hash::Hash};

/// What a store needs of an identifier type: the kernel's bounds, a JSON
/// encoding, and being sendable to the writer thread.
pub trait Id:
    Clone + Eq + Hash + Ord + Debug + Serialize + DeserializeOwned + Send + Sync + 'static
{
}

impl<T> Id for T where
    T: Clone + Eq + Hash + Ord + Debug + Serialize + DeserializeOwned + Send + Sync + 'static
{
}

/// An ontology whose economies a store can record and read back.
pub trait Recordable:
    Ontology<Account: Id, Asset: Id, Rate: Id, Role: Id> + Send + Sync + 'static
{
}

impl<O> Recordable for O where
    O: Ontology<Account: Id, Asset: Id, Rate: Id, Role: Id> + Send + Sync + 'static
{
}

/// An economy of an ontology.
pub type Economy<O> = axionomy::Economy<
    <O as Ontology>::Account,
    <O as Ontology>::Asset,
    <O as Ontology>::Rate,
    <O as Ontology>::Role,
>;

/// An exchange of an ontology.
pub type Exchange<O> =
    axionomy::Exchange<<O as Ontology>::Rate, <O as Ontology>::Role, <O as Ontology>::Account>;

/// The receipt of an exchange of an ontology.
pub type Receipt<O> = axionomy::Receipt<
    <O as Ontology>::Rate,
    <O as Ontology>::Role,
    <O as Ontology>::Account,
    <O as Ontology>::Asset,
>;

/// Why an exchange of an ontology does not apply.
pub type ApplyError<O> = axionomy::ApplyError<
    <O as Ontology>::Rate,
    <O as Ontology>::Role,
    <O as Ontology>::Account,
    <O as Ontology>::Asset,
>;

/// `O`'s name, as runs record it.
pub(crate) fn name<O: Ontology>() -> Result<OntologyPath, StoreError> {
    O::NAME
        .parse()
        .map_err(|_| StoreError::OntologyName(O::NAME))
}
