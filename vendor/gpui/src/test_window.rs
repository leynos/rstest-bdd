//! Window and entity handles for the local GPUI test support shim.
//!
//! This module models the subset of GPUI test-window behaviour that the
//! rstest-bdd GPUI harness needs for stateful BDD scenarios. `TestAppContext`
//! owns a `WindowRegistry`, creates windows through `add_window_view`, and
//! hands step definitions durable `Entity<T>` and `AnyWindowHandle` values that
//! can be carried between generated scenario steps.
//!
//! `VisualTestContext` is the window-bound access point reconstructed from an
//! `AnyWindowHandle` when a later step needs to read or mutate a view. Entity
//! operations validate both the originating test context and the owning window,
//! so stale handles, foreign context handles, and cross-window entity access
//! fail instead of mutating unrelated test state.

use std::{
    any::Any,
    cell::{RefCell, RefMut},
    collections::HashMap,
    error::Error,
    fmt,
    marker::PhantomData,
    rc::Rc,
    sync::atomic::{AtomicU64, Ordering},
};

use crate::TestAppContext;

/// Allocates process-unique registry identities for rejecting foreign handles.
static NEXT_REGISTRY_ID: AtomicU64 = AtomicU64::new(1);

/// Owns the mutable window/entity state for one `TestAppContext`.
#[derive(Clone, Debug)]
pub(crate) struct WindowRegistry {
    /// Shared interior state so visual contexts retain access after construction.
    inner: Rc<RefCell<WindowRegistryState>>,
}

impl Default for WindowRegistry {
    fn default() -> Self {
        Self {
            inner: Rc::new(RefCell::new(WindowRegistryState::new())),
        }
    }
}

impl WindowRegistry {
    /// Allocate a window/entity pair before invoking the view constructor.
    ///
    /// Reserving both identifiers first lets the constructor retain a visual
    /// context that can address the newly created window.
    pub(crate) fn add_window_view<T>(
        &self,
        build_view: impl FnOnce(&mut VisualTestContext) -> T,
    ) -> (Entity<T>, VisualTestContext)
    where
        T: 'static,
    {
        let (window, entity) = {
            let mut state = self.inner.borrow_mut();
            state.next_window_id += 1;
            state.next_entity_id += 1;

            let window = AnyWindowHandle {
                registry_id: state.registry_id,
                id: state.next_window_id,
            };
            let entity = Entity {
                registry_id: state.registry_id,
                id: state.next_entity_id,
                _marker: PhantomData,
            };
            state.windows.push(window);
            (window, entity)
        };

        let mut visual_context = VisualTestContext::new(window, Rc::clone(&self.inner));
        let view = build_view(&mut visual_context);
        self.insert_entity(window, entity, view);

        (entity, visual_context)
    }

    /// Return a snapshot of handles owned by this registry.
    pub(crate) fn handles(&self) -> Vec<AnyWindowHandle> { self.inner.borrow().windows.clone() }

    /// Report whether a handle belongs to this registry's window set.
    pub(crate) fn contains(&self, window: AnyWindowHandle) -> bool {
        self.inner.borrow().windows.contains(&window)
    }

    /// Store a view under the window and typed handle allocated for it.
    ///
    /// Callers must have allocated both values from this registry; later reads
    /// verify that ownership before downcasting the erased value.
    fn insert_entity<T>(&self, window: AnyWindowHandle, entity: Entity<T>, view: T)
    where
        T: 'static,
    {
        self.inner.borrow_mut().entities.insert(
            entity.id,
            StoredEntity {
                window,
                value: Box::new(view),
            },
        );
    }
}

/// Type-erased entity storage paired with its owning window.
#[derive(Debug)]
struct StoredEntity {
    /// Window permitted to access this value.
    window: AnyWindowHandle,
    /// Runtime-erased view value, downcast only through a matching `Entity<T>`.
    value: Box<dyn Any>,
}

/// Mutable bookkeeping for the windows and entities of one test context.
#[derive(Debug)]
struct WindowRegistryState {
    /// Process-unique provenance used to reject foreign handles.
    registry_id: u64,
    /// Last entity identity issued by this registry.
    next_entity_id: u64,
    /// Last window identity issued by this registry.
    next_window_id: u64,
    /// Handles returned to callers in creation order.
    windows: Vec<AnyWindowHandle>,
    /// Type-erased entity values indexed by their stable identity.
    entities: HashMap<u64, StoredEntity>,
}

