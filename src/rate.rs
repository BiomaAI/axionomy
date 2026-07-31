use crate::{Account, Basket, Quantity, QuantityScalar};
use num_traits::{CheckedAdd, Zero};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::hash::Hash;

#[derive(Debug, Clone)]
pub struct Rate<Role, A, N = u64> {
    consume: BTreeMap<Role, Basket<A, N>>,
    produce: BTreeMap<Role, Basket<A, N>>,
    preserve: BTreeMap<Role, Basket<A, N>>,
    roles: BTreeSet<Role>,
    distinct: BTreeSet<(Role, Role)>,
}

impl<Role, A, N> Rate<Role, A, N>
where
    Role: Clone + Ord,
    A: Clone + Eq + Hash,
    N: QuantityScalar,
{
    pub fn new() -> Self {
        Self {
            consume: BTreeMap::new(),
            produce: BTreeMap::new(),
            preserve: BTreeMap::new(),
            roles: BTreeSet::new(),
            distinct: BTreeSet::new(),
        }
    }

    pub fn consume(mut self, role: Role, basket: Basket<A, N>) -> Self {
        self.roles.insert(role.clone());
        merge(&mut self.consume, role, basket);
        self
    }

    pub fn produce(mut self, role: Role, basket: Basket<A, N>) -> Self {
        self.roles.insert(role.clone());
        merge(&mut self.produce, role, basket);
        self
    }

    pub fn preserve(mut self, role: Role, basket: Basket<A, N>) -> Self {
        self.roles.insert(role.clone());
        merge(&mut self.preserve, role, basket);
        self
    }

    pub fn distinct(mut self, left: Role, right: Role) -> Self {
        self.roles.insert(left.clone());
        self.roles.insert(right.clone());
        let pair = if left <= right {
            (left, right)
        } else {
            (right, left)
        };
        self.distinct.insert(pair);
        self
    }

    pub fn roles(&self) -> impl Iterator<Item = &Role> {
        self.roles.iter()
    }

    pub fn consumed(&self, role: &Role) -> Option<&Basket<A, N>> {
        self.consume.get(role)
    }

    pub fn produced(&self, role: &Role) -> Option<&Basket<A, N>> {
        self.produce.get(role)
    }

    pub fn preserved(&self, role: &Role) -> Option<&Basket<A, N>> {
        self.preserve.get(role)
    }

    pub fn distinct_roles(&self) -> impl Iterator<Item = &(Role, Role)> {
        self.distinct.iter()
    }
}

impl<Role, A, N> Default for Rate<Role, A, N>
where
    Role: Clone + Ord,
    A: Clone + Eq + Hash,
    N: QuantityScalar,
{
    fn default() -> Self {
        Self::new()
    }
}

fn merge<Role, A, N>(target: &mut BTreeMap<Role, Basket<A, N>>, role: Role, basket: Basket<A, N>)
where
    Role: Ord,
    A: Clone + Eq + Hash,
    N: QuantityScalar,
{
    if target
        .entry(role)
        .or_default()
        .checked_add(&basket)
        .is_err()
    {
        panic!("rate basket quantity overflow");
    }
}

#[derive(Debug, Clone)]
pub struct LinearInvariant<A> {
    name: String,
    weights: HashMap<A, i64>,
}

impl<A> LinearInvariant<A>
where
    A: Eq + Hash,
{
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            weights: HashMap::new(),
        }
    }

    pub fn weight(mut self, asset: A, weight: i64) -> Self {
        if weight == 0 {
            self.weights.remove(&asset);
        } else {
            self.weights.insert(asset, weight);
        }
        self
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn measure<AccountId, Holder, N>(
        &self,
        accounts: &HashMap<AccountId, Holder>,
    ) -> Option<N::SignedMeasure>
    where
        A: Clone,
        Holder: AsRef<Account<A, N>>,
        N: QuantityScalar,
    {
        let mut total = N::SignedMeasure::zero();
        for account in accounts.values() {
            for (asset, quantity) in account.as_ref().balances().iter() {
                let coefficient = *self.weights.get(asset).unwrap_or(&0);
                let weighted = quantity.as_scalar().checked_weighted(coefficient)?;
                total = CheckedAdd::checked_add(&total, &weighted)?;
            }
        }
        Some(total)
    }
}

pub fn basket<A, const LEN: usize>(entries: [(A, u64); LEN]) -> Basket<A>
where
    A: Eq + Hash,
{
    entries
        .map(|(asset, quantity)| (asset, Quantity::new(quantity)))
        .into()
}
