//! Labeled, directly selectable lab settings. Numeric IDs stay independent of display labels.
use std::{cell::RefCell, rc::Rc};
use gpui::{prelude::*, Context, Entity, SharedString, div};
use gpui_luma::controls::selector::{Selector, SelectorItem};
use gpui_luma_look_radix::{Look, Selector as RadixSelector};

#[derive(Clone)]
pub(crate) struct Choice {
    pub selector: Entity<Selector>,
    label: SharedString,
    labels: Rc<RefCell<Vec<String>>>,
}

pub(crate) fn labels(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

fn items(labels: &[String]) -> Vec<SelectorItem> {
    labels
        .iter()
        .enumerate()
        .map(|(index, label)| SelectorItem::new(index.to_string()).label(label.clone()))
        .collect()
}

impl Choice {
    pub fn new<M: 'static>(
        id: impl Into<SharedString>,
        label: &str,
        look: &Look,
        options: Vec<String>,
        selected: usize,
        cx: &mut Context<M>,
        on_change: impl Fn(&mut M, usize, &mut Context<M>) + 'static,
    ) -> Self {
        let labels = Rc::new(RefCell::new(options));
        let callback_labels = labels.clone();
        let selector = RadixSelector::new(id)
            .look(look)
            .enabled(!labels.borrow().is_empty())
            .items(items(&labels.borrow()))
            .selected_id(selected.to_string())
            .on_value_changed(cx, move |owner, id, _, cx| {
                let count = callback_labels.borrow().len();
                if let Ok(index) = id.parse::<usize>()
                    && index < count
                {
                    on_change(owner, index, cx);
                }
            })
            .spawn(cx);
        Self { selector, label: label.to_owned().into(), labels }
    }

    pub fn sync(&self, options: Vec<String>, selected: usize, cx: &mut gpui::App) {
        let changed = *self.labels.borrow() != options;
        if changed {
            *self.labels.borrow_mut() = options;
        }
        let id = (selected < self.labels.borrow().len()).then(|| selected.to_string());
        if changed || self.selector.read(cx).selected_id().map(|id| id.as_ref()) != id.as_deref() {
            self.selector.update(cx, |selector, cx| {
                if changed {
                    selector.set_items(items(&self.labels.borrow()), cx);
                    selector.set_enabled(!self.labels.borrow().is_empty(), cx);
                }
                if let Some(id) = id {
                    selector.set_selected_id(id, cx);
                }
            });
        }
    }

    #[cfg(test)]
    pub fn select(&self, index: usize, cx: &mut gpui::App) {
        let label = self.labels.borrow()[index].clone();
        self.selector.update(cx, |selector, cx| {
            selector.set_selected_id(index.to_string(), cx);
            cx.emit(gpui_luma::controls::selector::SelectorEvent::Change {
                item_id: index.to_string().into(),
                label: label.into(),
            });
        });
    }
}

impl IntoElement for Choice {
    type Element = gpui::Div;
    fn into_element(self) -> Self::Element {
        div().flex().items_center().gap_2().child(self.label).child(self.selector)
    }
}
