use std::{collections::BTreeMap, error::Error, fmt};

use surgeist_retained as retained;

use super::{AppInput, CorrelationId, Diagnostic, DiagnosticCode, InputProvenance, SurfaceId};

type CommandDecoder<T> =
    Box<dyn Fn(&retained::Command) -> Result<T, BridgeDecodeError> + Send + Sync + 'static>;

pub struct RetainedBridge<T> {
    decoders: BTreeMap<retained::CommandName, CommandDecoder<T>>,
}

impl<T> RetainedBridge<T> {
    #[must_use]
    pub fn new() -> Self {
        Self {
            decoders: BTreeMap::new(),
        }
    }

    #[must_use]
    pub fn command(
        mut self,
        name: retained::CommandName,
        decoder: impl Fn(&retained::Command) -> Result<T, BridgeDecodeError> + Send + Sync + 'static,
    ) -> Self {
        self.decoders.insert(name, Box::new(decoder));
        self
    }

    pub fn commands_to_inputs(
        &self,
        context: BridgeContext,
        commands: &[retained::Command],
    ) -> Result<Vec<AppInput<T>>, BridgeError> {
        commands
            .iter()
            .map(|command| self.command_to_input(&context, command))
            .collect()
    }

    fn command_to_input(
        &self,
        context: &BridgeContext,
        command: &retained::Command,
    ) -> Result<AppInput<T>, BridgeError> {
        let provenance = retained_provenance(context, command);
        let decoder = self.decoders.get(command.command()).ok_or_else(|| {
            BridgeError::new(Diagnostic::warning(
                DiagnosticCode::UNKNOWN_RETAINED_COMMAND,
                format!("unknown retained command `{}`", command.command()),
                provenance.clone(),
            ))
        })?;
        let payload = decoder(command).map_err(|error| {
            BridgeError::new(Diagnostic::warning(
                DiagnosticCode::INVALID_RETAINED_PAYLOAD,
                error.message,
                provenance.clone(),
            ))
        })?;

        Ok(AppInput::new(payload, provenance))
    }
}

impl<T> Default for RetainedBridge<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BridgeContext {
    surface_id: SurfaceId,
    route: retained::Route,
    correlation: CorrelationId,
}

impl BridgeContext {
    #[must_use]
    pub const fn new(
        surface_id: SurfaceId,
        route: retained::Route,
        correlation: CorrelationId,
    ) -> Self {
        Self {
            surface_id,
            route,
            correlation,
        }
    }

    #[must_use]
    pub const fn surface_id(&self) -> SurfaceId {
        self.surface_id
    }

    #[must_use]
    pub const fn route(&self) -> &retained::Route {
        &self.route
    }

    #[must_use]
    pub const fn correlation(&self) -> CorrelationId {
        self.correlation
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BridgeError {
    diagnostic: Box<Diagnostic>,
}

impl BridgeError {
    #[must_use]
    pub const fn diagnostic(&self) -> &Diagnostic {
        &self.diagnostic
    }

    fn new(diagnostic: Diagnostic) -> Self {
        Self {
            diagnostic: Box::new(diagnostic),
        }
    }
}

impl fmt::Display for BridgeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.diagnostic.message())
    }
}

impl Error for BridgeError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BridgeDecodeError {
    message: String,
}

impl BridgeDecodeError {
    #[must_use]
    pub fn invalid_payload(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for BridgeDecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl Error for BridgeDecodeError {}

fn retained_provenance(context: &BridgeContext, command: &retained::Command) -> InputProvenance {
    let provenance =
        InputProvenance::retained(context.surface_id).with_correlation(context.correlation);

    context
        .route
        .steps()
        .iter()
        .position(|step| step.id == command.target() && step.phase == command.phase())
        .map_or(provenance.clone(), |index| {
            provenance.with_sequence(index as u64)
        })
}
