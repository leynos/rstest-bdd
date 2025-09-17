//! Simple in-memory to-do list used by the `todo-cli` example.

/// Represents a single task within the list.
#[derive(Clone, Debug)]
struct Task {
    /// Description supplied by the user.
    description: String,
    /// Indicates whether the task has been completed.
    done: bool,
}

/// Collection of tasks with basic management operations.
#[derive(Clone, Default)]
pub struct TodoList {
    tasks: Vec<Task>,
}

impl TodoList {
    /// Create a new, empty to-do list.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a task with the given description.
    pub fn add<S: Into<String>>(&mut self, description: S) {
        self.tasks.push(Task {
            description: description.into(),
            done: false,
        });
    }

    /// Mark the first task matching `description` as complete.
    ///
    /// Returns `true` when a matching task is found.
    #[must_use]
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
        let mut out = String::new();
        use std::fmt::Write as _;
        for (i, t) in self.tasks.iter().enumerate() {
            writeln!(
                out,
                "{}. [{}] {}",
                i + 1,
                if t.done { "x" } else { " " },
                t.description
            )
            .expect("writing to a String cannot fail");
        }
        if out.ends_with('\n') {
            out.pop();
        }
        out
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
