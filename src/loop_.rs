use super::{AppProxy, ProxyInput, Runtime};

/// Adapter boundary for native event-loop callbacks entering the app runtime.
///
/// Native hosts should decode host events into typed app inputs, enqueue UI work
/// directly, and route proxy-drained task and service inputs through their
/// matching runtime lanes.
pub trait AppHandler<State, Reducer, Input> {
    fn enqueue_proxy_input(
        &mut self,
        runtime: &mut Runtime<State, Reducer, Input>,
        input: ProxyInput<Input>,
    ) {
        match input {
            ProxyInput::Task(input) => runtime.enqueue_task(input),
            ProxyInput::Service(input) => runtime.enqueue_service(input),
        }
    }
}

pub struct AppLoop<State = (), Reducer = (), Input = (), Handler = ()> {
    runtime: Runtime<State, Reducer, Input>,
    native_loop: Option<surgeist_window::Loop<Handler>>,
}

impl Default for AppLoop<(), (), (), ()> {
    fn default() -> Self {
        Self::new(Runtime::default())
    }
}

impl<State, Reducer, Input> AppLoop<State, Reducer, Input, ()> {
    #[must_use]
    pub fn new(runtime: Runtime<State, Reducer, Input>) -> Self {
        Self {
            runtime,
            native_loop: None,
        }
    }
}

impl<State, Reducer, Input, Handler> AppLoop<State, Reducer, Input, Handler> {
    #[must_use]
    pub fn with_native_loop(
        runtime: Runtime<State, Reducer, Input>,
        native_loop: surgeist_window::Loop<Handler>,
    ) -> Self {
        Self {
            runtime,
            native_loop: Some(native_loop),
        }
    }

    #[must_use]
    pub const fn runtime(&self) -> &Runtime<State, Reducer, Input> {
        &self.runtime
    }

    pub fn runtime_mut(&mut self) -> &mut Runtime<State, Reducer, Input> {
        &mut self.runtime
    }

    #[must_use]
    pub const fn native_loop(&self) -> Option<&surgeist_window::Loop<Handler>> {
        self.native_loop.as_ref()
    }

    pub fn native_loop_mut(&mut self) -> Option<&mut surgeist_window::Loop<Handler>> {
        self.native_loop.as_mut()
    }

    pub fn drain_proxy(&mut self, proxy: &AppProxy<Input>, limit: usize) -> usize {
        let drained = proxy.drain_pending(limit);
        let drained_len = drained.len();
        for input in drained {
            match input {
                ProxyInput::Task(input) => self.runtime.enqueue_task(input),
                ProxyInput::Service(input) => self.runtime.enqueue_service(input),
            }
        }
        drained_len
    }
}
