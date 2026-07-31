use crate::{Account, Basket, Quantity};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::hash::Hash;

#[derive(Debug, Clone)]
pub struct Rate<Role, A> {
    consume: BTreeMap<Role, Basket<A>>,
    produce: BTreeMap<Role, Basket<A>>,
    preserve: BTreeMap<Role, Basket<A>>,
    roles: BTreeSet<Role>,
    distinct: BTreeSet<(Role, Role)>,
}

impl<Role, A> Rate<Role, A>
where
    Role: Clone + Ord,
    A: Clone + Eq + Hash,
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

    pub fn consume(mut self, role: Role, basket: Basket<A>) -> Self {
        self.roles.insert(role.clone());
        merge(&mut self.consume, role, basket);
        self
    }

    pub fn produce(mut self, role: Role, basket: Basket<A>) -> Self {
        self.roles.insert(role.clone());
        merge(&mut self.produce, role, basket);
        self
    }

    pub fn preserve(mut self, role: Role, basket: Basket<A>) -> Self {
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

    pub fn consumed(&self, role: &Role) -> Option<&Basket<A>> {
        self.consume.get(role)
    }

    pub fn produced(&self, role: &Role) -> Option<&Basket<A>> {
        self.produce.get(role)
    }

    pub fn preserved(&self, role: &Role) -> Option<&Basket<A>> {
        self.preserve.get(role)
    }

    pub fn distinct_roles(&self) -> impl Iterator<Item = &(Role, Role)> {
        self.distinct.iter()
    }
}

impl<Role, A> Default for Rate<Role, A>
where
    Role: Clone + Ord,
    A: Clone + Eq + Hash,
{
    fn default() -> Self {
        Self::new()
    }
}

fn merge<Role, A>(target: &mut BTreeMap<Role, Basket<A>>, role: Role, basket: Basket<A>)
where
    Role: Ord,
    A: Clone + Eq + Hash,
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

    pub fn measure<AccountId>(&self, accounts: &HashMap<AccountId, Account<A>>) -> Option<i128>
    where
        A: Clone,
    {
        let mut total = 0_i128;
        for account in accounts.values() {
            for (asset, quantity) in account.balances().iter() {
                let weight = i128::from(*self.weights.get(asset).unwrap_or(&0));
                let weighted = weight.checked_mul(i128::from(quantity.get()))?;
                total = total.checked_add(weighted)?;
            }
        }
        Some(total)
    }
}

pub fn basket<A, const N: usize>(entries: [(A, u64); N]) -> Basket<A>
where
    A: Eq + Hash,
{
    entries
        .map(|(asset, quantity)| (asset, Quantity::new(quantity)))
        .into()
}
