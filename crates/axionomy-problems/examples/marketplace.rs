use axionomy_problems::marketplace::{self, Assessment, Asset};

fn operational_cost(assessment: &Assessment) -> u64 {
    assessment
        .shortfalls()
        .iter()
        .flat_map(|shortfall| shortfall.missing().iter())
        .map(|(asset, quantity)| {
            let weight = match asset {
                Asset::Money => 1,
                Asset::ShippingCapacity => 50,
                Asset::Widget => 100,
                _ => 1_000,
            };
            weight * quantity.get()
        })
        .sum()
}

fn main() {
    let mut market = marketplace::initial();
    let candidate_count = marketplace::candidates(&market).len();
    let exact = marketplace::exact_matches(&market);
    let near = marketplace::rank_near_matches(&market, operational_cost);

    let closest = near.first().expect("the bounded market has near matches");
    assert_eq!(closest.assessment().shortfalls().len(), 1);

    let settlement = exact.first().expect("the market has exact matches").clone();
    let projected_accounts = market
        .assess(&settlement)
        .projected_deltas()
        .expect("an exact match projects its effects")
        .len();
    let receipt = market.apply(settlement).expect("the exact match settles");

    assert_eq!(receipt.deltas().len(), projected_accounts);
    assert!(market.matches(&marketplace::goal()));

    println!(
        "Marketplace: {candidate_count} candidates, {} exact, closest near match {:?}, \
         {} accounts settled atomically",
        exact.len(),
        closest.candidate(),
        receipt.deltas().len(),
    );
}
