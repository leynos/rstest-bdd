//! Behavioural tests for the `todo-cli` crate.

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use todo_cli::TodoList;

#[fixture]
fn todo_list() -> TodoList {
    TodoList::new()
}

#[derive(Debug)]
struct TaskEntries(Vec<String>);

impl TryFrom<Vec<Vec<String>>> for TaskEntries {
    type Error = String;

    fn try_from(rows: Vec<Vec<String>>) -> Result<Self, Self::Error> {
        let mut tasks = Vec::with_capacity(rows.len());
        for (index, row) in rows.into_iter().enumerate() {
            if row.len() != 1 {
                return Err(format!(
                    "datatable row {} must have exactly one column (task description); got: {:?}",
                    index + 1,
                    row
                ));
            }
            let mut cells = row.into_iter();
            let task = cells.next().expect("row.len() == 1 just asserted");
            tasks.push(task);
        }
        Ok(Self(tasks))
    }
}

impl IntoIterator for TaskEntries {
    type Item = String;
    type IntoIter = std::vec::IntoIter<String>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

#[derive(Debug)]
struct StatusEntries(Vec<(String, bool)>);

impl TryFrom<Vec<Vec<String>>> for StatusEntries {
    type Error = String;

    fn try_from(rows: Vec<Vec<String>>) -> Result<Self, Self::Error> {
        let mut entries = Vec::with_capacity(rows.len());
        for (index, row) in rows.into_iter().enumerate() {
            if row.len() < 2 {
                return Err(format!(
                    "datatable row {} must have two columns: <task> | <yes/no>",
                    index + 1
                ));
            }
            let mut cells = row.into_iter();
            let task = cells.next().expect("row.len() >= 2 just asserted");
            let done_cell = cells.next().expect("row.len() >= 2 just asserted");
            let done = matches!(
                done_cell.trim().to_ascii_lowercase().as_str(),
                "yes" | "y" | "true"
            );
            entries.push((task, done));
        }
        Ok(Self(entries))
    }
}

impl From<StatusEntries> for Vec<(String, bool)> {
    fn from(entries: StatusEntries) -> Self {
        entries.0
    }
}

#[given("an empty to-do list")]
fn empty_list(todo_list: &TodoList) {
    assert!(todo_list.is_empty(), "list should start empty");
}

#[when("I add the following tasks")]
#[expect(
    non_snake_case,
    reason = "match Gherkin capitalisation for inferred pattern"
)]
fn I_add_the_following_tasks(
    mut todo_list: TodoList,
    #[datatable] entries: TaskEntries,
) -> TodoList {
    for task in entries {
        todo_list.add(task);
    }
    todo_list
}

#[then]
fn the_list_displays(todo_list: &TodoList, docstring: String) {
    // Normalise docstring indentation to prevent false negatives.
    let expected = dedent(&docstring);
    assert_eq!(todo_list.display(), expected);
}

fn dedent(input: &str) -> String {
    // Normalise Windows line endings to LF to keep comparisons stable.
    let s = input.replace("\r\n", "\n");
    // Find minimum leading spaces/tabs across non-empty lines
    let mut min_indent: Option<usize> = None;
    for line in s.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let indent = line.chars().take_while(|c| *c == ' ' || *c == '\t').count();
        min_indent = Some(min_indent.map_or(indent, |m| m.min(indent)));
    }
    let cut = min_indent.unwrap_or(0);
    let out = s
        .lines()
        .map(|l| if l.len() >= cut { &l[cut..] } else { "" })
        .collect::<Vec<_>>()
        .join("\n");
    out.trim_matches('\n').to_string()
}

#[given("a to-do list with {first} and {second}")]
fn list_with_two(mut todo_list: TodoList, first: String, second: String) -> TodoList {
    todo_list.add(first);
    todo_list.add(second);
    todo_list
}

#[when("I complete {task}")]
fn complete_task(mut todo_list: TodoList, task: String) -> TodoList {
    let ok = todo_list.complete(&task);
    assert!(
        ok,
        "expected to complete task '{}'; tasks present: {:?}",
        task,
        todo_list.statuses()
    );
    todo_list
}

#[then]
fn the_task_statuses_should_be(todo_list: &TodoList, #[datatable] entries: StatusEntries) {
    let expected: Vec<(String, bool)> = entries.into();
    assert_eq!(todo_list.statuses(), expected);
}

#[scenario(path = "tests/features/add.feature")]
fn add_scenario(todo_list: TodoList) {
    assert!(todo_list.is_empty(), "scenario fixture should start empty");
}

#[scenario(path = "tests/features/complete.feature")]
fn complete_scenario(todo_list: TodoList) {
    assert!(todo_list.is_empty(), "scenario fixture should start empty");
}
