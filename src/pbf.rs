use crate::secret::Secret;
use std::fmt;
use std::fmt::Display;

pub struct PbfStats {
    // Number of guess characters that exist in secret but not in the right position.
    pub p: i32,

    // Number of guess characters that exist in secret in the same position.
    pub f: i32,
}

impl PbfStats {
    pub fn create(secret: &Secret, guess: &str) -> Self {
        let mut p = 0;
        let mut f = 0;
        for (i, c) in guess.char_indices() {
            let i = i as i32;
            let secret_char = secret.get(&c);
            if let Some(secret_char_indicies) = secret_char {
                if secret_char_indicies.contains(&i) {
                    f = f + 1;
                } else {
                    p = p + 1;
                }
            }
        }
        PbfStats { p, f }
    }
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
