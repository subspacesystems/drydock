// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m10-tags-assign: assign the selected groups and components to a tag from the Tags panel
//! and from Entity Info, driven with egui_kittest through a real `Editor`.
//! See docs/specs/m10-tags-assign.md.

use drydock::editor::Editor;
use drydock::entity_info::*;
use drydock::input::EditKey;
use drydock::tags_panel::*;
use drydock_kernel::component::make_component;
use drydock_kernel::model::*;
use drydock_kernel::select::Entity;
use drydock_kernel::tags::TagId;
use eframe::egui;
use eframe::egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use glam::DVec3;

fn v(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

fn polygon(model: &mut Model, pts: &[DVec3]) {
    let mut log = ModelEdit::new();
    model
        .edit_geometry(BodyId::Root, &mut log, |ctx, edit| {
            for i in 0..pts.len() {
                ctx.add_edge(pts[i], pts[(i + 1) % pts.len()], edit)
                    .unwrap();
            }
        })
        .unwrap();
}

fn rect(x0: f64) -> Vec<DVec3> {
    vec![
        v(x0, 0.0, 0.0),
        v(x0 + 1.0, 0.0, 0.0),
        v(x0 + 1.0, 1.0, 0.0),
        v(x0, 1.0, 0.0),
    ]
}

fn root_edges_and_faces(model: &Model) -> Vec<Entity> {
    let geometry = model.body(BodyId::Root).unwrap().geometry();
    let mut out: Vec<Entity> = geometry.edges().map(|(id, _)| Entity::Edge(id)).collect();
    out.extend(geometry.faces().map(|(id, _)| Entity::Face(id)));
    out.sort();
    out
}

/// The rectangle at x..x+1 made into a component or group named `name`.
fn add_instance(model: &mut Model, x: f64, name: &str, group: bool) -> InstanceId {
    polygon(model, &rect(x));
    let selection = root_edges_and_faces(model);
    make_component(
        model,
        BodyId::Root,
        &selection,
        DefinitionInfo {
            name: name.to_string(),
            group,
        },
        &mut UndoStack::new(),
        "Make Component",
    )
    .unwrap()
}

struct Scene {
    model: Model,
    /// A component, a group, and a second component.
    a: InstanceId,
    b: InstanceId,
    c: InstanceId,
    walls: TagId,
    roof: TagId,
    /// A loose edge and face drawn at x 10.
    loose: Vec<Entity>,
}

fn scene() -> Scene {
    let mut model = Model::new();
    let a = add_instance(&mut model, 0.0, "A", false);
    let b = add_instance(&mut model, 3.0, "B", true);
    let c = add_instance(&mut model, 6.0, "C", false);
    polygon(&mut model, &rect(10.0));
    let loose = root_edges_and_faces(&model);
    let mut log = ModelEdit::new();
    let walls = model.add_tag("Walls", &mut log).unwrap();
    let roof = model.add_tag("Roof", &mut log).unwrap();
    Scene {
        model,
        a,
        b,
        c,
        walls,
        roof,
        loose,
    }
}

fn tag_of(model: &Model, instance: InstanceId) -> Option<TagId> {
    model.instance_tag(instance)
}

fn steps(editor: &Editor) -> usize {
    editor.undo_stack().undo_count()
}

// --- the editor operation ---

#[test]
fn assigns_every_selected_instance_in_one_step_and_ignores_edges_and_faces() {
    let mut s = scene();
    let mut editor = Editor::new();
    let mut selection = s.loose.clone();
    selection.extend([Entity::Instance(s.a), Entity::Instance(s.b)]);
    editor.select(selection);
    assert!(editor.assign_selection_tag(&mut s.model, Some(s.walls)));
    assert_eq!(tag_of(&s.model, s.a), Some(s.walls));
    assert_eq!(tag_of(&s.model, s.b), Some(s.walls));
    assert_eq!(tag_of(&s.model, s.c), None, "not selected");
    assert_eq!(steps(&editor), 1);
    assert_eq!(editor.undo_stack().undo_label(), Some("Assign Tag"));
    assert_eq!(
        editor.selection().len(),
        s.loose.len() + 2,
        "the selection is kept"
    );

    assert!(editor.key(EditKey::Undo, &mut s.model));
    assert_eq!(tag_of(&s.model, s.a), None);
    assert_eq!(tag_of(&s.model, s.b), None);
    assert_eq!(steps(&editor), 0);
    assert!(editor.key(EditKey::Redo, &mut s.model));
    assert_eq!(tag_of(&s.model, s.a), Some(s.walls));
    assert_eq!(tag_of(&s.model, s.b), Some(s.walls));
}

#[test]
fn none_untags_in_one_step() {
    let mut s = scene();
    let mut editor = Editor::new();
    editor.select(vec![Entity::Instance(s.a), Entity::Instance(s.b)]);
    assert!(editor.assign_selection_tag(&mut s.model, Some(s.walls)));
    assert!(editor.assign_selection_tag(&mut s.model, None));
    assert_eq!(tag_of(&s.model, s.a), None);
    assert_eq!(tag_of(&s.model, s.b), None);
    assert_eq!(steps(&editor), 2);
    assert_eq!(editor.undo_stack().undo_label(), Some("Remove Tag"));
    assert!(editor.key(EditKey::Undo, &mut s.model));
    assert_eq!(tag_of(&s.model, s.a), Some(s.walls));
    assert_eq!(tag_of(&s.model, s.b), Some(s.walls));
}

#[test]
fn mixed_tags_all_move_to_the_new_tag_in_one_step() {
    let mut s = scene();
    let mut editor = Editor::new();
    editor.select(vec![Entity::Instance(s.a)]);
    editor.assign_selection_tag(&mut s.model, Some(s.walls));
    editor.select(vec![
        Entity::Instance(s.a),
        Entity::Instance(s.b),
        Entity::Instance(s.c),
    ]);
    let before = steps(&editor);
    assert!(editor.assign_selection_tag(&mut s.model, Some(s.roof)));
    for i in [s.a, s.b, s.c] {
        assert_eq!(tag_of(&s.model, i), Some(s.roof));
    }
    assert_eq!(steps(&editor), before + 1);
    assert!(editor.key(EditKey::Undo, &mut s.model));
    assert_eq!(tag_of(&s.model, s.a), Some(s.walls));
    assert_eq!(tag_of(&s.model, s.b), None);
    assert_eq!(tag_of(&s.model, s.c), None);
}

#[test]
fn a_partly_tagged_selection_changes_only_what_differs() {
    let mut s = scene();
    let mut editor = Editor::new();
    editor.select(vec![Entity::Instance(s.a)]);
    editor.assign_selection_tag(&mut s.model, Some(s.walls));
    editor.select(vec![Entity::Instance(s.a), Entity::Instance(s.b)]);
    let before = steps(&editor);
    assert!(editor.assign_selection_tag(&mut s.model, Some(s.walls)));
    assert_eq!(tag_of(&s.model, s.b), Some(s.walls));
    assert_eq!(steps(&editor), before + 1);
}

#[test]
fn nothing_to_change_makes_no_step() {
    let mut s = scene();
    let mut editor = Editor::new();
    editor.select(vec![Entity::Instance(s.a)]);
    assert!(
        !editor.assign_selection_tag(&mut s.model, None),
        "already untagged"
    );
    assert_eq!(steps(&editor), 0);
    assert!(editor.assign_selection_tag(&mut s.model, Some(s.walls)));
    assert!(
        !editor.assign_selection_tag(&mut s.model, Some(s.walls)),
        "already walls"
    );
    assert_eq!(steps(&editor), 1);
}

#[test]
fn no_instance_selected_does_nothing() {
    let mut s = scene();
    let mut editor = Editor::new();
    assert!(!editor.assign_selection_tag(&mut s.model, Some(s.walls)));
    editor.select(s.loose.clone());
    assert!(!editor.assign_selection_tag(&mut s.model, Some(s.walls)));
    assert_eq!(steps(&editor), 0);
    for i in [s.a, s.b, s.c] {
        assert_eq!(tag_of(&s.model, i), None);
    }
}

#[test]
fn an_unknown_tag_is_refused_with_a_message() {
    let mut s = scene();
    let mut editor = Editor::new();
    let mut log = ModelEdit::new();
    s.model.remove_tag(s.roof, &mut log).unwrap();
    editor.select(vec![Entity::Instance(s.a)]);
    assert!(!editor.assign_selection_tag(&mut s.model, Some(s.roof)));
    assert!(editor.message().is_some());
    assert_eq!(steps(&editor), 0);
    assert_eq!(tag_of(&s.model, s.a), None);
}

// --- the shared tag of a selection ---

#[test]
fn selection_tag_is_none_same_or_varies() {
    let mut s = scene();
    let root = BodyId::Root;
    let none = selection_tag(&s.model, root, &s.loose);
    assert_eq!(none, None, "no instance");
    assert_eq!(selection_tag(&s.model, root, &[]), None);

    let one = [Entity::Instance(s.a)];
    assert_eq!(
        selection_tag(&s.model, root, &one),
        Some(Shared::Same(None))
    );

    let mut log = ModelEdit::new();
    s.model.assign_tag(s.a, Some(s.walls), &mut log).unwrap();
    assert_eq!(
        selection_tag(&s.model, root, &one),
        Some(Shared::Same(Some(s.walls)))
    );
    let two = [Entity::Instance(s.a), Entity::Instance(s.b)];
    assert_eq!(selection_tag(&s.model, root, &two), Some(Shared::Varies));
    s.model.assign_tag(s.b, Some(s.walls), &mut log).unwrap();
    assert_eq!(
        selection_tag(&s.model, root, &two),
        Some(Shared::Same(Some(s.walls)))
    );

    // Edges and faces are ignored.
    let mut with_loose = s.loose.clone();
    with_loose.extend(two);
    assert_eq!(
        selection_tag(&s.model, root, &with_loose),
        Some(Shared::Same(Some(s.walls)))
    );
}

#[test]
fn labels() {
    let s = scene();
    assert_eq!(tag_label(&s.model, Shared::Same(None)), "No tag");
    assert_eq!(tag_label(&s.model, Shared::Varies), "Mixed");
    assert_eq!(tag_label(&s.model, Shared::Same(Some(s.walls))), "Walls");
    let mut other = s.model.clone();
    let stray = other.add_tag("Elsewhere", &mut ModelEdit::new()).unwrap();
    assert_eq!(tag_label(&s.model, Shared::Same(Some(stray))), "?");
}

// --- the Tags panel ---

struct PanelState {
    panel: TagsPanel,
    model: Model,
    editor: Editor,
    actions: Vec<TagAction>,
}

fn panel_harness(s: Scene, selection: Vec<Entity>) -> Harness<'static, PanelState> {
    let mut editor = Editor::new();
    editor.select(selection);
    let mut h = Harness::builder()
        .with_size(egui::vec2(600.0, 800.0))
        .build_ui_state(
            |ui, state: &mut PanelState| {
                state.panel.selected = state
                    .editor
                    .selection()
                    .iter()
                    .filter(|e| matches!(e, Entity::Instance(_)))
                    .count();
                ui.allocate_ui(egui::vec2(240.0, 600.0), |ui| {
                    ui.set_max_width(240.0);
                    if let Some(action) = tags_panel(ui, &mut state.panel, &state.model) {
                        state.actions.push(action.clone());
                        if let TagAction::Assign(tag) = action {
                            state.editor.assign_selection_tag(&mut state.model, tag);
                        }
                    }
                });
            },
            PanelState {
                panel: TagsPanel::default(),
                model: s.model,
                editor,
                actions: Vec::new(),
            },
        );
    h.run();
    h
}

