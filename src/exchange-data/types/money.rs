/// Fixed scale: 100000000 raw units per whole unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Money {
    raw: i64,
}
impl Money {
    pub fn from_raw(raw: i64) -> Self {
        Self { raw }
    }
    pub fn raw(&self) -> i64 {
        self.raw
    }
}
impl std::fmt::Display for Money {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let raw = i128::from(self.raw);
        let sign = if raw < 0 { "-" } else { "" };
        let magnitude = raw.abs();
        write!(
            f,
            "{}{}.{:0digits$}",
            sign,
            magnitude / 100000000,
            magnitude % 100000000,
            digits = 8
        )
    }
}
