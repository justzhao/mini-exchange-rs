use super::books::bookhandler::{BookEvent, BookHandler};
use super::stream::{Handler, PrintHandler};
use crate::exchange_data::engine::ExecutionEngine;

pub enum AppHandler<Ex: ExecutionEngine> {
    Print(PrintHandler),
    Book(BookHandler<Ex>),
}

impl<Ev, Ex> Handler<Ev> for AppHandler<Ex>
where
    Ev: BookEvent + std::fmt::Debug + Send,
    Ex: ExecutionEngine,
{
    fn on_event(&mut self, event: Ev) {
        match self {
            Self::Print(h) => h.on_event(event),
            Self::Book(h) => h.on_event(event),
        }
    }
}