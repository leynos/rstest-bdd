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
    for row in datatable {
        if let Some(task) = row.first() {
            list.borrow_mut().add(task.clone());
        }
    }
}

#[then("the list displays")]
fn list_displays(#[from(todo)] list: &RefCell<TodoList>, docstring: String) {
    assert_eq!(list.borrow().display(), docstring.trim());
}

#[given("a todo list with {first} and {second}")]
fn list_with_two(#[from(todo)] list: &RefCell<TodoList>, first: String, second: String) {
    let mut l = list.borrow_mut();
    l.add(first);
    l.add(second);
}

#[when("I complete {task}")]
fn complete_task(#[from(todo)] list: &RefCell<TodoList>, task: String) {
    assert!(list.borrow_mut().complete(&task), "task should exist");
}

#[then("the task statuses should be")]
fn assert_statuses(#[from(todo)] list: &RefCell<TodoList>, datatable: Vec<Vec<String>>) {
    let expected: Vec<(String, bool)> = datatable
        .into_iter()
        .filter_map(|row| {
            let task = row.first()?.clone();
            let done = matches!(row.get(1).map(String::as_str), Some("yes"));
            Some((task, done))
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