#[test]
fn the_panel_has_an_assign_button_per_tag_and_a_no_tag_button() {
    let s = scene();
    let a = s.a;
    let h = panel_harness(s, vec![Entity::Instance(a)]);
    assert!(h.query_by_label("Assign selection to Walls").is_some());
    assert!(h.query_by_label("Assign selection to Roof").is_some());
    assert!(h.query_by_label("No tag").is_some());
    assert!(h.state().actions.is_empty(), "nothing without a click");
}

#[test]
fn clicking_assign_gives_the_selection_the_tag_in_one_undo_step() {
    let s = scene();
    let (a, b, walls) = (s.a, s.b, s.walls);
    let mut h = panel_harness(s, vec![Entity::Instance(a), Entity::Instance(b)]);
    h.get_by_label("Assign selection to Walls").click();
    h.run();
    assert_eq!(h.state().actions, vec![TagAction::Assign(Some(walls))]);
    assert_eq!(tag_of(&h.state().model, a), Some(walls));
    assert_eq!(tag_of(&h.state().model, b), Some(walls));
    assert_eq!(steps(&h.state().editor), 1);

    let PanelState { editor, model, .. } = h.state_mut();
    assert!(editor.key(EditKey::Undo, model));
    assert_eq!(tag_of(&h.state().model, a), None);
    assert_eq!(tag_of(&h.state().model, b), None);
}

