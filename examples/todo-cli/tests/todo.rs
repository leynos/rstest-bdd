//! Behavioural tests for the `todo-cli` crate.

use std::cell::RefCell;

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use todo_cli::TodoList;

#[fixture]
fn todo() -> RefCell<TodoList> {
    RefCell::new(TodoList::new())
}

#[given("an empty todo list")]
fn empty_list(#[from(todo)] list: &RefCell<TodoList>) {
    assert!(list.borrow().is_empty(), "list should start empty");
}

#[when("I add the following tasks")]
fn add_tasks(#[from(todo)] list: &RefCell<TodoList>, datatable: Vec<Vec<String>>) {
    for (i, row) in datatable.into_iter().enumerate() {
        assert!(
            !row.is_empty(),
            "datatable row {} must have at least one column (task description)",
            i + 1
        );
        list.borrow_mut().add(row[0].clone());
    }
}

#[then("the list displays")]
fn list_displays(#[from(todo)] list: &RefCell<TodoList>, docstring: String) {
    let normalised = docstring
        .trim()
        .lines()
        .map(str::trim_start)
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(list.borrow().display(), normalised);
}

#[given("a todo list with {first} and {second}")]
fn list_with_two(#[from(todo)] list: &RefCell<TodoList>, first: String, second: String) {
    let mut l = list.borrow_mut();
    l.add(first);
    l.add(second);
}

#[when("I complete {task}")]
fn complete_task(#[from(todo)] list: &RefCell<TodoList>, task: String) {
    let ok = list.borrow_mut().complete(&task);
    assert!(
        ok,
        "expected to complete task '{}'; tasks present: {:?}",
        task,
        list.borrow().statuses()
    );
}

#[then("the task statuses should be")]
fn assert_statuses(#[from(todo)] list: &RefCell<TodoList>, datatable: Vec<Vec<String>>) {
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
            let done = matches!(row[1].to_ascii_lowercase().as_str(), "yes" | "y" | "true");
            (task, done)
        })
        .collect();
    assert_eq!(list.borrow().statuses(), expected);
}

#[scenario(path = "tests/features/add.feature")]
fn add_scenario(todo: RefCell<TodoList>) {
    let _ = todo;
}

#[scenario(path = "tests/features/complete.feature")]
fn complete_scenario(todo: RefCell<TodoList>) {
    let _ = todo;
}
