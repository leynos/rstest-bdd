//! Behavioural tests for the `todo-cli` crate.

use std::cell::RefCell;

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use todo_cli::TodoList;

#[fixture]
fn todo_list() -> RefCell<TodoList> {
    RefCell::new(TodoList::new())
}

#[given("an empty to-do list")]
fn empty_list(todo_list: &RefCell<TodoList>) {
    assert!(todo_list.borrow().is_empty(), "list should start empty");
}

#[when("I add the following tasks")]
fn add_tasks(todo_list: &RefCell<TodoList>, datatable: Vec<Vec<String>>) {
    for (i, row) in datatable.into_iter().enumerate() {
        assert_eq!(
            row.len(),
            1,
            "datatable row {} must have exactly one column (task description); got: {:?}",
            i + 1,
            row
        );
        let task = row
            .into_iter()
            .next()
            .expect("row.len() == 1 just asserted");
        todo_list.borrow_mut().add(task);
    }
}

#[then("the list displays")]
fn list_displays(todo_list: &RefCell<TodoList>, docstring: String) {
    // Normalise docstring indentation to prevent false negatives.
    let expected = dedent(&docstring);
    assert_eq!(todo_list.borrow().display(), expected);
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
fn list_with_two(todo_list: &RefCell<TodoList>, first: String, second: String) {
    let mut l = todo_list.borrow_mut();
    l.add(first);
    l.add(second);
}

#[when("I complete {task}")]
fn complete_task(todo_list: &RefCell<TodoList>, task: String) {
    let ok = todo_list.borrow_mut().complete(&task);
    assert!(
        ok,
        "expected to complete task '{}'; tasks present: {:?}",
        task,
        todo_list.borrow().statuses()
    );
}

#[then("the task statuses should be")]
fn assert_statuses(todo_list: &RefCell<TodoList>, datatable: Vec<Vec<String>>) {
    let expected: Vec<(String, bool)> = datatable
        .into_iter()
        .enumerate()
        .map(|(i, row)| {
            assert!(
                row.len() >= 2,
                "datatable row {} must have two columns: <task> | <yes/no>",
                i + 1
            );
            let task = row[0].clone();
            let done = matches!(
                row[1].trim().to_ascii_lowercase().as_str(),
                "yes" | "y" | "true"
            );
            (task, done)
        })
        .collect();
    assert_eq!(todo_list.borrow().statuses(), expected);
}

#[allow(unused_variables)]
#[scenario(path = "tests/features/add.feature")]
fn add_scenario(todo_list: RefCell<TodoList>) {
    // Parameter triggers the `todo_list` fixture; no additional setup required.
}

#[allow(unused_variables)]
#[scenario(path = "tests/features/complete.feature")]
fn complete_scenario(todo_list: RefCell<TodoList>) {
    // Parameter triggers the `todo_list` fixture; no additional setup required.
}
