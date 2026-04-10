//! Notes application demonstrating utility helpers working together.
//!
//! Demonstrates:
//! - `util::Route` for stack-based view navigation (/list, /edit)
//! - `util::UndoStack` for reversible edits with labels
//! - `util::Selection` for multi-select with toggle
//! - View routing based on current route
//! - Complex state management in a single model
//!
//! Run with: `cargo run -p plushie --example notes`

use plushie::prelude::*;
use plushie::util::{Query, Route, Selection, SelectionMode, UndoCommand, UndoStack};

#[derive(Clone)]
struct NoteContent {
    title: String,
    body: String,
}

#[derive(Clone)]
struct Note {
    id: usize,
    title: String,
    body: String,
}

struct Notes {
    notes: Vec<Note>,
    next_id: usize,
    search_query: String,
    editing_id: Option<usize>,
    selection: Selection,
    undo: UndoStack<NoteContent>,
    route: Route,
}

impl App for Notes {
    type Model = Self;

    fn init() -> (Self, Command) {
        (
            Notes {
                notes: Vec::new(),
                next_id: 1,
                search_query: String::new(),
                editing_id: None,
                selection: Selection::new(SelectionMode::Multi, Vec::new()),
                undo: UndoStack::new(NoteContent {
                    title: String::new(),
                    body: String::new(),
                }),
                route: Route::new("/list"),
            },
            Command::none(),
        )
    }

    fn update(model: &mut Self, event: Event) -> Command {
        match event.widget_match() {
            Some(Click("new_note")) => {
                let id = model.next_id;
                model.next_id += 1;
                model.notes.push(Note {
                    id,
                    title: String::new(),
                    body: String::new(),
                });
                model.editing_id = Some(id);
                model.undo = UndoStack::new(NoteContent {
                    title: String::new(),
                    body: String::new(),
                });
                model.route.push("/edit");
                update_selection_order(model);
            }

            Some(Click(id)) if id.starts_with("note:") => {
                if let Ok(note_id) = id[5..].parse::<usize>() {
                    if let Some(note) = model.notes.iter().find(|n| n.id == note_id) {
                        model.editing_id = Some(note_id);
                        model.undo = UndoStack::new(NoteContent {
                            title: note.title.clone(),
                            body: note.body.clone(),
                        });
                        model.route.push("/edit");
                    }
                }
            }

            Some(Click("back")) => {
                save_current_edit(model);
                model.editing_id = None;
                model.route.pop();
            }

            Some(Click("delete_selected")) => {
                let selected = model.selection.selected().clone();
                model.notes.retain(|n| !selected.contains(&n.id.to_string()));
                model.selection.clear();
                update_selection_order(model);
            }

            Some(Click("undo")) => { model.undo.undo(); }
            Some(Click("redo")) => { model.undo.redo(); }

            Some(Input("search", query)) => {
                model.search_query = query.to_string();
            }

            Some(Input("title", value)) => {
                let title = value.to_string();
                model.undo.apply(
                    UndoCommand::new(
                        move |c: &NoteContent| NoteContent { title: title.clone(), body: c.body.clone() },
                        |c: &NoteContent| c.clone(),
                    )
                    .label("edit title")
                    .coalesce("typing", 500),
                );
            }

            Some(Input("body", value)) => {
                let body = value.to_string();
                model.undo.apply(
                    UndoCommand::new(
                        move |c: &NoteContent| NoteContent { title: c.title.clone(), body: body.clone() },
                        |c: &NoteContent| c.clone(),
                    )
                    .label("edit body")
                    .coalesce("typing", 500),
                );
            }

            Some(Toggle(id, _)) if id.starts_with("note_select:") => {
                let note_id = &id["note_select:".len()..];
                model.selection.toggle(note_id);
            }

            _ => {}
        }
        Command::none()
    }

    fn view(model: &Self) -> View {
        match model.route.current() {
            "/list" => view_list(model),
            "/edit" => view_edit(model),
            _ => view_list(model),
        }
    }
}

fn view_list(model: &Notes) -> View {
    let q = model.search_query.to_lowercase();
    let result = Query::new(&model.notes)
        .filter(|note| {
            q.is_empty()
                || note.title.to_lowercase().contains(&q)
                || note.body.to_lowercase().contains(&q)
        })
        .page_size(model.notes.len().max(1))
        .run();

    let mut note_list = column().spacing(4.0).width(Fill);
    for note in &result.entries {
        let id_str = note.id.to_string();
        let label = if note.title.is_empty() { "(untitled)" } else { &note.title };
        note_list = note_list.child(
            row().id(&id_str).spacing(8.0).width(Fill)
                .child(
                    checkbox(&format!("note_select:{}", note.id),
                        model.selection.is_selected(&id_str))
                        .label(label),
                )
                .child(button(&format!("note:{}", note.id), "Edit")),
        );
    }

    window("main").title("Notes").child(
        column().padding(16).spacing(12.0).width(Fill)
            .child(text("Notes").id("heading").size(24.0))
            .child(text_input("search", &model.search_query)
                .placeholder("Search notes..."))
            .child(
                scrollable().id("notes_list").height(Fill).child(note_list),
            )
            .child(row().spacing(8.0)
                .child(button("new_note", "New Note"))
                .child(button("delete_selected", "Delete Selected"))),
    ).into()
}

fn view_edit(model: &Notes) -> View {
    let current = model.undo.current();

    window("main").title("Edit Note").child(
        column().padding(16).spacing(12.0).width(Fill)
            .child(row().spacing(8.0)
                .child(button("back", "Back"))
                .child(button("undo", "Undo"))
                .child(button("redo", "Redo")))
            .child(text_input("title", &current.title)
                .placeholder("Note title"))
            .child(text_editor("body", &current.body)
                .placeholder("Write your note...")
                .width(Fill)
                .height(Fill)),
    ).into()
}

fn save_current_edit(model: &mut Notes) {
    if let Some(editing_id) = model.editing_id {
        let current = model.undo.current().clone();
        if let Some(note) = model.notes.iter_mut().find(|n| n.id == editing_id) {
            note.title = current.title;
            note.body = current.body;
        }
    }
}

fn update_selection_order(model: &mut Notes) {
    let order: Vec<String> = model.notes.iter().map(|n| n.id.to_string()).collect();
    model.selection = Selection::new(SelectionMode::Multi, order);
}

fn main() -> plushie::Result {
    plushie::run::<Notes>()
}
