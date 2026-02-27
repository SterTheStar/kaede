use adw::prelude::*;

pub(crate) fn clear_listbox(list: &gtk::ListBox) {
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }
}

pub(crate) fn widget_is_descendant_of(widget: &gtk::Widget, ancestor: &gtk::Widget) -> bool {
    let mut current = Some(widget.clone());
    while let Some(node) = current {
        if node == *ancestor {
            return true;
        }
        current = node.parent();
    }
    false
}

pub(crate) fn set_details_panel_visible(
    _content: &gtk::Paned,
    details_revealer: &gtk::Revealer,
    apps_scrolled: &gtk::ScrolledWindow,
    visible: bool,
) {
    if visible {
        apps_scrolled.set_margin_end(10);
    } else {
        apps_scrolled.set_margin_end(0);
    }
    if visible {
        details_revealer.set_visible(true);
    }
    details_revealer.set_reveal_child(visible);
}

