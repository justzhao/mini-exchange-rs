use super::books::bookhandler::{BookEvent, BookHandler};
use super::stream::{Handler, PrintHandler};

pub enum AppHandler {
    Print(PrintHandler),
    Book(BookHandler),
}

impl<E> Handler<E> for AppHandler
where
    E: BookEvent + std::fmt::Debug + Send,
{
    fn on_event(&mut self, event: E) {
        match self {
            Self::Print(h) => h.on_event(event),
            Self::Book(h) => h.on_event(event),
        }
    }
}