impl WindowRegistryState {
    /// Initialize empty state with a unique registry provenance identity.
    fn new() -> Self {
        Self {
            registry_id: NEXT_REGISTRY_ID.fetch_add(1, Ordering::Relaxed),
            next_entity_id: 0,
            next_window_id: 0,
            windows: Vec::new(),
            entities: HashMap::new(),
        }
    }
}

/// Durable typed handle for an entity stored in a GPUI test window.
#[derive(Debug)]
pub struct Entity<T> {
    /// Registry that created this handle and therefore owns its entity value.
    registry_id: u64,
    /// Stable identity used as the entity-store key.
    id: u64,
    /// Retains the entity's compile-time type without storing a `T` value.
    _marker: PhantomData<fn() -> T>,
}

impl<T> Entity<T> {
    /// Returns the stable identifier backing this test handle.
    #[must_use]
    pub const fn id(&self) -> u64 { self.id }
}

impl<T> Clone for Entity<T> {
    fn clone(&self) -> Self { *self }
}

impl<T> Copy for Entity<T> {}

/// Type-erased durable handle for a GPUI test window.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AnyWindowHandle {
    /// Registry that created this handle and therefore owns its window.
    registry_id: u64,
    /// Stable identity used to distinguish windows within the registry.
    id: u64,
}

impl AnyWindowHandle {
    /// Returns the stable identifier backing this test window handle.
    #[must_use]
    pub const fn id(&self) -> u64 { self.id }
}

/// Error returned when a visual-context entity operation cannot find a handle.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EntityError {
    /// The entity handle does not identify an entity of the expected type.
    NotFound {
        /// Stable identifier from the handle that could not be resolved.
        id: u64,
    },
}

impl fmt::Display for EntityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound { id } => write!(
                formatter,
                "entity handle {id} did not identify an entity of the expected type"
            ),
        }
    }
}

impl Error for EntityError {}

/// Window-bound visual context reconstructed during GPUI tests.
#[derive(Clone, Debug)]
pub struct VisualTestContext {
    /// Window to which all reads and writes through this context are confined.
    window: AnyWindowHandle,
    /// Shared state that validates registry and window ownership before access.
    registry: Rc<RefCell<WindowRegistryState>>,
}

impl VisualTestContext {
    /// Reconstructs a visual context from a durable window handle.
    #[must_use]
    pub fn from_window(window: AnyWindowHandle, context: &mut TestAppContext) -> Option<Self> {
        context
            .windows
            .contains(window)
            .then(|| Self::new(window, Rc::clone(&context.windows.inner)))
    }

    /// Returns the durable handle for this visual context's window.
    #[must_use]
    pub const fn window_handle(&self) -> AnyWindowHandle { self.window }

    /// Mutates an entity when the handle identifies a value of the expected type.
    ///
    /// # Errors
    ///
    /// Returns [`EntityError::NotFound`] when the handle comes from another
    /// registry or window, or when it identifies a different concrete type.
    pub fn update_entity<T>(
        &mut self,
        entity: Entity<T>,
        update: impl FnOnce(&mut T),
    ) -> Result<(), EntityError>
    where
        T: 'static,
    {
        let Some(mut value) = self.entity_mut(entity) else {
            return Err(EntityError::NotFound { id: entity.id });
        };
        update(&mut value);
        Ok(())
    }

    /// Reads an entity when the handle identifies a value of the expected type.
    #[must_use]
    pub fn read_entity<T, R>(&self, entity: Entity<T>, read: impl FnOnce(&T) -> R) -> Option<R>
    where
        T: 'static,
    {
        let registry = self.registry.borrow();
        registry
            .entities
            .get(&entity.id)
            .filter(|stored_entity| {
                entity.registry_id == registry.registry_id && stored_entity.window == self.window
            })
            .and_then(|stored_entity| stored_entity.value.downcast_ref::<T>())
            .map(read)
    }

    /// Bind a visual context to one known window and its registry state.
    fn new(window: AnyWindowHandle, registry: Rc<RefCell<WindowRegistryState>>) -> Self {
        Self { window, registry }
    }

    /// Borrow a typed entity only when its registry and window match this context.
    fn entity_mut<T>(&mut self, entity: Entity<T>) -> Option<RefMut<'_, T>>
    where
        T: 'static,
    {
        let window = self.window;
        RefMut::filter_map(self.registry.borrow_mut(), |registry| {
            if entity.registry_id != registry.registry_id {
                return None;
            }
            registry
                .entities
                .get_mut(&entity.id)
                .filter(|stored_entity| stored_entity.window == window)
                .and_then(|stored_entity| stored_entity.value.downcast_mut::<T>())
        })
        .ok()
    }
}
