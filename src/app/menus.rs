use glib::VariantTy;
use relm4::gtk::prelude::*;
use relm4::{adw, gtk, Sender};
use std::path::PathBuf;

use super::AppMsg;

pub const SHORTCUTS: &[(&str, &str)] = &[
    ("o", "win.open"),
    ("s", "win.save-filtered"),
    ("n", "win.add-filter"),
    ("h", "win.toggle-highlight"),
    ("c", "win.copy-line"),
    ("q", "win.quit"),
];

fn accel_label(label: &str, action: &str) -> String {
    match SHORTCUTS.iter().find(|(_, a)| *a == action) {
        Some((key, _)) => format!("{label} (Ctrl+{})", key.to_ascii_uppercase()),
        None => label.to_string(),
    }
}

pub fn build_menu_model(recent: &[PathBuf], sets: &[(String, PathBuf)]) -> gio::Menu {
    let menu_bar = gio::Menu::new();

    let file_menu = gio::Menu::new();
    let file_io_section = gio::Menu::new();
    file_io_section.append(
        Some(&accel_label("Open Log File...", "win.open")),
        Some("win.open"),
    );
    file_io_section.append(
        Some(&accel_label("Save Filtered Output...", "win.save-filtered")),
        Some("win.save-filtered"),
    );
    file_menu.append_section(None, &file_io_section);

    if !recent.is_empty() {
        let recent_menu = gio::Menu::new();
        for p in recent {
            let label = p.display().to_string();
            let action = format!("win.open-recent::{}", p.display());
            recent_menu.append(Some(&label), Some(&action));
        }
        file_menu.append_submenu(Some("Open Recent"), &recent_menu);
    }

    let file_quit_section = gio::Menu::new();
    file_quit_section.append(Some(&accel_label("Quit", "win.quit")), Some("win.quit"));
    file_menu.append_section(None, &file_quit_section);
    menu_bar.append_submenu(Some("File"), &file_menu);

    let edit_menu = gio::Menu::new();
    let edit_clip_section = gio::Menu::new();
    edit_clip_section.append(
        Some(&accel_label("Copy Selected Line", "win.copy-line")),
        Some("win.copy-line"),
    );
    edit_menu.append_section(None, &edit_clip_section);

    let edit_filter_section = gio::Menu::new();
    edit_filter_section.append(
        Some(&accel_label("Add New Filter...", "win.add-filter")),
        Some("win.add-filter"),
    );
    edit_filter_section.append(Some("Enable All Filters"), Some("win.enable-all-filters"));
    edit_filter_section.append(Some("Disable All Filters"), Some("win.disable-all-filters"));
    edit_filter_section.append(Some("Clear All Filters"), Some("win.clear-filters"));
    edit_filter_section.append(Some("Import Filters..."), Some("win.import-filters"));
    edit_filter_section.append(Some("Export Filters..."), Some("win.export-filters"));
    edit_filter_section.append(Some("Save Filter Set As..."), Some("win.save-set"));
    edit_menu.append_section(None, &edit_filter_section);

    if !sets.is_empty() {
        let sets_menu = gio::Menu::new();
        for (name, _) in sets {
            let action = format!("win.apply-set::{name}");
            sets_menu.append(Some(name), Some(&action));
        }
        edit_menu.append_submenu(Some("Filter Sets"), &sets_menu);
    }
    menu_bar.append_submenu(Some("Edit"), &edit_menu);

    let view_menu = gio::Menu::new();
    view_menu.append(
        Some(&accel_label("Highlight-Only Mode", "win.toggle-highlight")),
        Some("win.toggle-highlight"),
    );
    view_menu.append(Some("Descending Line Order"), Some("win.toggle-sort"));
    view_menu.append(Some("Reset Split Pane Position"), Some("win.reset-pane"));
    menu_bar.append_submenu(Some("View"), &view_menu);

    let help_menu = gio::Menu::new();
    help_menu.append(Some("About Zerogrep"), Some("win.about"));
    menu_bar.append_submenu(Some("Help"), &help_menu);

    menu_bar
}

pub fn register_window_actions(window: &adw::ApplicationWindow, sender: Sender<AppMsg>) {
    let add_action = |name: &str, msg_fn: fn() -> AppMsg| {
        let action = gio::SimpleAction::new(name, None);
        let s = sender.clone();
        action.connect_activate(move |_, _| {
            s.emit(msg_fn());
        });
        window.add_action(&action);
    };

    add_action("open", || AppMsg::OpenFileDialog);
    add_action("save-filtered", || AppMsg::SaveFilteredDialog);
    add_action("quit", || AppMsg::QuitApp);
    add_action("copy-line", || AppMsg::CopySelectedLine);
    add_action("add-filter", || AppMsg::OpenAddFilterDialog);
    add_action("enable-all-filters", || AppMsg::SetAllFiltersActive(true));
    add_action("disable-all-filters", || AppMsg::SetAllFiltersActive(false));
    add_action("clear-filters", || AppMsg::ClearAllFilters);
    add_action("export-filters", || AppMsg::ExportFiltersDialog);
    add_action("import-filters", || AppMsg::ImportFiltersDialog);
    add_action("save-set", || AppMsg::SaveSetDialog);
    add_action("reset-pane", || AppMsg::ResetSplitPane);
    add_action("about", || AppMsg::ShowAboutDialog);

    let open_recent = gio::SimpleAction::new("open-recent", Some(VariantTy::STRING));
    {
        let s = sender.clone();
        open_recent.connect_activate(move |_, param| {
            if let Some(path) = param.and_then(|v| v.get::<String>()).map(PathBuf::from) {
                s.emit(AppMsg::OpenRecent(path));
            }
        });
    }
    window.add_action(&open_recent);

    let apply_set = gio::SimpleAction::new("apply-set", Some(VariantTy::STRING));
    {
        let s = sender.clone();
        apply_set.connect_activate(move |_, param| {
            if let Some(name) = param.and_then(|v| v.get::<String>()) {
                s.emit(AppMsg::ApplySet(name));
            }
        });
    }
    window.add_action(&apply_set);

    let highlight_action = gio::SimpleAction::new_stateful(
        "toggle-highlight",
        None,
        &false.to_variant(),
    );
    let s_high = sender.clone();
    highlight_action.connect_activate(move |action, _| {
        let current = action.state().and_then(|v| v.get::<bool>()).unwrap_or(false);
        let next = !current;
        action.set_state(&next.to_variant());
        s_high.emit(AppMsg::SetHighlightOnly(next));
    });
    window.add_action(&highlight_action);

    let sort_action = gio::SimpleAction::new_stateful(
        "toggle-sort",
        None,
        &false.to_variant(),
    );
    let s_sort = sender.clone();
    sort_action.connect_activate(move |action, _| {
        let current = action.state().and_then(|v| v.get::<bool>()).unwrap_or(false);
        let next = !current;
        action.set_state(&next.to_variant());
        s_sort.emit(AppMsg::SetSortOrder(next));
    });
    window.add_action(&sort_action);

    let key_controller = gtk::EventControllerKey::new();
    let shortcut_window = window.clone();
    key_controller.connect_key_pressed(move |_, keyval, _, state| {
        if state.intersects(gtk::gdk::ModifierType::CONTROL_MASK)
            && let Some(key) = keyval.to_unicode()
            && let Some((_k, action)) = SHORTCUTS
                .iter()
                .find(|(k, _)| *k == key.to_ascii_lowercase().to_string())
        {
            let _ = gtk::prelude::WidgetExt::activate_action(&shortcut_window, action, None);
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    });
    window.add_controller(key_controller);
}
