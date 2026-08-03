use cursive::view::{Nameable, Resizable};
use cursive::views::{Dialog, DummyView, EditView, LinearLayout, SelectView, TextView};

use crate::palette::command::Command;
use crate::palette::registry::CommandRegistry;

pub fn create_command_palette_layer(registry: CommandRegistry) -> impl cursive::view::View {
    let submit_registry = registry.clone();
    let select_view = SelectView::<String>::new()
        .on_submit(move |siv, id: &str| {
            let id = id.to_string();
            submit_registry.execute(&id, siv);
            siv.pop_layer();
        })
        .with_name("palette_list");

    let edit_registry = registry;
    let search_input = EditView::new()
        .on_edit(move |siv, query, _cursor| {
            let results: Vec<(String, String)> = edit_registry
                .search(query)
                .into_iter()
                .map(|cmd| (display_name(cmd), cmd.id.clone()))
                .collect();
            siv.call_on_name("palette_list", |list: &mut SelectView<String>| {
                list.clear();
                for (display, id) in results {
                    list.add_item(display, id);
                }
                if !list.is_empty() {
                    list.set_selection(0);
                }
            });
        })
        .with_name("palette_search");

    let layout = LinearLayout::vertical()
        .child(TextView::new("Type to filter commands"))
        .child(search_input.fixed_width(60))
        .child(DummyView)
        .child(select_view);

    Dialog::around(layout)
        .title("Command Palette")
        .button("Close", |siv| {
            siv.pop_layer();
        })
        .with_name("command_palette")
}

fn display_name(cmd: &Command) -> String {
    match &cmd.shortcut {
        Some(shortcut) => format!("{}  ({:?})", cmd.name, shortcut),
        None => cmd.name.clone(),
    }
}
