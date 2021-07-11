use crate::ecosystem::Food;

#[derive(Debug)]
pub struct State {
    outgoing: String,
    incoming: String,
}

impl State {
    pub fn new(outgoing: &str, incoming: &str) -> Self {
        State {
            outgoing: outgoing.to_string(),
            incoming: incoming.to_string(),
        }
    }

    pub fn produce_food(&mut self) -> Food {
        Food {
            kind: self.outgoing.to_string(),
            amount: 1,
        }
    }

    pub fn consume_food(&mut self, food: &Food) -> bool {
        food.kind == self.incoming
    }
}
