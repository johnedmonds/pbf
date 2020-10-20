mod solver;

use std::fmt;
use std::fmt::Display;

#[derive(Eq, PartialEq)]
pub struct PbfStats {
    // Number of guess characters that exist in secret but not in the right position.
    pub p: i32,

    // Number of guess characters that exist in secret in the same position.
    pub f: i32,
}

impl Display for PbfStats {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.p == 0 && self.f == 0 {
            write!(f, "b")
        } else {
            write!(
                f,
                "{}{}",
                vec!["f"; self.f as usize].join(""),
                vec!["p"; self.p as usize].join("")
            )
        }
    }
}
