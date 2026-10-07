use crate::exchange_data::books::localbook::LocalBook;

pub struct OrderBookFeature{

    pub  spread:f64,
    pub mid:f64,
    pub imbalance:f64,
    pub  microprice:f64
}

impl OrderBookFeature{

    pub fn new(book  : &LocalBook )->Option<Self> {

        Some(Self {
            spread: book.spread()?,
            mid: book.mid()?,
            imbalance: book.imbalance()?,
            microprice: book.microprice()?,
        })

    }

}