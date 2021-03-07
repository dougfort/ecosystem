use std::collections::HashMap;

use crate::ecosystem::Food;

const BIN_START_AMOUNT: usize = 100;
const BIN_MAX_AMOUNT: usize = 1000;
const FOOD_PORTION_AMOUNT: usize = 5;

#[derive(Debug)]
pub struct State {
    source: String,
    food_id: usize,
    kind: usize,
    bins: HashMap<usize, usize>,
}

impl State {
    pub fn new(source: &str, kind: usize) -> Self {
        let mut bins = HashMap::new();
        bins.insert(kind, BIN_START_AMOUNT);
        State {
            source: source.to_string(),
            food_id: 0,
            kind,
            bins,
        }
    }

    pub fn produce_food(&mut self) -> Food {
        self.food_id += 1;

        let available = match self.bins.get(&self.kind) {
            None => 0,
            Some(n) => *n,
        };
        let (amount, remainder) = if available < FOOD_PORTION_AMOUNT {
            (available, 0)
        } else {
            (FOOD_PORTION_AMOUNT, available - FOOD_PORTION_AMOUNT)
        };

        self.bins.insert(self.kind, remainder);

        Food {
            id: self.food_id as i32,
            source: self.source.to_string(),
            kind: self.kind as i32,
            amount: amount as i32,
        }
    }

    pub fn consume_food(&mut self, food: &Food) {
        let kind = food.kind as usize;
        let available = match self.bins.get(&kind) {
            None => 0,
            Some(n) => *n,
        };

        let new_available = std::cmp::max(available + food.amount as usize, BIN_MAX_AMOUNT);
        self.bins.insert(self.kind, new_available);
    }
}
