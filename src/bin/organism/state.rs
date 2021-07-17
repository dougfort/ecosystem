use crate::ecosystem::Food;

const OUTGOING_START_AMOUNT: usize = 1000;

#[derive(Debug)]
pub struct State {
    outgoing_amount: usize,
    outgoing: String,
    incoming_amount: usize,
    incoming: String,
}

impl State {
    pub fn new(outgoing: &str, incoming: &str) -> Self {
        State {
            outgoing_amount: OUTGOING_START_AMOUNT,
            outgoing: outgoing.to_string(),
            incoming_amount: 0,
            incoming: incoming.to_string(),
        }
    }

    pub fn produce_food(&mut self) -> Option<Food> {
        if self.outgoing_amount > 0 {
            let amount = 1;
            self.outgoing_amount -= amount;
            Some(Food {
                kind: self.outgoing.to_string(),
                amount: amount as i32,
            })
        } else {
            None
        }
    }

    pub fn consume_food(&mut self, food: &Food) -> bool {
        if food.kind == self.incoming {
            self.incoming_amount += food.amount as usize;
            return true
        }

        false
    }
}
