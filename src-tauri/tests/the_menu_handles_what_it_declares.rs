//! Every menu id the handler acts on has to be an id the menu offers.
//!
//! `cargo test -p cypcb-desktop --test the_menu_handles_what_it_declares`
//!
//! The first test this crate has ever had. It went uncompiled for long enough
//! to collect nine errors from the Tauri v1 to v2 move, so "does it work" was
//! never a question anybody could ask - and the viewer answers a version of
//! this one by reading `menu.rs` as text, because until 2026-08-12 nothing
//! here could compile it.
//!
//! Reading the real data model is the stronger half of the same check. A
//! regex over `action("...")` sees what the file says; this sees what
//! `create_app_menu()` returns, which is what the menu bar is actually built
//! from. The two disagree the moment somebody builds an id rather than writing
//! it as a literal.
//!
//! What it cannot see is the running app: a window that opens, a file through
//! the native picker, a menu click reaching the frontend. Those need a display
//! and are not made true by this passing.

use cypcb_platform::{MenuBar, MenuItem};

/// Every action id the menu bar offers, in declaration order.
fn declared_ids(bar: &MenuBar) -> Vec<String> {
    let mut ids = Vec::new();
    for menu in &bar.items {
        for item in &menu.items {
            if let MenuItem::Action { id, .. } = item {
                ids.push(id.clone());
            }
        }
    }
    ids
}

/// The ids `handle_menu_event` acts on itself instead of emitting.
///
/// Read out of the source, because the function needs an `AppHandle` and one
/// cannot be built without a running app. The list is short and the arms are
/// literals; a match arm that stopped being a literal would show up as this
/// list shrinking, which the assertions below would catch.
fn handled_natively() -> Vec<String> {
    let source = include_str!("../src/menu.rs");
    let body = &source[source
        .find("pub fn handle_menu_event")
        .expect("the handler is still called that")..];

    let mut ids = Vec::new();
    let mut rest = body;
    while let Some(at) = rest.find("\" => {") {
        let before = &rest[..at];
        if let Some(open) = before.rfind('"') {
            let id = &before[open + 1..];
            if id.contains('.') && !id.contains(' ') {
                ids.push(id.to_string());
            }
        }
        rest = &rest[at + 1..];
    }
    ids
}

#[test]
fn the_menu_offers_something_to_click() {
    let ids = declared_ids(&cypcb_desktop::menu::create_app_menu());
    assert!(
        ids.len() > 5,
        "a menu bar with {} items is not the one this app ships: {ids:?}",
        ids.len()
    );
    // Every id is `group.action`, which is what the frontend's switch and the
    // handler's match arms both assume.
    for id in &ids {
        assert!(
            id.contains('.') && !id.contains(' '),
            "{id} is not shaped like a menu id"
        );
    }
}

#[test]
fn no_two_menu_items_share_an_id() {
    // Two items with one id means one of them is unreachable: the frontend
    // switches on the id and cannot tell which was clicked.
    let ids = declared_ids(&cypcb_desktop::menu::create_app_menu());
    let mut sorted = ids.clone();
    sorted.sort();
    let before = sorted.len();
    sorted.dedup();
    assert_eq!(
        sorted.len(),
        before,
        "the menu declares an id twice: {ids:?}"
    );
}

#[test]
fn everything_the_handler_acts_on_is_in_the_menu() {
    // The defect this exists for. `handle_menu_event` matches "file.quit" and
    // "view.fullscreen" and emits everything else; an arm naming an id the
    // menu does not declare is dead code that looks like a feature, and a menu
    // item whose id the handler misspells is a control that does nothing.
    let declared = declared_ids(&cypcb_desktop::menu::create_app_menu());
    let handled = handled_natively();

    assert!(
        !handled.is_empty(),
        "no natively handled ids were found; the scan is broken rather than \
         the menu being empty"
    );
    for id in &handled {
        assert!(
            declared.contains(id),
            "handle_menu_event acts on `{id}`, which no menu item offers: \
             {declared:?}"
        );
    }
}
