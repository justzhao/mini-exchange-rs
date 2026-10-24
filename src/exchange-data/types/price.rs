
use std::fmt::Display;
use std::str::FromStr;

#[derive(Debug)]
pub enum PriceError {
    NonPositive,
    Overflow,
    InvalidFormat,
    TooManyDecimalPlaces,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Price {
    ticks: i64,
}

const TICKS_PER_UNIT: i64 = 100;

impl Price {
    // 在这里实现 from_ticks() 和 ticks()
    pub fn from_ticks(ticks:i64) -> Result<Self,PriceError> {

        if ticks<=0 {
           return Err(PriceError::NonPositive);     
        }

        Ok(Self { ticks })

    }

    pub  fn ticks(&self) ->i64{

        return self.ticks;
    }
}
impl std::str::FromStr for Price {

    type Err =PriceError ;

    fn from_str(s: &str) -> Result<Self, <Self as FromStr>::Err>{



        let (whole_str, fraction_str) = match s.split_once('.') {
            Some((whole, fraction)) => (whole, Some(fraction)),
            None => (s, None),
        };
    
        // 整数部分必须非空，且只能包含 ASCII 数字
        if whole_str.is_empty()
            || !whole_str.bytes().all(|b| b.is_ascii_digit()){
            return Err(PriceError::InvalidFormat);
        }
    

        let whole = whole_str.parse::<i64>()
        .map_err(|_| PriceError::Overflow)?;

    let fraction = match fraction_str {
        None => 0,
        Some(part) => {
            if part.is_empty()
                || !part.bytes().all(|b| b.is_ascii_digit())
            {
                return Err(PriceError::InvalidFormat);
            }

            if part.len() > 2 {
                return Err(PriceError::TooManyDecimalPlaces);
            }

            let value = part.parse::<i64>()
                .map_err(|_| PriceError::InvalidFormat)?;

            if part.len() == 1 {
                value * 10
            } else {
                value
            }
        }
    };

    let ticks = whole
        .checked_mul(TICKS_PER_UNIT)
        .and_then(|value| value.checked_add(fraction))
        .ok_or(PriceError::Overflow)?;

    Self::from_ticks(ticks)



        // let num=s.trim().parse::<i64>().map_err(|_|PriceError::NonPositive);

        // match num {
        //     Ok(whole) =>{
           

        //         let ticks = whole.checked_mul(TICKS_PER_UNIT)
        //             .ok_or(PriceError::Overflow)?;
        //         Self::from_ticks(ticks)
        //     },
        //     Err(e)=>{
        //         Err(e)
        //     }
        // }

    }



}

impl std::fmt::Display for Price {

    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let whole = self.ticks / TICKS_PER_UNIT;
        let fraction = self.ticks % TICKS_PER_UNIT;

        write!(f, "{}.{:02}", whole, fraction)
    }

}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_integer_prices_in_whole_units() {
        for (input, expected) in [("1", 100), ("3000", 300000)] {
            let price = input.parse::<Price>().expect("整数价格应合法");
            assert_eq!(price.ticks(), expected, "input={input:?}");
        }
    }

    #[test]
    fn parses_one_or_two_decimal_places() {
        for (input, expected) in [
            ("3000.2", 300020),
            ("3000.25", 300025),
            ("0.01", 1),
            ("1.05", 105),
        ] {
            let price = input.parse::<Price>()
                .unwrap_or_else(|err| panic!("input={input:?}, error={err:?}"));
            assert_eq!(price.ticks(), expected, "input={input:?}");
        }
    }

    #[test]
    fn rejects_zero_and_negative_prices() {
        for input in ["0", "0.00", "-1", "-0.01"] {
            assert!(input.parse::<Price>().is_err(), "input={input:?}");
        }
    }

    #[test]
    fn rejects_invalid_price_formats() {
        for input in ["", "abc", "1.2.3", ".5", "1.", "+1", "1e2", "NaN", "inf"] {
            assert!(input.parse::<Price>().is_err(), "input={input:?}");
        }
    }

    #[test]
    fn rejects_whitespace_in_prices() {
        for input in [" 1", "1 ", "\t1", "1\n", "1 0"] {
            assert!(input.parse::<Price>().is_err(), "input={input:?}");
        }
    }

    #[test]
    fn rejects_more_than_two_decimal_places_without_rounding() {
        for input in ["1.234", "1.000", "0.001"] {
            assert!(input.parse::<Price>().is_err(), "input={input:?}");
        }
    }

    #[test]
    fn parses_largest_representable_price() {
        let price = "92233720368547758.07".parse::<Price>()
            .expect("价格上界应可精确表示");
        assert_eq!(price.ticks(), i64::MAX);
    }

    #[test]
    fn rejects_prices_that_overflow_when_scaled() {
        for input in ["92233720368547758.08", "92233720368547759", "999999999999999999999"] {
            assert!(input.parse::<Price>().is_err(), "input={input:?}");
        }
    }

    #[test]
    fn displays_price_with_two_decimal_places() {
        let cases = [
            (1, "0.01"),
            (105, "1.05"),
            (300025, "3000.25"),
        ];

        for (ticks, expected) in cases {
            let price = Price::from_ticks(ticks)
                .expect("测试价格应合法");

            assert_eq!(price.to_string(), expected);
        }
    }
}
