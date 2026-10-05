//! Step 3.6: duplicating blocks with `D`.

use std::{fs, time::Duration};

use crate::support::{Harness, focused_rows, outline_rows};

/// The open file's text once the autosave delay has passed and its save is done.
fn saved(app: &mut Harness) -> String {
    app.wait(Duration::from_secs(1));
    app.settle();
    let path = app.path.as_ref().expect("a file was opened");
    fs::read_to_string(path).expect("the file is readable")
}

// FR-OPS-06, FR-OPS-08
#[test]
fn shift_d_duplicates_a_block_with_its_subtree_below_it_and_z_takes_it_back() {
    let mut app = Harness::with_text("Groceries:\n\t- Milk\n\t\tOat\n\t- Bread\n", 50, 10);
    app.type_keys("jD");
    assert_eq!(
        outline_rows(&mut app)[..5],
        ["Groceries:", "  - Milk", "    Oat", "  - Milk", "    Oat"]
    );
    assert_eq!(focused_rows(&mut app), ["  - Milk"]);
    app.type_keys("jj");
    assert_eq!(
        focused_rows(&mut app),
        ["  - Bread"],
        "the copy was focused"
    );
    app.type_keys("kk");
    assert_eq!(
        saved(&mut app),
        "Groceries:\n\t- Milk\n\t\tOat\n\t- Milk\n\t\tOat\n\t- Bread\n"
    );
    app.type_keys("z");
    assert_eq!(
        saved(&mut app),
        "Groceries:\n\t- Milk\n\t\tOat\n\t- Bread\n"
    );
}

// FR-OPS-06, FR-OPS-07
#[test]
fn shift_d_duplicates_every_selected_block() {
    let mut app = Harness::with_text("- a\n- b\n- c\n", 50, 10);
    app.type_keys("JD");
    assert_eq!(
        outline_rows(&mut app)[..5],
        ["- a", "- a", "- b", "- b", "- c"]
    );
}
