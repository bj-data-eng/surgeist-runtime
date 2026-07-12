use super::{
    AppId, AppScope, CommandDescriptor, EventDescriptor, NameError, PayloadTypeName, ResourceId,
    RootId, SnapshotBinding, TaskIntentName,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct App {
    descriptor: AppDescriptor,
}

impl App {
    #[must_use]
    pub fn new(descriptor: AppDescriptor) -> Self {
        Self { descriptor }
    }

    #[must_use]
    pub fn descriptor(&self) -> &AppDescriptor {
        &self.descriptor
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
}
