use gtk::gdk;
use relm4::adw::prelude::*;
use relm4::factory::DynamicIndex;
use relm4::Sender;
use relm4::{adw, gtk};

use super::dialog::{dialog_action_row, dialog_shell, wire_submit};
use super::{build_pango_attrs, pango_to_rgba, rgba_to_pango};
use crate::app::AppMsg;
use crate::core::filter_set::{color_to_pango, pango_to_color, FilterData};
use crate::core::types::{ContextSpec, RowStyle};

pub fn show_filter_dialog(
    parent: &adw::ApplicationWindow,
    sender: Sender<AppMsg>,
    edit_target: Option<(DynamicIndex, FilterData)>,
) {
    let is_edit = edit_target.is_some();
    let edit_index = edit_target.as_ref().map(|(i, _)| i.clone());

    let (dialog, content) = dialog_shell(
        if is_edit { "Edit Filter" } else { "Add Filter" },
        parent,
        480,
    );

    let query_entry = gtk::Entry::builder()
        .placeholder_text("Enter filter pattern (Regex or Substring)...")
        .hexpand(true)
        .build();
    content.append(&query_entry);

    let options_row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    let type_dropdown = gtk::DropDown::from_strings(&["Regex", "Substring"]);
    let mode_dropdown = gtk::DropDown::from_strings(&["Include", "Exclude"]);
    let case_check = gtk::CheckButton::with_label("Case sensitive");

    options_row.append(&type_dropdown);
    options_row.append(&mode_dropdown);
    options_row.append(&case_check);
    content.append(&options_row);

    let ctx_check = gtk::CheckButton::with_label("Enable context");
    let ctx_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(8)
        .margin_top(4)
        .margin_start(12)
        .build();
    let c_chk = gtk::CheckButton::with_label("C both");
    let b_chk = gtk::CheckButton::with_label("B before");
    let a_chk = gtk::CheckButton::with_label("A after");
    let ctx_spin = gtk::SpinButton::with_range(1.0, 50.0, 1.0);
    ctx_spin.set_value(3.0);
    ctx_spin.set_tooltip_text(Some("Number of context lines"));
    ctx_box.append(&c_chk);
    ctx_box.append(&b_chk);
    ctx_box.append(&a_chk);
    ctx_box.append(&ctx_spin);
    ctx_box.set_sensitive(false);

    {
        let c1 = c_chk.clone();
        let a1 = a_chk.clone();
        let b1 = b_chk.clone();
        c_chk.connect_toggled(move |_| {
            if c1.is_active() {
                a1.set_active(false);
                b1.set_active(false);
            }
        });
        let c2 = c_chk.clone();
        let a2 = a_chk.clone();
        a_chk.connect_toggled(move |_| {
            if a2.is_active() {
                c2.set_active(false);
            }
        });
        let c3 = c_chk.clone();
        let b2 = b_chk.clone();
        b_chk.connect_toggled(move |_| {
            if b2.is_active() {
                c3.set_active(false);
            }
        });
    }
    {
        let ctx_box = ctx_box.clone();
        ctx_check.connect_toggled(move |btn| ctx_box.set_sensitive(btn.is_active()));
    }
    {
        let dd = mode_dropdown.clone();
        let dd2 = mode_dropdown.clone();
        let ctx_check = ctx_check.clone();
        dd.connect_selected_notify(move |_| {
            let is_exclude = dd2.selected() == 1;
            ctx_check.set_sensitive(!is_exclude);
            if is_exclude {
                ctx_check.set_active(false);
            }
        });
    }
    content.append(&ctx_check);
    content.append(&ctx_box);

    let expander = gtk::Expander::builder()
        .label("Colors & Appearance (Text & Background)")
        .expanded(true)
        .build();

    let color_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(10)
        .margin_top(8)
        .build();

    let color_dialog = gtk::ColorDialog::builder().with_alpha(false).build();

    let default_fg = gdk::RGBA::new(1.0, 0.85, 0.35, 1.0);
    let default_bg = gdk::RGBA::new(0.45, 0.12, 0.15, 1.0);

    let fg_row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    let use_fg_check = gtk::CheckButton::builder()
        .label("Custom Text Color")
        .active(true)
        .hexpand(true)
        .build();
    let fg_btn = gtk::ColorDialogButton::builder()
        .dialog(&color_dialog)
        .rgba(&default_fg)
        .build();
    fg_row.append(&use_fg_check);
    fg_row.append(&fg_btn);

    let bg_row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    let use_bg_check = gtk::CheckButton::builder()
        .label("Custom Background Color")
        .active(true)
        .hexpand(true)
        .build();
    let bg_btn = gtk::ColorDialogButton::builder()
        .dialog(&color_dialog)
        .rgba(&default_bg)
        .build();
    bg_row.append(&use_bg_check);
    bg_row.append(&bg_btn);

    if let Some((_, cfg)) = &edit_target {
        query_entry.set_text(&cfg.query);
        let selected_idx = match cfg.filter_type.as_str() {
            "Regex" => 0,
            _ => 1,
        };
        type_dropdown.set_selected(selected_idx);
        mode_dropdown.set_selected(if cfg.mode == "Include" { 0 } else { 1 });
        case_check.set_active(!cfg.case_insensitive);

        let fg = cfg.fg.as_deref().and_then(color_to_pango);
        use_fg_check.set_active(fg.is_some());
        if let Some(c) = fg {
            fg_btn.set_rgba(&pango_to_rgba(c));
        }

        let bg = cfg.bg.as_deref().and_then(color_to_pango);
        use_bg_check.set_active(bg.is_some());
        if let Some(c) = bg {
            bg_btn.set_rgba(&pango_to_rgba(c));
        }

        match cfg.context {
            ContextSpec::None => {}
            ContextSpec::Before(n) => {
                ctx_check.set_active(true);
                b_chk.set_active(true);
                ctx_spin.set_value(n as f64);
            }
            ContextSpec::After(n) => {
                ctx_check.set_active(true);
                a_chk.set_active(true);
                ctx_spin.set_value(n as f64);
            }
            ContextSpec::Both(n) => {
                ctx_check.set_active(true);
                c_chk.set_active(true);
                ctx_spin.set_value(n as f64);
            }
        }
        if matches!(
            cfg.context,
            ContextSpec::Before(0) | ContextSpec::After(0) | ContextSpec::Both(0)
        ) {
            ctx_check.set_active(false);
        }
        if cfg.mode == "Exclude" {
            ctx_check.set_active(false);
            ctx_check.set_sensitive(false);
        }
    }

    let preview_label = gtk::Label::builder()
        .label("[2026-11-13 18:00:00] Preview matching log line appearance")
        .css_classes(["monospace"])
        .halign(gtk::Align::Start)
        .margin_top(4)
        .build();

    let update_preview = {
        let use_fg_check = use_fg_check.clone();
        let fg_btn = fg_btn.clone();
        let use_bg_check = use_bg_check.clone();
        let bg_btn = bg_btn.clone();
        let preview_label = preview_label.clone();
        move || {
            let fg = if use_fg_check.is_active() {
                Some(rgba_to_pango(&fg_btn.rgba()))
            } else {
                None
            };
            let bg = if use_bg_check.is_active() {
                Some(rgba_to_pango(&bg_btn.rgba()))
            } else {
                None
            };
            preview_label.set_attributes(build_pango_attrs(Some(RowStyle { fg, bg })).as_ref());
        }
    };
    update_preview();

    let up1 = update_preview.clone();
    use_fg_check.connect_toggled(move |_| up1());
    let up2 = update_preview.clone();
    use_bg_check.connect_toggled(move |_| up2());
    let up3 = update_preview.clone();
    fg_btn.connect_rgba_notify(move |_| up3());
    let up4 = update_preview.clone();
    bg_btn.connect_rgba_notify(move |_| up4());

    color_box.append(&fg_row);
    color_box.append(&bg_row);
    color_box.append(&preview_label);
    expander.set_child(Some(&color_box));
    content.append(&expander);

    let submit_btn =
        dialog_action_row(&content, &dialog, if is_edit { "Save Changes" } else { "Add Filter" });

    let submit = {
        let dialog = dialog.clone();
        let query_entry = query_entry.clone();
        move || {
            let text = query_entry.text().to_string();
            if !text.is_empty() {
                let filter_type = match type_dropdown.selected() {
                    0 => "Regex",
                    _ => "Substring",
                }
                .to_string();
                let mode = if mode_dropdown.selected() == 0 {
                    "Include"
                } else {
                    "Exclude"
                }
                .to_string();

                let fg_rgba = if use_fg_check.is_active() {
                    Some(fg_btn.rgba())
                } else {
                    None
                };
                let bg_rgba = if use_bg_check.is_active() {
                    Some(bg_btn.rgba())
                } else {
                    None
                };

                let context = if ctx_check.is_active() {
                    let n = ctx_spin.value() as u32;
                    if c_chk.is_active() || (b_chk.is_active() && a_chk.is_active()) {
                        ContextSpec::Both(n)
                    } else if b_chk.is_active() {
                        ContextSpec::Before(n)
                    } else if a_chk.is_active() {
                        ContextSpec::After(n)
                    } else {
                        ContextSpec::None
                    }
                } else {
                    ContextSpec::None
                };

                let new_cfg = FilterData {
                    filter_type,
                    mode,
                    query: text,
                    case_insensitive: !case_check.is_active(),
                    active: true,
                    fg: fg_rgba.as_ref().map(|c| pango_to_color(rgba_to_pango(c))),
                    bg: bg_rgba.as_ref().map(|c| pango_to_color(rgba_to_pango(c))),
                    context,
                };

                if let Some(ref idx) = edit_index {
                    sender.emit(AppMsg::UpdateFilter(idx.clone(), new_cfg));
                } else {
                    sender.emit(AppMsg::AddFilter(new_cfg));
                }
                dialog.close();
            }
        }
    };

    wire_submit(&submit_btn, &query_entry, submit);

    dialog.present();
    query_entry.grab_focus();
}
