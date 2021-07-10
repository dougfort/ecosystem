use crate::ecosystem::Food;

#[derive(Debug)]
pub struct State {
    source: String,
    food_id: usize,
    outgoing: String,
    incoming: String,
}

impl State {
    pub fn new(source: &str, outgoing: &str, incoming: &str) -> Self {
        State {
            source: source.to_string(),
            food_id: 0,
            outgoing: outgoing.to_string(),
            incoming: incoming.to_string(),
        }
    }

    pub fn produce_food(&mut self) -> Food {
        self.food_id += 1;

        Food {
            id: self.food_id as i32,
            source: self.source.to_string(),
            kind: self.outgoing.to_string(),
            amount: 1,
        }
    }

    pub fn consume_food(&mut self, food: &Food) -> bool {
        food.kind == self.incoming
    }
}
