// Map of character -> set<Positions it appears in>.
use std::collections::{HashMap, HashSet};

pub type Secret = HashMap<char, HashSet<i32>>;
