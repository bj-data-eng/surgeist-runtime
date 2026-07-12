//! App runtime and authoring DSL boundary for Surgeist.
//!
//! This module coordinates deterministic app state, retained UI surfaces,
//! resources, tasks, services, native wakeups, and declared effects. Native
//! window mechanics stay in `surgeist-window`.

mod bridge;
mod command;
mod coord;
mod descriptor;
mod diagnostic;
mod effect;
mod event;
mod ids;
mod input;
mod loop_;
mod provenance;
mod proxy;
mod reducer;
mod resource;
mod runtime;
mod service;
mod snapshot;
mod surface;
mod task;
pub mod testing;

#[cfg(test)]
mod tests;

pub use bridge::{BridgeContext, BridgeDecodeError, BridgeError, RetainedBridge};
pub use command::{AppCommand, CommandDescriptor, CommandName};
pub use coord::{
    AppScope, CoordinationState, ScopePathSegment, Subscription, SubscriptionPriority,
    SubscriptionTarget, SubscriptionTargetKindId,
};
pub use descriptor::{
    App, AppDescriptor, AppManifest, ResourceDescriptor, RootDescriptor, StartupWindow,
    TaskDescriptor, WindowDescriptor, WindowDescriptorId,
};
pub use diagnostic::{
    Diagnostic, DiagnosticCode, DiagnosticLog, DiagnosticSeverity, QueueDiagnostic,
};
pub use effect::{
    AppEffect, AppEffectPayload, CallServiceEffect, CancelTaskEffect, DiagnosticEffect,
    EffectBatch, EffectKindId, InvalidateResourceEffect, LoadResourceEffect, PersistEffect,
    RedrawTarget, ReprioritizeTaskEffect, RequestRedrawEffect, ServiceDiagnosticEffect,
    StartServiceEffect, StartTaskEffect, StopServiceEffect,
};
pub use event::{AppEvent, EventDescriptor, EventName};
pub use ids::{
    AppId, CalcId, CorrelationId, CustomScopeId, ElementId, ExpressionId, ResourceGeneration,
    ResourceId, ResourceOperationId, RootId, ServiceId, SurfaceGeneration, SurfaceId,
    SurfaceInvalidationGeneration, ValueExprId, VersionError, WindowId,
};
pub use input::AppInput;
pub use loop_::{AppHandler, AppLoop};
pub use provenance::{
    InputOrigin, InputProvenance, InputSourceId, ServiceProvenance, SurfaceProvenance,
    TaskProvenance,
};
pub use proxy::{AppProxy, AppProxyError, AppProxyErrorCode, ProxyInput, QueuePolicy, WakeBridge};
pub use reducer::{Reducer, ReducerResult};
pub use resource::{
    FailureVisibility, Freshness, ResourceSnapshot, ResourceState, ResourceStateReadyTransition,
    ResourceStatus,
};
pub use runtime::{
    Runtime, RuntimeBudget, RuntimeDrainReport, RuntimeInputError, RuntimeLane, RuntimeQueuePolicy,
    ServiceInput, TaskInput, UiInput,
};
pub use service::{
    MailboxOverflow, MailboxPolicy, ServiceCommandName, ServiceCommandPayload, ServiceMailbox,
    ServiceRegistration, ServiceRestart, ServiceShutdown, ServiceStartup, ServiceStatus,
};
pub use snapshot::{
    AppSnapshot, SnapshotBinding, SnapshotBindingId, SnapshotSourceType, StateVersion,
};
pub use surface::{
    ElementPhase, ElementRegistration, SurfaceElementRef, SurfaceElements, SurfaceError,
    SurfaceErrorCode, SurfaceInvalidation, SurfaceInvalidationKind, SurfaceLifecycle,
    SurfaceMutation, SurfacePoint, SurfaceRef, SurfaceRoot, SurfaceRoute, SurfaceRouteStep,
    SurfaceSize, UiSurface,
};
pub use task::{
    TaskIntentAttemptId, TaskIntentHandle, TaskIntentId, TaskIntentKey, TaskIntentName,
    TaskPriorityHint,
};
pub use testing::{FakeClock, FakeWakeBridge, FakeWindowBridge, HeadlessApp, HeadlessHarness};

/// Returns the crate identity while the runtime API is being designed.
#[must_use]
pub const fn crate_name() -> &'static str {
    "surgeist-runtime"
}
