let pantry = model();
let meal = asset("meal");
pantry.open(account("stockroom"), basket([amount(meal, 3)]));
pantry.open(account("counter"), basket([]));

let meals = invariant("meals");
meals.weight(meal, 1);
pantry.add(meals.finish());
pantry.instance("pantry", ["delivery"])
