use crate::PbfStats;
use std::collections::HashSet;
use std::hash::Hash;

pub struct Secret<T> {
    in_order: Vec<T>,
    indexed: HashSet<T>,
}

impl<T: Clone + Hash + Eq> Secret<T> {
    pub fn new(available_guess: Vec<T>) -> Self {
        Self {
            indexed: available_guess.clone().into_iter().collect(),
            in_order: available_guess,
        }
    }

    pub fn compare(&self, guess: &Vec<T>) -> PbfStats {
        let f = self
            .in_order
            .iter()
            .enumerate()
            .zip(guess.iter().enumerate())
            .filter(|(a, b)| a == b)
            .count();
        let p = guess
            .iter()
            .filter(|guess| self.indexed.contains(guess))
            .count()
            - f;
        PbfStats {
            f: f as i32,
            p: p as i32,
        }
    }
}
