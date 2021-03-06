use crate::ecosystem::Food;

#[derive(Debug)]
pub struct State {
    source: String,
    food_id: usize,
}

impl State {
    pub fn new(source: &str) -> Self {
        State {
            source: source.to_string(),
            food_id: 0,
        }
    }

    pub fn create_food(&mut self) -> Option<Food> {
        self.food_id += 1;
        Some(Food {
            id: self.food_id as i32,
            source: self.source.clone(),
            kind: 1,
            amount: 1,
        })
    }
}