#[test]
fn clicking_no_tag_untags_the_selection() {
    let mut s = scene();
    let (a, walls) = (s.a, s.walls);
    s.model
        .assign_tag(a, Some(walls), &mut ModelEdit::new())
        .unwrap();
    let mut h = panel_harness(s, vec![Entity::Instance(a)]);
    h.get_by_label("No tag").click();
    h.run();
    assert_eq!(h.state().actions, vec![TagAction::Assign(None)]);
    assert_eq!(tag_of(&h.state().model, a), None);
    assert_eq!(steps(&h.state().editor), 1);
}

#[test]
fn the_buttons_do_nothing_without_an_instance_selected() {
    let s = scene();
    let loose = s.loose.clone();
    let mut h = panel_harness(s, loose);
    h.get_by_label("Assign selection to Walls").click();
    h.get_by_label("No tag").click();
    h.run();
    assert!(h.state().actions.is_empty());
    assert_eq!(steps(&h.state().editor), 0);
}

// --- Entity Info ---

struct InfoState {
    panel: EntityInfoPanel,
    model: Model,
    editor: Editor,
    actions: Vec<InfoAction>,
}

fn info_harness(s: Scene, selection: Vec<Entity>) -> Harness<'static, InfoState> {
    let mut editor = Editor::new();
    editor.select(selection);
    let mut h = Harness::builder()
        .with_size(egui::vec2(600.0, 800.0))
        .build_ui_state(
            |ui, state: &mut InfoState| {
                let selection = state.editor.selection().to_vec();
                if let Some(action) =
                    entity_info_panel(ui, &mut state.panel, &state.model, BodyId::Root, &selection)
                {
                    state.actions.push(action.clone());
                    if let InfoAction::SetTag(tag) = action {
                        state.editor.assign_selection_tag(&mut state.model, tag);
                    }
                }
            },
            InfoState {
                panel: EntityInfoPanel::default(),
                model: s.model,
                editor,
                actions: Vec::new(),
            },
        );
    h.run();
    h
}

