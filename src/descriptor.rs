use std::{
    collections::{BTreeMap, BTreeSet, btree_map::Entry},
    error::Error,
    fmt,
};

use super::{
    AppId, AppScope, CommandDescriptor, CommandName, EventDescriptor, EventName, NameError,
    PayloadTypeName, ResourceId, RootId, SnapshotBinding, SnapshotBindingId, TaskIntentName,
};

/// A runtime application that owns one validated manifest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct App {
    manifest: ValidatedAppManifest,
}

impl App {
    /// Validates an authored manifest and stores the resulting immutable manifest.
    pub fn try_new(manifest: AppManifest) -> Result<Self, ManifestValidationError> {
        manifest.validate().map(|manifest| Self { manifest })
    }

    /// Returns the validated manifest owned by this application.
    #[must_use]
    pub fn manifest(&self) -> &ValidatedAppManifest {
        &self.manifest
    }

    /// Returns this application's descriptor.
    #[must_use]
    pub fn descriptor(&self) -> &AppDescriptor {
        self.manifest().app()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppDescriptor {
    id: AppId,
    version: String,
    diagnostics_namespace: String,
}

impl AppDescriptor {
    #[must_use]
    pub fn new(id: AppId, version: impl Into<String>) -> Self {
        let diagnostics_namespace = id.as_str().to_owned();
        Self {
            id,
            version: version.into(),
            diagnostics_namespace,
        }
    }

    #[must_use]
    pub fn id(&self) -> &AppId {
        &self.id
    }

    #[must_use]
    pub fn version(&self) -> &str {
        &self.version
    }

    #[must_use]
    pub fn diagnostics_namespace(&self) -> &str {
        &self.diagnostics_namespace
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct WindowDescriptorId(String);

impl WindowDescriptorId {
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WindowDescriptor {
    id: WindowDescriptorId,
    title: String,
    allowed_roots: Vec<RootId>,
}

impl WindowDescriptor {
    #[must_use]
    pub fn new(id: WindowDescriptorId, title: impl Into<String>) -> Self {
        Self {
            id,
            title: title.into(),
            allowed_roots: Vec::new(),
        }
    }

    #[must_use]
    pub fn allows_root(mut self, id: RootId) -> Self {
        self.allowed_roots.push(id);
        self
    }

    #[must_use]
    pub fn id(&self) -> &WindowDescriptorId {
        &self.id
    }

    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }

    #[must_use]
    pub fn allowed_roots(&self) -> &[RootId] {
        &self.allowed_roots
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RootDescriptor {
    id: RootId,
    required_commands: Vec<CommandDescriptor>,
    required_events: Vec<EventDescriptor>,
    snapshot_bindings: Vec<SnapshotBinding>,
}

impl RootDescriptor {
    #[must_use]
    pub fn new(id: RootId) -> Self {
        Self {
            id,
            required_commands: Vec::new(),
            required_events: Vec::new(),
            snapshot_bindings: Vec::new(),
        }
    }

    #[must_use]
    pub fn requires_command(mut self, descriptor: CommandDescriptor) -> Self {
        self.required_commands.push(descriptor);
        self
    }

    #[must_use]
    pub fn emits_event(mut self, descriptor: EventDescriptor) -> Self {
        self.required_events.push(descriptor);
        self
    }

    #[must_use]
    pub fn binds_snapshot(mut self, binding: SnapshotBinding) -> Self {
        self.snapshot_bindings.push(binding);
        self
    }

    #[must_use]
    pub fn id(&self) -> &RootId {
        &self.id
    }

    #[must_use]
    pub fn required_commands(&self) -> &[CommandDescriptor] {
        &self.required_commands
    }

    #[must_use]
    pub fn required_events(&self) -> &[EventDescriptor] {
        &self.required_events
    }

    #[must_use]
    pub fn snapshot_bindings(&self) -> &[SnapshotBinding] {
        &self.snapshot_bindings
    }
}

/// Declares an abstract task intent and its semantic input type.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaskDescriptor {
    name: TaskIntentName,
    input_type: PayloadTypeName,
}

impl TaskDescriptor {
    /// Creates a task descriptor after validating its semantic input type.
    pub fn try_new(name: TaskIntentName, input_type: impl Into<String>) -> Result<Self, NameError> {
        Ok(Self {
            name,
            input_type: PayloadTypeName::try_new_for_field(input_type, "task.input_type")?,
        })
    }

    /// Returns the abstract task intent name.
    #[must_use]
    pub fn name(&self) -> &TaskIntentName {
        &self.name
    }

    /// Returns the semantic input type name.
    #[must_use]
    pub fn input_type(&self) -> &PayloadTypeName {
        &self.input_type
    }
}

/// Declares a resource and its semantic value type.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourceDescriptor {
    id: ResourceId,
    value_type: PayloadTypeName,
}

impl ResourceDescriptor {
    /// Creates a resource descriptor after validating its semantic value type.
    pub fn try_new(id: ResourceId, value_type: impl Into<String>) -> Result<Self, NameError> {
        Ok(Self {
            id,
            value_type: PayloadTypeName::try_new_for_field(value_type, "resource.value_type")?,
        })
    }

    /// Returns the resource identifier.
    #[must_use]
    pub fn id(&self) -> &ResourceId {
        &self.id
    }

    /// Returns the semantic value type name.
    #[must_use]
    pub fn value_type(&self) -> &PayloadTypeName {
        &self.value_type
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StartupWindow {
    window_id: WindowDescriptorId,
    root_id: RootId,
    scope: AppScope,
}

impl StartupWindow {
    #[must_use]
    pub const fn new(window_id: WindowDescriptorId, root_id: RootId, scope: AppScope) -> Self {
        Self {
            window_id,
            root_id,
            scope,
        }
    }

    #[must_use]
    pub fn window_id(&self) -> &WindowDescriptorId {
        &self.window_id
    }

    #[must_use]
    pub fn root_id(&self) -> &RootId {
        &self.root_id
    }

    #[must_use]
    pub fn scope(&self) -> &AppScope {
        &self.scope
    }
}

/// An authored, unchecked set of application descriptors.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppManifest {
    app: AppDescriptor,
    commands: Vec<CommandDescriptor>,
    events: Vec<EventDescriptor>,
    tasks: Vec<TaskDescriptor>,
    resources: Vec<ResourceDescriptor>,
    windows: Vec<WindowDescriptor>,
    roots: Vec<RootDescriptor>,
    startup: Vec<StartupWindow>,
}

impl AppManifest {
    /// Starts an authored manifest for an application descriptor.
    #[must_use]
    pub fn new(app: AppDescriptor) -> Self {
        Self {
            app,
            commands: Vec::new(),
            events: Vec::new(),
            tasks: Vec::new(),
            resources: Vec::new(),
            windows: Vec::new(),
            roots: Vec::new(),
            startup: Vec::new(),
        }
    }

    #[must_use]
    pub fn command(mut self, descriptor: CommandDescriptor) -> Self {
        self.commands.push(descriptor);
        self
    }

    #[must_use]
    pub fn event(mut self, descriptor: EventDescriptor) -> Self {
        self.events.push(descriptor);
        self
    }

    #[must_use]
    pub fn task(mut self, descriptor: TaskDescriptor) -> Self {
        self.tasks.push(descriptor);
        self
    }

    #[must_use]
    pub fn resource(mut self, descriptor: ResourceDescriptor) -> Self {
        self.resources.push(descriptor);
        self
    }

    #[must_use]
    pub fn window(mut self, descriptor: WindowDescriptor) -> Self {
        self.windows.push(descriptor);
        self
    }

    #[must_use]
    pub fn root(mut self, descriptor: RootDescriptor) -> Self {
        self.roots.push(descriptor);
        self
    }

    #[must_use]
    pub fn startup_window(mut self, descriptor: StartupWindow) -> Self {
        self.startup.push(descriptor);
        self
    }

    #[must_use]
    pub fn app(&self) -> &AppDescriptor {
        &self.app
    }

    #[must_use]
    pub fn commands(&self) -> &[CommandDescriptor] {
        &self.commands
    }

    #[must_use]
    pub fn events(&self) -> &[EventDescriptor] {
        &self.events
    }

    #[must_use]
    pub fn tasks(&self) -> &[TaskDescriptor] {
        &self.tasks
    }

    #[must_use]
    pub fn resources(&self) -> &[ResourceDescriptor] {
        &self.resources
    }

    #[must_use]
    pub fn windows(&self) -> &[WindowDescriptor] {
        &self.windows
    }

    #[must_use]
    pub fn roots(&self) -> &[RootDescriptor] {
        &self.roots
    }

    #[must_use]
    pub fn startup(&self) -> &[StartupWindow] {
        &self.startup
    }

    /// Consumes this authored manifest into an immutable validated manifest.
    pub fn validate(self) -> Result<ValidatedAppManifest, ManifestValidationError> {
        let Self {
            app,
            commands,
            events,
            tasks,
            resources,
            windows,
            roots,
            mut startup,
        } = self;

        let mut issues = Vec::new();

        let mut command_index = BTreeMap::new();
        let mut duplicate_commands = Vec::new();
        for descriptor in commands {
            let name = descriptor.name().clone();
            match command_index.entry(name) {
                Entry::Vacant(entry) => {
                    entry.insert(descriptor);
                }
                Entry::Occupied(entry) => {
                    duplicate_commands.push(entry.key().clone());
                }
            }
        }
        duplicate_commands.sort();
        issues.extend(duplicate_commands.into_iter().map(|command_name| {
            ManifestValidationIssue::new(ManifestValidationErrorCode::DuplicateCommand)
                .with_command_name(command_name)
        }));

        let mut event_index = BTreeMap::new();
        let mut duplicate_events = Vec::new();
        for descriptor in events {
            let name = descriptor.name().clone();
            match event_index.entry(name) {
                Entry::Vacant(entry) => {
                    entry.insert(descriptor);
                }
                Entry::Occupied(entry) => {
                    duplicate_events.push(entry.key().clone());
                }
            }
        }
        duplicate_events.sort();
        issues.extend(duplicate_events.into_iter().map(|event_name| {
            ManifestValidationIssue::new(ManifestValidationErrorCode::DuplicateEvent)
                .with_event_name(event_name)
        }));

        let mut task_index = BTreeMap::new();
        let mut duplicate_tasks = Vec::new();
        for descriptor in tasks {
            let name = descriptor.name().clone();
            match task_index.entry(name) {
                Entry::Vacant(entry) => {
                    entry.insert(descriptor);
                }
                Entry::Occupied(entry) => {
                    duplicate_tasks.push(entry.key().clone());
                }
            }
        }
        duplicate_tasks.sort();
        issues.extend(
            duplicate_tasks
                .into_iter()
                .map(|_| ManifestValidationIssue::new(ManifestValidationErrorCode::DuplicateTask)),
        );

        let mut resource_index = BTreeMap::new();
        let mut duplicate_resources = Vec::new();
        for descriptor in resources {
            let id = descriptor.id().clone();
            match resource_index.entry(id) {
                Entry::Vacant(entry) => {
                    entry.insert(descriptor);
                }
                Entry::Occupied(entry) => {
                    duplicate_resources.push(entry.key().clone());
                }
            }
        }
        duplicate_resources.sort();
        issues.extend(
            duplicate_resources.into_iter().map(|_| {
                ManifestValidationIssue::new(ManifestValidationErrorCode::DuplicateResource)
            }),
        );

        let mut window_index = BTreeMap::new();
        let mut duplicate_windows = Vec::new();
        for descriptor in windows {
            let id = descriptor.id().clone();
            match window_index.entry(id) {
                Entry::Vacant(entry) => {
                    entry.insert(descriptor);
                }
                Entry::Occupied(entry) => {
                    duplicate_windows.push(entry.key().clone());
                }
            }
        }
        duplicate_windows.sort();
        issues.extend(duplicate_windows.into_iter().map(|window_id| {
            ManifestValidationIssue::new(ManifestValidationErrorCode::DuplicateWindow)
                .with_window_id(window_id)
        }));

        let mut root_index = BTreeMap::new();
        let mut duplicate_roots = Vec::new();
        for descriptor in roots {
            let id = descriptor.id().clone();
            match root_index.entry(id) {
                Entry::Vacant(entry) => {
                    entry.insert(descriptor);
                }
                Entry::Occupied(entry) => {
                    duplicate_roots.push(entry.key().clone());
                }
            }
        }
        duplicate_roots.sort();
        issues.extend(duplicate_roots.into_iter().map(|root_id| {
            ManifestValidationIssue::new(ManifestValidationErrorCode::DuplicateRoot)
                .with_root_id(root_id)
        }));

        for root in root_index.values() {
            let root_id = root.id().clone();
            let mut bindings = root.snapshot_bindings().iter().collect::<Vec<_>>();
            bindings.sort_by_key(|binding| binding.id());
            let mut binding_ids = BTreeSet::new();
            for binding in bindings {
                let binding_id = binding.id().clone();
                if !binding_ids.insert(binding_id.clone()) {
                    issues.push(
                        ManifestValidationIssue::new(
                            ManifestValidationErrorCode::DuplicateRootSnapshotBinding,
                        )
                        .with_root_id(root_id.clone())
                        .with_snapshot_binding_id(binding_id),
                    );
                }
            }

            let mut required_commands = root.required_commands().iter().collect::<Vec<_>>();
            required_commands.sort_by_key(|descriptor| descriptor.name());
            for required in required_commands {
                let command_name = required.name().clone();
                match command_index.get(&command_name) {
                    None => issues.push(
                        ManifestValidationIssue::new(ManifestValidationErrorCode::MissingCommand)
                            .with_root_id(root_id.clone())
                            .with_command_name(command_name),
                    ),
                    Some(declared) if declared.payload_type() != required.payload_type() => {
                        issues.push(
                            ManifestValidationIssue::new(
                                ManifestValidationErrorCode::CommandPayloadTypeMismatch,
                            )
                            .with_root_id(root_id.clone())
                            .with_command_name(command_name)
                            .with_expected_payload_type(declared.payload_type().clone())
                            .with_actual_payload_type(required.payload_type().clone()),
                        );
                    }
                    Some(_) => {}
                }
            }

            let mut emitted_events = root.required_events().iter().collect::<Vec<_>>();
            emitted_events.sort_by_key(|descriptor| descriptor.name());
            for emitted in emitted_events {
                let event_name = emitted.name().clone();
                match event_index.get(&event_name) {
                    None => issues.push(
                        ManifestValidationIssue::new(ManifestValidationErrorCode::MissingEvent)
                            .with_root_id(root_id.clone())
                            .with_event_name(event_name),
                    ),
                    Some(declared) if declared.payload_type() != emitted.payload_type() => {
                        issues.push(
                            ManifestValidationIssue::new(
                                ManifestValidationErrorCode::EventPayloadTypeMismatch,
                            )
                            .with_root_id(root_id.clone())
                            .with_event_name(event_name)
                            .with_expected_payload_type(declared.payload_type().clone())
                            .with_actual_payload_type(emitted.payload_type().clone()),
                        );
                    }
                    Some(_) => {}
                }
            }
        }

        startup.sort_by(|left, right| {
            left.window_id()
                .cmp(right.window_id())
                .then_with(|| left.root_id().cmp(right.root_id()))
                .then_with(|| left.scope().cmp(right.scope()))
        });
        for startup_window in &startup {
            let window_id = startup_window.window_id().clone();
            let root_id = startup_window.root_id().clone();
            let window = window_index.get(&window_id);
            if window.is_none() {
                issues.push(
                    ManifestValidationIssue::new(ManifestValidationErrorCode::UnknownStartupWindow)
                        .with_window_id(window_id.clone())
                        .with_root_id(root_id.clone()),
                );
            }
            if !root_index.contains_key(&root_id) {
                issues.push(
                    ManifestValidationIssue::new(ManifestValidationErrorCode::UnknownStartupRoot)
                        .with_window_id(window_id.clone())
                        .with_root_id(root_id.clone()),
                );
            }
            if let Some(window) = window
                && !window.allowed_roots().is_empty()
                && !window.allowed_roots().contains(&root_id)
            {
                issues.push(
                    ManifestValidationIssue::new(
                        ManifestValidationErrorCode::DisallowedStartupRoot,
                    )
                    .with_window_id(window_id)
                    .with_root_id(root_id),
                );
            }
        }

        if !window_index.is_empty() && startup.is_empty() {
            issues.push(ManifestValidationIssue::new(
                ManifestValidationErrorCode::MissingStartupRoot,
            ));
        }

        if issues.is_empty() {
            Ok(ValidatedAppManifest {
                app,
                commands: command_index,
                events: event_index,
                tasks: task_index,
                resources: resource_index,
                windows: window_index,
                roots: root_index,
                startup,
            })
        } else {
            Err(ManifestValidationError { issues })
        }
    }
}

/// An immutable manifest whose cross-descriptor references have been validated.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedAppManifest {
    app: AppDescriptor,
    commands: BTreeMap<CommandName, CommandDescriptor>,
    events: BTreeMap<EventName, EventDescriptor>,
    tasks: BTreeMap<TaskIntentName, TaskDescriptor>,
    resources: BTreeMap<ResourceId, ResourceDescriptor>,
    windows: BTreeMap<WindowDescriptorId, WindowDescriptor>,
    roots: BTreeMap<RootId, RootDescriptor>,
    startup: Vec<StartupWindow>,
}

impl ValidatedAppManifest {
    /// Returns the descriptor for the application that owns this manifest.
    #[must_use]
    pub fn app(&self) -> &AppDescriptor {
        &self.app
    }

    /// Finds a command descriptor by its validated name.
    #[must_use]
    pub fn command(&self, name: &CommandName) -> Option<&CommandDescriptor> {
        self.commands.get(name)
    }

    /// Iterates commands in ascending command-name order.
    pub fn commands(&self) -> impl Iterator<Item = &CommandDescriptor> {
        self.commands.values()
    }

    /// Finds an event descriptor by its validated name.
    #[must_use]
    pub fn event(&self, name: &EventName) -> Option<&EventDescriptor> {
        self.events.get(name)
    }

    /// Iterates events in ascending event-name order.
    pub fn events(&self) -> impl Iterator<Item = &EventDescriptor> {
        self.events.values()
    }

    /// Finds a task descriptor by its intent name.
    #[must_use]
    pub fn task(&self, name: &TaskIntentName) -> Option<&TaskDescriptor> {
        self.tasks.get(name)
    }

    /// Iterates tasks in ascending intent-name order.
    pub fn tasks(&self) -> impl Iterator<Item = &TaskDescriptor> {
        self.tasks.values()
    }

    /// Finds a resource descriptor by its identifier.
    #[must_use]
    pub fn resource(&self, id: &ResourceId) -> Option<&ResourceDescriptor> {
        self.resources.get(id)
    }

    /// Iterates resources in ascending identifier order.
    pub fn resources(&self) -> impl Iterator<Item = &ResourceDescriptor> {
        self.resources.values()
    }

    /// Finds a window descriptor by its identifier.
    #[must_use]
    pub fn window(&self, id: &WindowDescriptorId) -> Option<&WindowDescriptor> {
        self.windows.get(id)
    }

    /// Iterates windows in ascending identifier order.
    pub fn windows(&self) -> impl Iterator<Item = &WindowDescriptor> {
        self.windows.values()
    }

    /// Finds a root descriptor by its identifier.
    #[must_use]
    pub fn root(&self, id: &RootId) -> Option<&RootDescriptor> {
        self.roots.get(id)
    }

    /// Iterates roots in ascending identifier order.
    pub fn roots(&self) -> impl Iterator<Item = &RootDescriptor> {
        self.roots.values()
    }

    /// Iterates startup windows by window, root, then scope.
    pub fn startup_windows(&self) -> impl Iterator<Item = &StartupWindow> {
        self.startup.iter()
    }
}

/// The kind of manifest relationship that validation rejected.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ManifestValidationErrorCode {
    /// A command name appears more than once.
    DuplicateCommand,
    /// An event name appears more than once.
    DuplicateEvent,
    /// A task intent name appears more than once.
    DuplicateTask,
    /// A resource identifier appears more than once.
    DuplicateResource,
    /// A window identifier appears more than once.
    DuplicateWindow,
    /// A root identifier appears more than once.
    DuplicateRoot,
    /// A root declares the same snapshot binding identifier more than once.
    DuplicateRootSnapshotBinding,
    /// A root requires a command absent from the manifest.
    MissingCommand,
    /// A root emits an event absent from the manifest.
    MissingEvent,
    /// A root command payload type differs from the manifest declaration.
    CommandPayloadTypeMismatch,
    /// A root event payload type differs from the manifest declaration.
    EventPayloadTypeMismatch,
    /// A startup entry names a window absent from the manifest.
    UnknownStartupWindow,
    /// A startup entry names a root absent from the manifest.
    UnknownStartupRoot,
    /// A startup root is outside a window's non-empty allowed-root list.
    DisallowedStartupRoot,
    /// Windows are declared without any startup entry.
    MissingStartupRoot,
}

/// One semantic issue found while validating an authored manifest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManifestValidationIssue {
    code: ManifestValidationErrorCode,
    root_id: Option<RootId>,
    window_id: Option<WindowDescriptorId>,
    command_name: Option<CommandName>,
    event_name: Option<EventName>,
    snapshot_binding_id: Option<SnapshotBindingId>,
    expected_payload_type: Option<PayloadTypeName>,
    actual_payload_type: Option<PayloadTypeName>,
}

impl ManifestValidationIssue {
    fn new(code: ManifestValidationErrorCode) -> Self {
        Self {
            code,
            root_id: None,
            window_id: None,
            command_name: None,
            event_name: None,
            snapshot_binding_id: None,
            expected_payload_type: None,
            actual_payload_type: None,
        }
    }

    fn with_root_id(mut self, root_id: RootId) -> Self {
        self.root_id = Some(root_id);
        self
    }

    fn with_window_id(mut self, window_id: WindowDescriptorId) -> Self {
        self.window_id = Some(window_id);
        self
    }

    fn with_command_name(mut self, command_name: CommandName) -> Self {
        self.command_name = Some(command_name);
        self
    }

    fn with_event_name(mut self, event_name: EventName) -> Self {
        self.event_name = Some(event_name);
        self
    }

    fn with_snapshot_binding_id(mut self, snapshot_binding_id: SnapshotBindingId) -> Self {
        self.snapshot_binding_id = Some(snapshot_binding_id);
        self
    }

    fn with_expected_payload_type(mut self, expected_payload_type: PayloadTypeName) -> Self {
        self.expected_payload_type = Some(expected_payload_type);
        self
    }

    fn with_actual_payload_type(mut self, actual_payload_type: PayloadTypeName) -> Self {
        self.actual_payload_type = Some(actual_payload_type);
        self
    }

    /// Returns the rejected relationship kind.
    #[must_use]
    pub const fn code(&self) -> ManifestValidationErrorCode {
        self.code
    }

    /// Returns the root involved in this issue, when applicable.
    #[must_use]
    pub fn root_id(&self) -> Option<&RootId> {
        self.root_id.as_ref()
    }

    /// Returns the window involved in this issue, when applicable.
    #[must_use]
    pub fn window_id(&self) -> Option<&WindowDescriptorId> {
        self.window_id.as_ref()
    }

    /// Returns the command involved in this issue, when applicable.
    #[must_use]
    pub fn command_name(&self) -> Option<&CommandName> {
        self.command_name.as_ref()
    }

    /// Returns the event involved in this issue, when applicable.
    #[must_use]
    pub fn event_name(&self) -> Option<&EventName> {
        self.event_name.as_ref()
    }

    /// Returns the snapshot binding involved in this issue, when applicable.
    #[must_use]
    pub fn snapshot_binding_id(&self) -> Option<&SnapshotBindingId> {
        self.snapshot_binding_id.as_ref()
    }

    /// Returns the declared payload type expected by this issue, when applicable.
    #[must_use]
    pub fn expected_payload_type(&self) -> Option<&PayloadTypeName> {
        self.expected_payload_type.as_ref()
    }

    /// Returns the root payload type observed by this issue, when applicable.
    #[must_use]
    pub fn actual_payload_type(&self) -> Option<&PayloadTypeName> {
        self.actual_payload_type.as_ref()
    }
}

/// All validation issues found while consuming an authored manifest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManifestValidationError {
    issues: Vec<ManifestValidationIssue>,
}

impl ManifestValidationError {
    /// Returns every validation issue in deterministic descriptor order.
    #[must_use]
    pub fn issues(&self) -> &[ManifestValidationIssue] {
        &self.issues
    }
}

impl fmt::Display for ManifestValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "manifest validation failed with {} issue(s)",
            self.issues.len()
        )
    }
}

impl Error for ManifestValidationError {}
