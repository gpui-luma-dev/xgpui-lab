use super::*;
use gpui::{AnyElement, IntoElement, MouseButton, Window, div, prelude::*};
use gpui_luma::controls::listbox::{
    ListBoxAxis, ListBoxBinding, ListBoxEvent, ListBoxInput, ListBoxSnapshot, ListBoxState, SelectionMode,
    SelectionPolicy,
};

type InputHandler = fn(&mut GradientBuilder, ListBoxInput<u64>, &mut Window, &mut Context<GradientBuilder>);

/// Persistent collection focus; nested controls retain their own keyboard input.
pub(super) struct ColorFieldList {
    pub state: ListBoxState<u64, u64>,
    pub binding: ListBoxBinding,
}

impl ColorFieldList {
    pub fn new(cx: &mut Context<GradientBuilder>) -> Self {
        let mut state = ListBoxState::try_new([], |key: &u64| *key, SelectionMode::SingleAllowNone)
            .expect("empty color list has unique keys");
        state.set_selection_policy(SelectionPolicy {
            selection_follows_active: true,
            ..SelectionPolicy::new(SelectionMode::SingleAllowNone)
        });
        Self { state, binding: ListBoxBinding::new(cx) }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &mut self,
        rows: Vec<(u64, AnyElement)>,
        selected: Option<u64>,
        id: &'static str,
        chrome: gpui_luma::theme::LumaChrome,
        window: &mut Window,
        cx: &mut Context<GradientBuilder>,
        on_input: InputHandler,
    ) -> AnyElement {
        // Reconcile domain edits while constructing this frame, without requesting
        // another frame. The initiating edit already notifies the owning view.
        let snapshot = ListBoxSnapshot::try_new(rows.iter().map(|(key, _)| *key), |key| *key)
            .expect("color field keys are unique");
        self.state.replace_snapshot(snapshot);
        if self.state.selected_keys().copied().next() != selected {
            if let Some(key) = selected {
                self.state.apply(ListBoxInput::Select(key));
            } else {
                self.state.apply(ListBoxInput::ClearSelection);
            }
        }
        let focus_visible = self.binding.focus_visible(window);
        let states = self.state.visible_items().map(|item| item.state).collect::<Vec<_>>();
        let focus = self.binding.focus_handle().clone();
        let root = div()
            .id(id)
            .w_full()
            .flex()
            .flex_col()
            .gap_1()
            .py_2()
            .bg(chrome.panel_background)
            .on_mouse_down_out(cx.listener(move |owner, event: &gpui::MouseDownEvent, window, cx| {
                if event.button == MouseButton::Left && !owner.color_picker_open {
                    if focus.contains_focused(window, cx) {
                        window.blur(cx);
                    }
                    on_input(owner, ListBoxInput::ClearSelection, window, cx);
                }
            }));
        let root =
            self.binding
                .bind_root(root, ListBoxAxis::Vertical, SelectionMode::SingleAllowNone, window, cx, on_input);
        root.children(rows.into_iter().zip(states).enumerate().map(|(index, ((key, content), state))| {
            let row = div()
                .id(format!("{id}-row-{key}"))
                .debug_selector(move || {
                    if index == 0 {
                        format!("{id}-first-row")
                    } else {
                        format!("{id}-row-{key}")
                    }
                })
                .w_full()
                .rounded(px(4.0))
                .border_1()
                .border_color(if focus_visible && state.active {
                    chrome.body_text
                } else {
                    chrome.panel_background
                })
                .aria_label(format!("Color field {}", index + 1))
                .child(content);
            self.binding.bind_row(row, key, state, cx, on_input)
        }))
        .into_any_element()
    }
}

impl GradientBuilder {
    pub(super) fn handle_stop_list_input(
        &mut self,
        input: ListBoxInput<u64>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let update = self.stop_list.state.apply(input);
        for event in update.events {
            match event {
                ListBoxEvent::SelectionChanged { selected, .. } => {
                    let selected = selected.first().copied();
                    let thumb = self
                        .ordered_stop_rows(cx)
                        .into_iter()
                        .find(|(id, _, _)| Some(id.as_u64()) == selected)
                        .map(|(id, _, _)| id);
                    if let Some(id) = thumb {
                        self.select_stop_row(id, cx);
                    } else {
                        self.selected_stop = None;
                    }
                }
                ListBoxEvent::ItemActivated { key } => {
                    if let Some((id, _, _)) =
                        self.ordered_stop_rows(cx).into_iter().find(|(id, _, _)| id.as_u64() == key)
                    {
                        self.open_color_picker_for_stop(id, cx);
                    }
                }
                _ => {}
            }
        }
        cx.notify();
    }

    pub(super) fn handle_point_list_input(
        &mut self,
        input: ListBoxInput<u64>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let update = self.point_list.state.apply(input);
        for event in update.events {
            match event {
                ListBoxEvent::SelectionChanged { selected, .. } => {
                    self.selected_mesh_point =
                        selected.first().and_then(|key| self.mesh_point_ids.iter().position(|id| id == key));
                    self.mesh_color_target =
                        self.selected_mesh_point.map(MeshColorTarget::Point).unwrap_or(MeshColorTarget::Background);
                    self.active_mesh_drag = None;
                    self.sync_point_spread(cx);
                }
                ListBoxEvent::ItemActivated { key } => {
                    if let Some(index) = self.mesh_point_ids.iter().position(|id| *id == key) {
                        self.open_color_picker_for_mesh_point(index, cx);
                    }
                }
                _ => {}
            }
        }
        cx.notify();
    }
}
