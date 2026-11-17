
use relm4::{adw, gtk};
use relm4::adw::prelude::*;

pub fn dialog_shell(
    title: &str,
    parent: &impl IsA<gtk::Window>,
    width: i32,
) -> (adw::Window, gtk::Box) {
    let window = adw::Window::builder()
        .title(title)
        .transient_for(parent)
        .modal(true)
        .default_width(width)
        .build();
    let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
    root.append(&adw::HeaderBar::new());
    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(12)
        .margin_top(16)
        .margin_bottom(16)
        .margin_start(16)
        .margin_end(16)
        .build();
    root.append(&content);
    window.set_content(Some(&root));
    (window, content)
}

pub fn dialog_action_row(parent: &gtk::Box, window: &adw::Window, ok_label: &str) -> gtk::Button {
    let row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(8)
        .halign(gtk::Align::End)
        .margin_top(8)
        .build();
    let cancel = gtk::Button::with_label("Cancel");
    let cancel_target = window.clone();
    cancel.connect_clicked(move |_| cancel_target.close());
    let ok = gtk::Button::builder()
        .label(ok_label)
        .css_classes(["suggested-action"])
        .build();
    row.append(&cancel);
    row.append(&ok);
    parent.append(&row);
    ok
}

pub fn wire_submit<F>(ok: &gtk::Button, entry: &gtk::Entry, f: F)
where
    F: Fn() + Clone + 'static,
{
    let f2 = f.clone();
    ok.connect_clicked(move |_| f2());
    entry.connect_activate(move |_| f());
}
