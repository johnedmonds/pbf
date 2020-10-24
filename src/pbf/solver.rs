use std::fmt::Debug;
use crate::secret::Secret;
use crate::PbfStats;
use itertools::Itertools;
use std::collections::HashSet;
use std::hash::Hash;

fn real_combinations<T, I>(items: I, times: usize) -> impl Iterator<Item=Vec<T>>
where I: Iterator<Item=T> + Clone, T: Clone {
    std::iter::repeat(items).take(times).multi_cartesian_product()
}

// A guess someone has made with the results.
pub struct Guess<T> {
    pub guess: Vec<T>,
    pub result: PbfStats,
}

pub struct GuessState<T> {
    cached_potential_outcomes: Vec<PbfStats>,

    guesses: Vec<Guess<T>>,

    // Based on the guesses, which combinations are still valid, A combination is valid if--were it to be the real secret number--the result of applying each guess in the set of guesses to the combination would produce the result associated with the guess.
    available_guesses: Vec<Secret<T>>,

    // The things you can guess. For Pico, Bagel, Fermi, this is the digits from 0 to 9, inclusive.
    guess_space: Vec<T>,

    // Length of the solution.
    guess_length: usize,
}

impl<T> GuessState<T>
where
    T: Clone + Eq + Hash + Debug,
{
    pub fn new(guess_space: Vec<T>, guess_length: usize) -> GuessState<T> {
        #[derive(Clone, Eq, PartialEq)]
        enum Pbf {
            P,
            B,
            F,
        }
        let cached_potential_outcomes = real_combinations(vec![Pbf::P, Pbf::B, Pbf::F].into_iter(), guess_length)
            .map(|result| PbfStats {
                p: result.iter().filter(|pbf| pbf == &&Pbf::P).count() as i32,
                f: result.iter().filter(|pbf| pbf == &&Pbf::F).count() as i32,
            })
            .collect();
        Self {
            cached_potential_outcomes,
            guesses: Vec::new(),
            available_guesses: real_combinations(guess_space.clone().into_iter(), guess_length)
                .map(Secret::new)
                .collect(),
            guess_space: guess_space,
            guess_length,
        }
    }

    pub fn guesses(&self) -> &Vec<Guess<T>> {
        &self.guesses
    }

    pub fn add_guess(&mut self, guess: Guess<T>) {
        self.guesses.push(guess);

        // Rust doesn't like when we use self for some reason so work around it by borrowing here.
        let self_guesses = &self.guesses;
        self.available_guesses.retain(|possible_solution| {
            self_guesses
                .iter()
                .all(|guess| {
                    possible_solution.compare(&guess.guess) == guess.result
                })
        });
    }

    pub fn next_guess(&self) -> Option<Vec<T>> {
        if let [answer] = &*self.available_guesses {
            Some(answer.as_guess().to_vec())
        } else {
        // Index the guesses so we can quickly check whether we've already guessed it.
        let indexed_guesses: HashSet<&Vec<T>> =
            self.guesses.iter().map(|guess| &guess.guess).collect();
        real_combinations(self.guess_space.clone().into_iter(), self.guess_length)
            .filter(|guess| !indexed_guesses.contains(guess))
            .max_by_key(|guess| self.score_guess(guess))
        }
    }

    fn score_guess(&self, guess: &Vec<T>) -> Option<usize> {
        self
            .cached_potential_outcomes
            .iter()
            .map(|outcome| {
                self.available_guesses
                    .iter()
                    .filter(|available_guess| {
                        &available_guess.compare(guess) != outcome
                    })
                    .count()
            })
            .min()
    }
}
