//! Simple in-memory todo list used by the `todo-cli` example.

/// Represents a single task within the list.
#[derive(Clone)]
pub struct Task {
    /// Description supplied by the user.
    pub description: String,
    /// Indicates whether the task has been completed.
    pub done: bool,
}

/// Collection of tasks with basic management operations.
#[derive(Default)]
pub struct TodoList {
    tasks: Vec<Task>,
}

impl TodoList {
    /// Create a new, empty todo list.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a task with the given description.
    pub fn add(&mut self, description: String) {
        self.tasks.push(Task {
            description,
            done: false,
        });
    }

    /// Mark the first task matching `description` as complete.
    ///
    /// Returns `true` when a matching task is found.
    pub fn complete(&mut self, description: &str) -> bool {
        if let Some(task) = self.tasks.iter_mut().find(|t| t.description == description) {
            task.done = true;
            true
        } else {
            false
        }
    }

    /// Render the list into a user-facing string.
    #[must_use]
    pub fn display(&self) -> String {
        self.tasks
            .iter()
            .enumerate()
            .map(|(i, t)| {
                format!(
                    "{}. [{}] {}",
                    i + 1,
                    if t.done { "x" } else { " " },
                    t.description
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Return task descriptions and completion state for verification.
    #[must_use]
    pub fn statuses(&self) -> Vec<(String, bool)> {
        self.tasks
            .iter()
            .map(|t| (t.description.clone(), t.done))
            .collect()
    }

    /// Check whether the list is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tasks.is_empty()
    }
}
