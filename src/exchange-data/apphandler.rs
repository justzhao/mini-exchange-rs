use super::books::bookhandler::{BookEvent, BookHandler};
use super::stream::{Handler, PrintHandler};
use crate::exchange_data::engine::ExecutionEngine;
use crate::exchange_data::signals::strategy::Strategy;

pub enum AppHandler<Ex: ExecutionEngine,S:Strategy> {
    Print(PrintHandler),
    Book(BookHandler<Ex,S>),
}

impl<Ev, Ex,S> Handler<Ev> for AppHandler<Ex,S>
where
    Ev: BookEvent + std::fmt::Debug + Send,
    Ex: ExecutionEngine,
    S:Strategy
{
    async fn on_event(&mut self, event: Ev) {
        match self {
            Self::Print(h) => h.on_event(event).await,
            Self::Book(h) => h.on_event(event).await,
        }
    }

    fn shutdown(
        self,
    ) -> impl Future<Output = Result<(), tokio::task::JoinError>> + Send {
        async move {
            match self {
                Self::Print(_) => Ok(()),
                Self::Book(handler) => {
                    BookHandler::shutdown(handler).await
                }
            }
        }
    }
}

