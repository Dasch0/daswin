use super::runner;
use crate::event::Event;
use crate::event_loop::EventLoopClosed;

pub struct EventLoopProxy<'event_loop, T> {
    runner: runner::Shared<'event_loop, T>,
}

impl<'event_loop, T> EventLoopProxy<T> {
    pub fn new(runner: runner::Shared<'event_loop, T>) -> Self {
        Self { runner }
    }

    pub fn send_event(&self, event: T) -> Result<(), EventLoopClosed<T>> {
        self.runner.send_event(Event::UserEvent(event));
        Ok(())
    }
}

impl<'event_loop, T> Clone for EventLoopProxy<'event_loop, T> {
    fn clone(&self) -> Self {
        Self {
            runner: self.runner.clone(),
        }
    }
}