/// The Tag dropdown: a combo box labelled "Tag" whose value is the current text.
fn combo<'a>(h: &'a Harness<'static, InfoState>) -> Option<egui_kittest::Node<'a>> {
    h.query_by_role_and_label(Role::ComboBox, "Tag")
}

fn combo_value(h: &Harness<'static, InfoState>) -> Option<String> {
    combo(h).and_then(|node| node.accesskit_node().value())
}

#[test]
fn entity_info_shows_the_tag_of_one_instance() {
    let mut s = scene();
    let a = s.a;
    let walls = s.walls;
    s.model
        .assign_tag(a, Some(walls), &mut ModelEdit::new())
        .unwrap();
    let h = info_harness(s, vec![Entity::Instance(a)]);
    assert_eq!(combo_value(&h).as_deref(), Some("Walls"));
}

#[test]
fn entity_info_says_no_tag_for_an_untagged_instance() {
    let s = scene();
    let b = s.b;
    let h = info_harness(s, vec![Entity::Instance(b)]);
    assert_eq!(combo_value(&h).as_deref(), Some("No tag"));
}

#[test]
fn entity_info_says_mixed_when_the_tags_differ() {
    let mut s = scene();
    let (a, b, walls) = (s.a, s.b, s.walls);
    s.model
        .assign_tag(a, Some(walls), &mut ModelEdit::new())
        .unwrap();
    let h = info_harness(s, vec![Entity::Instance(a), Entity::Instance(b)]);
    assert_eq!(combo_value(&h).as_deref(), Some("Mixed"));
}

#[test]
fn entity_info_has_no_tag_row_without_an_instance() {
    let s = scene();
    let loose = s.loose.clone();
    let h = info_harness(s, loose);
    assert!(combo(&h).is_none());
}

#[test]
fn the_dropdown_changes_the_tag_of_every_selected_instance_in_one_step() {
    let s = scene();
    let (a, b, c, roof) = (s.a, s.b, s.c, s.roof);
    let mut selection = s.loose.clone();
    selection.extend([Entity::Instance(a), Entity::Instance(b)]);
    let mut h = info_harness(s, selection);
    h.get_by_role_and_label(Role::ComboBox, "Tag").click();
    h.run();
    h.get_by_label("Roof").click();
    h.run();
    assert_eq!(h.state().actions, vec![InfoAction::SetTag(Some(roof))]);
    assert_eq!(tag_of(&h.state().model, a), Some(roof));
    assert_eq!(tag_of(&h.state().model, b), Some(roof));
    assert_eq!(tag_of(&h.state().model, c), None);
    assert_eq!(steps(&h.state().editor), 1);
    assert_eq!(
        combo_value(&h).as_deref(),
        Some("Roof"),
        "the row shows the tag"
    );

    let InfoState { editor, model, .. } = h.state_mut();
    assert!(editor.key(EditKey::Undo, model));
    h.run();
    assert_eq!(tag_of(&h.state().model, a), None);
    assert_eq!(combo_value(&h).as_deref(), Some("No tag"));
}

#[test]
fn the_dropdown_can_untag() {
    let mut s = scene();
    let (a, walls) = (s.a, s.walls);
    s.model
        .assign_tag(a, Some(walls), &mut ModelEdit::new())
        .unwrap();
    let mut h = info_harness(s, vec![Entity::Instance(a)]);
    h.get_by_role_and_label(Role::ComboBox, "Tag").click();
    h.run();
    h.get_by_label("No tag").click();
    h.run();
    assert_eq!(h.state().actions, vec![InfoAction::SetTag(None)]);
    assert_eq!(tag_of(&h.state().model, a), None);
}
