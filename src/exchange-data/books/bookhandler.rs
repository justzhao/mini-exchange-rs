use crate::exchange_data::books::localbook::LocalBook;
use crate::exchange_data::books::localbook::BookSnapshot;
use crate::exchange_data::stream::Handler;
pub struct BookHandler {
    book: LocalBook,
}
impl BookHandler {
    pub fn new(coin: impl Into<String>) -> Self {
        Self {
            book: LocalBook::new(coin),
        }
    }
}

pub trait BookEvent {
    type Snap: BookSnapshot + std::fmt::Debug;
    fn as_book(&self) -> Option<&Self::Snap>;
}


impl<E: BookEvent> Handler<E> for BookHandler {
    fn on_event(&mut self, event: E) {
        if let Some(snap) = event.as_book() {        
            println!("BookHandler event is {snap:?}");
            self.book.rebuild_book(snap);
        }
    }
}


