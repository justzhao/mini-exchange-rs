#[derive(Debug, Default)]
pub struct ReplayClock {
    now_ms: u64,
}

impl ReplayClock {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn now_ms(&self) -> u64 {
        self.now_ms
    }

    pub fn advance_to(&mut self, time_ms: u64) {
        self.now_ms = self.now_ms.max(time_ms);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clock_never_moves_backward() {
        let mut clock = ReplayClock::new();

        for (input, expected) in [
            (1000, 1000),
            (1500, 1500),
            (1500, 1500),
            (1400, 1500),
            (1600, 1600),
        ] {
            clock.advance_to(input);
            assert_eq!(clock.now_ms(), expected);
        }
    }
}