//! Source declarations shared by Rust ontology constructors and the package
//! compiler. Both lower directly into the same checked kernel builder.
use axionomy::{
    Account, Economy, EconomyBuilder, LinearInvariant, Ontology, OntologyPath, PathKind, Pattern,
    Rate, Relation, Segment, Variable, VariablePath, path::Path,
};
use serde::Deserialize;
use std::collections::BTreeMap;

pub mod model;
pub mod program;

pub mod script;
mod vocabulary;

pub enum Laws {}
impl PathKind for Laws {
    const SCHEMA: &'static str = "LawSetPath";
}
pub type LawSetPath = Path<Laws>;
pub type LawSet<O> = Vec<(Pattern<<O as Ontology>::Rate>, Rate<O>)>;

/// A written rate stays a template in the economy. An expanded declaration
/// materializes its finite instances as individual, concrete rates in order.
pub enum Law<O: Ontology> {
    Written((Pattern<O::Rate>, Rate<O>)),
    Family {
        expand: Pattern<O::Rate>,
        rate: Rate<O>,
    },
}
impl<'de, O: Ontology> Deserialize<'de> for Law<O>
where
    O::Account: Deserialize<'de>,
    O::Asset: Deserialize<'de>,
    O::Rate: Deserialize<'de>,
    O::Role: Deserialize<'de>,
{
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        // Select by the JSON shape so nested errors retain their original
        // field and location, including inside a written kernel rate.
        struct Entry<O>(std::marker::PhantomData<O>);
        impl<'de, O: Ontology> serde::de::Visitor<'de> for Entry<O>
        where
            O::Account: Deserialize<'de>,
            O::Asset: Deserialize<'de>,
            O::Rate: Deserialize<'de>,
            O::Role: Deserialize<'de>,
        {
            type Value = Law<O>;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("a [rate identifier, law] pair or an expanded rate family")
            }
            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                seq: A,
            ) -> Result<Self::Value, A::Error> {
                Deserialize::deserialize(serde::de::value::SeqAccessDeserializer::new(seq))
                    .map(Law::Written)
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                map: A,
            ) -> Result<Self::Value, A::Error> {
                #[derive(Deserialize)]
                #[serde(
                    deny_unknown_fields,
                    bound(
                        deserialize = "O::Account:Deserialize<'de>, O::Asset:Deserialize<'de>, O::Rate:Deserialize<'de>, O::Role:Deserialize<'de>"
                    )
                )]
                struct Family<O: Ontology> {
                    expand: Pattern<O::Rate>,
                    rate: Rate<O>,
                }
                let Family { expand, rate } =
                    Family::deserialize(serde::de::value::MapAccessDeserializer::new(map))?;
                Ok(Law::Family { expand, rate })
            }
        }
        d.deserialize_any(Entry(std::marker::PhantomData))
    }
}
pub fn expand<O: Ontology>(declarations: Vec<Law<O>>) -> Result<LawSet<O>, String> {
    let mut output = Vec::new();
    for d in declarations {
        match d {
            Law::Written(entry) => output.push(entry),
            Law::Family { expand, rate } => {
                let prototype = EconomyBuilder::new()
                    .rate_at(expand.clone(), rate)
                    .build()
                    .map_err(|e| format!("{:?}", e.issues()))?;
                let mut ids = prototype.instances_of(&expand).peekable();
                if ids.peek().is_none() {
                    return Err(format!("family {expand} has no instances"));
                }
                for id in ids {
                    let law = prototype.instance(&id).map_err(|e| format!("{e:?}"))?;
                    output.push((Pattern::literal(id), (*law).clone()));
                }
            }
        }
    }
    Ok(output)
}

#[derive(Deserialize)]
#[serde(
    deny_unknown_fields,
    bound(
        deserialize = "O::Account:Deserialize<'de>, O::Asset:Deserialize<'de>, O::Rate:Deserialize<'de>, O::Role:Deserialize<'de>"
    )
)]
pub struct Instance<O: Ontology> {
    pub ontology: OntologyPath,
    pub laws: Vec<LawSetPath>,
    #[serde(default)]
    pub tables: BTreeMap<Pattern<O::Rate>, Tables>,
    pub accounts: Vec<(O::Account, Account<O::Asset>)>,
    #[serde(default)]
    pub rates: Vec<Law<O>>,
    #[serde(default)]
    pub constants: BTreeMap<VariablePath, Segment>,
    #[serde(default)]
    pub invariants: Vec<LinearInvariant<O::Asset>>,
}

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Tables {
    pub variables: Vec<Variable>,
    pub relations: Vec<Relation>,
}

impl<O: Ontology> Instance<O> {
    pub fn build<'a>(
        self,
        mut resolve: impl FnMut(&LawSetPath) -> Option<&'a LawSet<O>>,
    ) -> Result<Economy<O>, String> {
        let mut builder = EconomyBuilder::new();
        for (name, value) in self.constants {
            builder = builder.constant_value(name, value);
        }
        for (id, account) in self.accounts {
            builder = builder.account(id, account);
        }
        let mut tables = self.tables;
        let inline = expand(self.rates)?;
        let sets = self
            .laws
            .iter()
            .map(|set| resolve(set).ok_or_else(|| format!("unknown law set {set}")))
            .collect::<Result<Vec<_>, _>>()?;
        for (key, law) in sets.into_iter().chain(std::iter::once(&inline)).flatten() {
            let mut rate = law.clone();
            if let Some(table) = tables.remove(key) {
                for v in table.variables {
                    rate = rate.variable(v);
                }
                for r in table.relations {
                    rate = rate.relation(r);
                }
            }
            builder = builder.rate_at(key.clone(), rate);
        }
        if let Some((key, _)) = tables.first_key_value() {
            return Err(format!("tables for unknown rate {key}"));
        }
        for invariant in self.invariants {
            builder = builder.invariant(invariant);
        }
        builder.build().map_err(|e| format!("{:?}", e.issues()))
    }
}
