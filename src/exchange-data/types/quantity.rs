/// Fixed scale: 1000000 raw units per whole unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Quantity {
    raw: i64,
}
impl Quantity {
    pub fn from_raw(raw: i64) -> Self {
        Self { raw }
    }
    pub fn raw(&self) -> i64 {
        self.raw
    }
}
impl std::fmt::Display for Quantity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let raw = i128::from(self.raw);
        let sign = if raw < 0 { "-" } else { "" };
        let magnitude = raw.abs();
        write!(
            f,
            "{}{}.{:0digits$}",
            sign,
            magnitude / 1000000,
            magnitude % 1000000,
            digits = 6
        )
    }
}

impl std::str::FromStr for Quantity {
    type Err = QuantityError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (whole, fraction) = match s.split_once('.') {
            Some((whole, fraction)) => (whole, Some(fraction)),
            None => (s, None),
        };

        if whole.is_empty()
            || !whole.bytes().all(|b| b.is_ascii_digit())
        {
            return Err(QuantityError::InvalidFormat);
        }

        let whole = whole.parse::<i64>()
            .map_err(|_| QuantityError::Overflow)?;

        let fraction = match fraction {
            None => 0,
            Some(part) => {
                if part.is_empty()
                    || !part.bytes().all(|b| b.is_ascii_digit())
                {
                    return Err(QuantityError::InvalidFormat);
                }

                if part.len() > 6 {
                    return Err(QuantityError::TooManyDecimalPlaces);
                }

                let value = part.parse::<i64>()
                    .map_err(|_| QuantityError::InvalidFormat)?;

                value * 10_i64.pow((6 - part.len()) as u32)
            }
        };

        let raw = whole
            .checked_mul(1_000_000)
            .and_then(|value| value.checked_add(fraction))
            .ok_or(QuantityError::Overflow)?;

        Ok(Self::from_raw(raw))
    }
}

#[derive(Debug)]
pub enum QuantityError {
    InvalidFormat,
    TooManyDecimalPlaces,
    Overflow,
}
