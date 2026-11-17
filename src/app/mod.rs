pub mod menus;

use gio::ListStore;
use glib::BoxedAnyObject;
use gtk::{gdk, pango};
use memmap2::Mmap;
use relm4::factory::{DynamicIndex, FactoryVecDeque};
use relm4::gtk::prelude::*;
use relm4::{adw, gtk, ComponentParts, ComponentSender, RelmWidgetExt, Sender, SimpleComponent};
use relm4::adw::prelude::*;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, Mutex, OnceLock};

use crate::components::build_line_attrs;
use crate::components::dialog::{dialog_action_row, dialog_shell, wire_submit};
use crate::components::filter_dialog::show_filter_dialog;
use crate::components::filter_row::{FilterMsg, FilterOutput, FilterRow};
use crate::core::filter_set::{self, FilterData};
use crate::core::log_file;
use crate::core::report;
use crate::core::rules;
use crate::core::search::{self, SearchOutput};
use crate::core::search_session::SearchSession;
use crate::core::session;
use crate::core::types::{line_range, LineEntry};
use menus::register_window_actions;

static INPUT_SENDER: OnceLock<Mutex<Option<Sender<AppMsg>>>> = OnceLock::new();

static PENDING_FILES: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());

pub(crate) fn forward_file(path: PathBuf) {
    if let Some(slot) = INPUT_SENDER.get()
        && let Some(sender) = &*slot.lock().unwrap()
    {
        sender.emit(AppMsg::LoadFile(path));
        return;
    }
    PENDING_FILES.lock().unwrap().push(path);
}

pub(crate) fn register_input_sender(sender: &Sender<AppMsg>) {
    let slot = INPUT_SENDER.get_or_init(|| Mutex::new(None));
    *slot.lock().unwrap() = Some(sender.clone());
}

pub(crate) fn take_pending_files() -> Vec<PathBuf> {
    std::mem::take(&mut *PENDING_FILES.lock().unwrap())
}

pub struct ZerogrepApp {
    filters: FactoryVecDeque<FilterRow>,
    filtered_store: ListStore,
    selection_model: gtk::SingleSelection,
    search_session: Rc<RefCell<SearchSession>>,
    current_file_label: String,
    main_window: adw::ApplicationWindow,
    split_paned: gtk::Paned,
    highlight_only: bool,
    list_view: gtk::ListView,
    menu_bar: gtk::PopoverMenuBar,
    recent: Vec<PathBuf>,
    status_file: String,
}

impl ZerogrepApp {
    fn sync_store(&self) {
        let session = self.search_session.borrow();
        let mut items: Vec<BoxedAnyObject> = session
            .results()
            .iter()
            .copied()
            .map(BoxedAnyObject::new)
            .collect();
        if session.descending() {
            items.reverse();
        }
        self.filtered_store
            .splice(0, self.filtered_store.n_items(), &items);
    }

    fn sync_action_state(&self, action_name: &str, value: bool) {
        if let Some(action) = self
            .main_window
            .lookup_action(action_name)
            .and_then(|a| a.downcast::<gio::SimpleAction>().ok())
        {
            action.set_state(&value.to_variant());
        }
    }

    fn sync_filter_actions(&self) {
        let enabled = !self.filters.is_empty();
        for name in [
            "export-filters",
            "enable-all-filters",
            "disable-all-filters",
            "clear-filters",
        ] {
            if let Some(action) = self
                .main_window
                .lookup_action(name)
                .and_then(|a| a.downcast::<gio::SimpleAction>().ok())
            {
                action.set_enabled(enabled);
            }
        }
    }

    fn rebuild_menu(&self) {
        let sets = session::list_sets();
        let model = menus::build_menu_model(&self.recent, &sets);
        self.menu_bar.set_menu_model(Some(&model));
    }

    fn persist_session(&self) {
        let filters: Vec<FilterData> = self
            .filters
            .iter()
            .map(|row| row.data.clone())
            .collect();
        let state = self.search_session.borrow();
        session::save_session(&session::Session {
            file: state.path().map(PathBuf::from),
            recent: self.recent.clone(),
            filters,
            descending: state.descending(),
            highlight_only: self.highlight_only,
        });
    }
}

#[derive(Debug)]
pub enum AppMsg {
    OpenFileDialog,
    SaveFilteredDialog,
    SaveFilteredToPath(PathBuf),
    SaveFilteredDone(PathBuf, Result<(), String>, u64),
    LoadFile(PathBuf),
    FileLoaded(PathBuf, Arc<Mmap>, Arc<Vec<usize>>, u64),
    FileLoadFailed(PathBuf, u64),
    OpenAddFilterDialog,
    OpenEditFilterDialog(DynamicIndex, FilterData),
    AddFilter(FilterData),
    UpdateFilter(DynamicIndex, FilterData),
    RemoveFilter(DynamicIndex),
    ToggleFilter(DynamicIndex, bool),
    SetAllFiltersActive(bool),
    ClearAllFilters,
    ExportFiltersDialog,
    ExportFiltersToPath(PathBuf),
    ExportFiltersDone(PathBuf, usize, Result<(), String>),
    ImportFiltersDialog,
    ImportFiltersFile(PathBuf),
    ImportFiltersLoaded(Result<(String, Vec<FilterData>, Vec<String>), String>),
    ApplyImportedFilters(Vec<FilterData>, bool, Vec<String>),
    OpenRecent(PathBuf),
    ApplySet(String),
    SaveSetDialog,
    SaveSetToName(String),
    CopySelectedLine,
    SetSortOrder(bool),
    SetHighlightOnly(bool),
    ResetSplitPane,
    ShowAboutDialog,
    QuitApp,
    TriggerSearch,
    RunSearch(Vec<crate::core::types::FilterSpec>, Vec<Option<usize>>),
    ApplySearchResults(SearchOutput, Vec<Option<usize>>, u64),
    NavigateFilter(DynamicIndex, bool),
}

fn hex_digit(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

fn file_uri_to_path(uri: &str) -> Option<PathBuf> {
    let rest = uri.trim().strip_prefix("file://")?;
    let path_part = rest
        .split_once('/')
        .and_then(|(host, path)| (host.is_empty() || host == "localhost").then_some(path))?
        .to_string();
    let mut out: Vec<u8> = Vec::with_capacity(path_part.len());
    let mut bytes = path_part.bytes();
    while let Some(b) = bytes.next() {
        if b == b'%'
            && let Some(h) = bytes.next()
            && let Some(l) = bytes.next()
        {
            out.push(hex_digit(h)? * 16 + hex_digit(l)?);
        } else {
            out.push(b);
        }
    }
    Some(PathBuf::from(String::from_utf8(out).ok()?))
}

fn nav_start_index(matches: &[usize], cur: usize, next: bool) -> Option<usize> {
    let n = matches.len();
    if n == 0 {
        return None;
    }
    Some(match matches.binary_search(&cur) {
        Ok(p) if next => (p + 1) % n,
        Ok(p) => {
            if p == 0 {
                n - 1
            } else {
                p - 1
            }
        }
        Err(p) if next => p % n,
        Err(p) => {
            if p == 0 {
                n - 1
            } else {
                p - 1
            }
        }
    })
}

#[relm4::component(pub)]
impl SimpleComponent for ZerogrepApp {
    type Init = ();
    type Input = AppMsg;
    type Output = ();

    view! {
        #[name = "main_window"]
        adw::ApplicationWindow {
            set_title: Some("Zerogrep"),
            set_default_size: (1200, 800),

            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                
                gtk::HeaderBar {
                    #[wrap(Some)]
                    set_title_widget = &adw::WindowTitle {
                        set_title: "Zerogrep",
                        #[watch]
                        set_subtitle: &model.current_file_label,
                    },
                },

                #[local_ref]
                menu_bar -> gtk::PopoverMenuBar {},

                gtk::Separator {
                    set_orientation: gtk::Orientation::Horizontal,
                },
                
                #[name = "split_paned"]
                gtk::Paned {
                    set_orientation: gtk::Orientation::Vertical,
                    set_position: 580,
                    set_vexpand: true,

                    #[wrap(Some)]
                    set_start_child = &gtk::ScrolledWindow {
                        set_vexpand: true,
                        set_hexpand: true,
                        #[local_ref]
                        list_view -> gtk::ListView {} 
                    },

                    #[wrap(Some)]
                    set_end_child = &gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_vexpand: false,

                        gtk::Label {
                            set_label: "<b>Active Filters</b>",
                            set_use_markup: true,
                            set_halign: gtk::Align::Start,
                            set_margin_all: 8,
                        },

                        gtk::ScrolledWindow {
                            set_vexpand: true,
                            set_min_content_height: 120,
                            
                            #[local_ref]
                            filter_list_box -> gtk::ListBox {
                                set_css_classes: &["rich-list"],
                                set_selection_mode: gtk::SelectionMode::None,
                            }
                        },
                    }
                },

                gtk::Separator {
                    set_orientation: gtk::Orientation::Horizontal,
                },

                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_spacing: 12,
                    set_margin_start: 10,
                    set_margin_end: 10,
                    set_margin_top: 4,
                    set_margin_bottom: 4,

                    gtk::Label {
                        #[watch]
                        set_label: &model.status_file,
                        #[watch]
                        set_tooltip_text: Some(&model.status_file),
                        set_selectable: true,
                        set_halign: gtk::Align::Start,
                        set_hexpand: true,
                        set_ellipsize: pango::EllipsizeMode::Start,
                        add_css_class: "caption",
                        add_css_class: "monospace",
                    },
                }
            }
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let session = session::load_session();

        let empty = std::env::temp_dir().join(format!("zerogrep-empty-{}", std::process::id()));
        std::fs::File::create(&empty).expect("Failed to create empty log buffer");
        let (mapped, offsets) =
            log_file::open_and_index(&empty).expect("Failed to map empty log buffer");
        let mut search_session = SearchSession::new(mapped, offsets);
        search_session.set_sort_order(session.descending);
        let search_session = Rc::new(RefCell::new(search_session));

        let filtered_store = ListStore::new::<BoxedAnyObject>();

        let selection_model = gtk::SingleSelection::new(Some(filtered_store.clone()));

        let factory = gtk::SignalListItemFactory::new();
        factory.connect_setup(move |_, obj| {
            let list_item = obj.downcast_ref::<gtk::ListItem>().unwrap();

            let row_box = gtk::Box::builder()
                .orientation(gtk::Orientation::Horizontal)
                .spacing(12)
                .build();

            let line_num_label = gtk::Label::builder()
                .halign(gtk::Align::End)
                .xalign(1.0)
                .width_chars(8)
                .selectable(false)
                .css_classes(["monospace", "dim-label"])
                .build();

            let text_label = gtk::Label::builder()
                .halign(gtk::Align::Start)
                .hexpand(true)
                .xalign(0.0)
                .css_classes(["monospace"])
                .build();

            row_box.append(&line_num_label);
            row_box.append(&text_label);
            list_item.set_child(Some(&row_box));
        });

        let session_clone = search_session.clone();
        factory.connect_bind(move |_, obj| {
            let list_item = obj
                .downcast_ref::<gtk::ListItem>()
                .expect("bind: row object is not a ListItem");
            let item = list_item
                .item()
                .expect("bind: row has no list item")
                .downcast::<BoxedAnyObject>()
                .expect("bind: store element is not a boxed LineEntry");
            let entry = *item.borrow::<LineEntry>();

            let session_borrow = session_clone.borrow();

            if let Some(range) = line_range(session_borrow.offsets(), entry.line_num) {
                let start = range.start;
                let end = range.end.saturating_sub(1);
                let raw = session_borrow.raw();
                let text = String::from_utf8_lossy(&raw[start..end]);
                let span = if std::str::from_utf8(&raw[start..end]).is_ok() {
                    entry.match_span
                } else {
                    None
                };

                let row_box = list_item
                    .child()
                    .expect("bind: row has no child widget")
                    .downcast::<gtk::Box>()
                    .expect("bind: row child is not a Box");
                let line_num_label = row_box
                    .first_child()
                    .expect("bind: row is missing the line-number label")
                    .downcast::<gtk::Label>()
                    .expect("bind: line-number slot is not a Label");
                let text_label = line_num_label
                    .next_sibling()
                    .expect("bind: row is missing the text label")
                    .downcast::<gtk::Label>()
                    .expect("bind: text slot is not a Label");

                line_num_label.set_text(&format!("{}", entry.line_num + 1));
                text_label.set_text(&text);
                text_label
                    .set_attributes(build_line_attrs(entry.style, &text, span, entry.context_color).as_ref());
            }
        });

        let list_view = gtk::ListView::new(Some(selection_model.clone()), Some(factory));

        let nav_ctrl = gtk::EventControllerKey::new();
        nav_ctrl.set_propagation_phase(gtk::PropagationPhase::Capture);
        let nav_sel = selection_model.clone();
        let nav_lv = list_view.clone();
        let nav_store = filtered_store.clone();
        nav_ctrl.connect_key_pressed(move |_, key, _, _| {
            let n = nav_store.n_items();
            let cur = nav_sel.selected();
            let target: Option<u32> = match key {
                gdk::Key::Down => {
                    if cur == u32::MAX {
                        if n > 0 {
                            Some(0)
                        } else {
                            None
                        }
                    } else if cur + 1 < n {
                        Some(cur + 1)
                    } else {
                        None
                    }
                }
                gdk::Key::Up => {
                    if cur != u32::MAX && cur > 0 {
                        Some(cur - 1)
                    } else {
                        None
                    }
                }
                _ => return glib::Propagation::Proceed,
            };
            match target {
                Some(t) => {
                    nav_sel.set_selected(t);
                    nav_lv.scroll_to(t, gtk::ListScrollFlags::NONE, None);
                    glib::Propagation::Stop
                }
                None => glib::Propagation::Proceed,
            }
        });
        list_view.add_controller(nav_ctrl);

        let filter_list_box = gtk::ListBox::default();

        let filters = FactoryVecDeque::builder()
            .launch(filter_list_box.clone())
            .forward(sender.input_sender(), |output| match output {
                FilterOutput::Remove(idx) => AppMsg::RemoveFilter(idx),
                FilterOutput::Toggle(idx, active) => AppMsg::ToggleFilter(idx, active),
                FilterOutput::Edit(idx, cfg) => AppMsg::OpenEditFilterDialog(idx, cfg),
                FilterOutput::Updated => AppMsg::TriggerSearch,
                FilterOutput::Navigate(idx, next) => AppMsg::NavigateFilter(idx, next),
            });

        let menu_bar = gtk::PopoverMenuBar::from_model(None::<&gio::Menu>);

        let mut model = ZerogrepApp {
            filters,
            filtered_store,
            selection_model,
            search_session,
            current_file_label: "No file opened".to_string(),
            main_window: root.clone(),
            split_paned: gtk::Paned::new(gtk::Orientation::Vertical),
            highlight_only: session.highlight_only,
            list_view: gtk::ListView::default(),
            menu_bar: gtk::PopoverMenuBar::from_model(None::<&gio::Menu>),
            recent: session.recent,
            status_file: String::new(),
        };

        for f in session.filters {
            let (kept, _warnings) =
                filter_set::validate(&filter_set::FilterSet {
                    name: String::new(),
                    filters: vec![f],
                });
            for f in kept {
                model.filters.guard().push_back(f);
            }
        }

        let widgets = view_output!();
        model.main_window = widgets.main_window.clone();
        model.split_paned = widgets.split_paned.clone();
        model.list_view = widgets.list_view.clone();
        model.menu_bar = widgets.menu_bar.clone();
        model.rebuild_menu();
        model.sync_action_state("toggle-sort", model.search_session.borrow().descending());
        model.sync_action_state("toggle-highlight", model.highlight_only);
        model.sync_filter_actions();

        register_window_actions(&widgets.main_window, sender.input_sender().clone());
        register_input_sender(sender.input_sender());

        let drop = gtk::DropTarget::new(glib::Type::STRING, gdk::DragAction::COPY);
        {
            let s = sender.input_sender().clone();
            drop.connect_drop(move |_target, value, _x, _y| {
                value
                    .get::<String>()
                    .ok()
                    .and_then(|uris| uris.split_whitespace().next().map(str::to_string))
                    .and_then(|uri| file_uri_to_path(&uri))
                    .map(|path| s.emit(AppMsg::LoadFile(path)))
                    .is_some()
            });
        }
        widgets.main_window.add_controller(drop);

        let mut files: Vec<PathBuf> = Vec::new();
        if let Some(p) = session.file.filter(|p| p.exists()) {
            files.push(p);
        }
        for p in take_pending_files() {
            if !files.contains(&p) {
                files.push(p);
            }
        }
        if files.is_empty() {
            sender.input(AppMsg::OpenFileDialog);
        }
        for p in files {
            sender.input(AppMsg::LoadFile(p));
        }

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            AppMsg::OpenFileDialog => {
                let dialog = gtk::FileDialog::builder()
                    .title("Open Log File")
                    .modal(true)
                    .build();

                let input_sender = sender.input_sender().clone();
                dialog.open(
                    Some(&self.main_window),
                    None::<&gio::Cancellable>,
                    move |res| {
                        if let Ok(file) = res
                            && let Some(path) = file.path()
                        {
                            input_sender.emit(AppMsg::LoadFile(path));
                        }
                    },
                );
            }
            AppMsg::SaveFilteredDialog => {
                let dialog = gtk::FileDialog::builder()
                    .title("Save Filtered Log Output")
                    .initial_name("filtered.log")
                    .modal(true)
                    .build();

                let input_sender = sender.input_sender().clone();
                dialog.save(
                    Some(&self.main_window),
                    None::<&gio::Cancellable>,
                    move |res| {
                        if let Ok(file) = res
                            && let Some(path) = file.path()
                        {
                            input_sender.emit(AppMsg::SaveFilteredToPath(path));
                        }
                    },
                );
            }
            AppMsg::SaveFilteredToPath(path) => {
                let session = self.search_session.borrow();
                let raw = session.raw().clone();
                let offsets = session.offsets().clone();
                let entries = session.results().to_vec();
                let descending = session.descending();
                let gen_id = session.generation();
                let input_sender = sender.input_sender().clone();

                relm4::tokio::task::spawn_blocking(move || {
                    let res = report::save_filtered(&path, &raw, &offsets, &entries, descending);
                    input_sender.emit(AppMsg::SaveFilteredDone(path, res, gen_id));
                });
            }
            AppMsg::SaveFilteredDone(path, res, gen_id) => {
                if !self.search_session.borrow().is_current(gen_id) {
                    if res.is_ok() {
                        let _ = std::fs::remove_file(&path);
                    }
                    self.status_file = format!(
                        "Save to {} discarded — the log changed while saving",
                        path.display()
                    );
                    return;
                }
                match res {
                    Ok(()) => {
                        self.status_file = format!("Saved filtered output to {}", path.display())
                    }
                    Err(e) => self.status_file = format!("Save failed: {e}"),
                }
            }
            AppMsg::LoadFile(path) => {
                let gen_id = self.search_session.borrow_mut().begin_load();
                self.current_file_label = format!("Indexing {}...", path.display());
                let input_sender = sender.input_sender().clone();

                relm4::tokio::task::spawn_blocking(move || {
                    match log_file::open_and_index(&path) {
                        Some((mapped, offsets)) => {
                            input_sender.emit(AppMsg::FileLoaded(path, mapped, offsets, gen_id))
                        }
                        None => input_sender.emit(AppMsg::FileLoadFailed(path, gen_id)),
                    }
                });
            }
            AppMsg::FileLoadFailed(path, gen_id) => {
                if !self.search_session.borrow().finish_load_failed(gen_id) {
                    return;
                }
                self.current_file_label = "No file opened".to_string();
                self.status_file = format!(
                    "Failed to open {} — check that the file exists and is readable",
                    path.display()
                );
            }
            AppMsg::FileLoaded(path, new_mmap, new_offsets, gen_id) => {
                if !self
                    .search_session
                    .borrow_mut()
                    .finish_load(path.clone(), new_mmap, new_offsets, gen_id)
                {
                    return;
                }
                let total_lines = self.search_session.borrow().total_lines();
                self.current_file_label = format!("{} ({} lines)", path.display(), total_lines);
                self.status_file = path.display().to_string();

                let mut recent = self.recent.clone();
                recent.retain(|p| p != &path);
                recent.insert(0, path);
                recent.truncate(10);
                self.recent = recent;
                self.rebuild_menu();
                self.sync_store();
                self.persist_session();

                sender.input(AppMsg::TriggerSearch);
            }
            AppMsg::OpenAddFilterDialog => {
                show_filter_dialog(&self.main_window, sender.input_sender().clone(), None);
            }
            AppMsg::OpenEditFilterDialog(index, config) => {
                show_filter_dialog(
                    &self.main_window,
                    sender.input_sender().clone(),
                    Some((index, config)),
                );
            }
            AppMsg::AddFilter(config) => {
                self.filters.guard().push_back(config);
                self.sync_filter_actions();
                sender.input(AppMsg::TriggerSearch);
            }
            AppMsg::UpdateFilter(index, config) => {
                let i = index.current_index();
                if i < self.filters.len() {
                    self.filters.send(i, FilterMsg::UpdateConfig(config));
                }
            }
            AppMsg::RemoveFilter(index) => {
                self.filters.guard().remove(index.current_index());
                self.sync_filter_actions();
                sender.input(AppMsg::TriggerSearch);
            }
            AppMsg::ToggleFilter(_index, _is_active) => {
                sender.input(AppMsg::TriggerSearch);
            }
            AppMsg::SetAllFiltersActive(active) => {
                let mut guard = self.filters.guard();
                for row in guard.iter_mut() {
                    row.data.active = active;
                }
                sender.input(AppMsg::TriggerSearch);
            }
            AppMsg::ClearAllFilters => {
                self.filters.guard().clear();
                self.sync_filter_actions();
                sender.input(AppMsg::TriggerSearch);
            }
            AppMsg::ExportFiltersDialog => {
                if self.filters.is_empty() {
                    self.status_file = "No filters to export".to_string();
                    return;
                }
                let dialog = gtk::FileDialog::builder()
                    .title("Export Filters")
                    .initial_name("filters.zerofilter")
                    .modal(true)
                    .build();

                let input_sender = sender.input_sender().clone();
                dialog.save(
                    Some(&self.main_window),
                    None::<&gio::Cancellable>,
                    move |res| {
                        if let Ok(file) = res
                            && let Some(path) = file.path()
                        {
                            input_sender.emit(AppMsg::ExportFiltersToPath(path));
                        }
                    },
                );
            }
            AppMsg::ExportFiltersToPath(path) => {
                let filters: Vec<FilterData> = self
                    .filters
                    .iter()
                    .map(|row| row.data.clone())
                    .collect();
                let set = filter_set::FilterSet {
                    name: "Zerogrep filters".to_string(),
                    filters,
                };
                let count = set.filters.len();
                self.status_file = format!("Exporting {count} filter(s) to {}...", path.display());
                let input_sender = sender.input_sender().clone();

                relm4::tokio::task::spawn_blocking(move || {
                    let res = filter_set::save(&path, &set);
                    input_sender.emit(AppMsg::ExportFiltersDone(path, count, res));
                });
            }
            AppMsg::ExportFiltersDone(path, count, res) => match res {
                Ok(()) => {
                    self.status_file =
                        format!("Exported {count} filter(s) to {}", path.display());
                }
                Err(e) => self.status_file = format!("Export failed: {e}"),
            },
            AppMsg::ImportFiltersDialog => {
                let dialog = gtk::FileDialog::builder()
                    .title("Import Filters")
                    .modal(true)
                    .build();

                let input_sender = sender.input_sender().clone();
                dialog.open(
                    Some(&self.main_window),
                    None::<&gio::Cancellable>,
                    move |res| {
                        if let Ok(file) = res
                            && let Some(path) = file.path()
                        {
                            input_sender.emit(AppMsg::ImportFiltersFile(path));
                        }
                    },
                );
            }
            AppMsg::ImportFiltersFile(path) => {
                self.status_file = format!("Importing filters from {}...", path.display());
                let input_sender = sender.input_sender().clone();

                relm4::tokio::task::spawn_blocking(move || {
                    let res = (|| -> Result<(String, Vec<FilterData>, Vec<String>), String> {
                        let set = filter_set::load(&path)?;
                        let (kept, warnings) = filter_set::validate(&set);
                        Ok((set.name, kept, warnings))
                    })();
                    input_sender.emit(AppMsg::ImportFiltersLoaded(res));
                });
            }
            AppMsg::ImportFiltersLoaded(res) => match res {
                Err(e) => self.status_file = format!("Import failed: {e}"),
                Ok((name, filters, warnings)) => {
                    if filters.is_empty() {
                        let detail = if warnings.is_empty() {
                            "no filters in file".to_string()
                        } else {
                            warnings.join("; ")
                        };
                        self.status_file = format!("Nothing imported from \"{name}\": {detail}");
                        return;
                    }
                    if self.filters.is_empty() {
                        sender.input(AppMsg::ApplyImportedFilters(filters, false, warnings));
                    } else {
                        let existing = self.filters.len();
                        let prompt = adw::MessageDialog::builder()
                            .transient_for(&self.main_window)
                            .modal(true)
                            .heading("Import Filters")
                            .body(format!(
                                "You already have {existing} filter(s). Import the {} filter(s) from \"{name}\" on top of them, or replace the current ones?",
                                filters.len()
                            ))
                            .default_response("append")
                            .close_response("cancel")
                            .build();
                        prompt.add_responses(&[
                            ("cancel", "Cancel"),
                            ("append", "Append"),
                            ("replace", "Replace"),
                        ]);
                        prompt.set_response_appearance(
                            "append",
                            adw::ResponseAppearance::Suggested,
                        );

                        let input_sender = sender.input_sender().clone();
                        let imported: Option<(Vec<FilterData>, Vec<String>)> =
                            Some((filters, warnings));
                        prompt.connect_response(None, move |_, resp| {
                            let replace = match resp {
                                "replace" => true,
                                "append" => false,
                                _ => return,
                            };
                            if let Some((f, w)) = imported.as_ref() {
                                input_sender.emit(AppMsg::ApplyImportedFilters(
                                    f.clone(),
                                    replace,
                                    w.clone(),
                                ));
                            }
                        });
                        prompt.present();
                    }
                }
            },
            AppMsg::ApplyImportedFilters(data, replace, warnings) => {
                let count = data.len();
                if replace {
                    self.filters.guard().clear();
                }
                for d in &data {
                    self.filters.guard().push_back(d.clone());
                }
                let detail = if warnings.is_empty() {
                    String::new()
                } else {
                    format!(" ({}: {})", warnings.len(), warnings.join("; "))
                };
                self.status_file =
                    format!("Imported {count} filter(s) from file{detail}");
                self.persist_session();
                self.sync_filter_actions();
                sender.input(AppMsg::TriggerSearch);
            }
            AppMsg::OpenRecent(path) => {
                sender.input(AppMsg::LoadFile(path));
            }
            AppMsg::ApplySet(name) => {
                let path = session::sets_dir().map(|d| d.join(format!("{name}.zerofilter")));
                match path {
                    Some(p) if p.exists() => sender.input(AppMsg::ImportFiltersFile(p)),
                    _ => self.status_file = format!("Filter set \"{name}\" not found"),
                }
            }
            AppMsg::SaveSetDialog => {
                if self.filters.is_empty() {
                    self.status_file = "No filters to save".to_string();
                    return;
                }
                let (dialog, content) = dialog_shell("Save Filter Set", &self.main_window, 380);

                let entry = gtk::Entry::builder()
                    .placeholder_text("Set name (e.g. nginx errors)")
                    .hexpand(true)
                    .build();
                content.append(&entry);

                let save_btn = dialog_action_row(&content, &dialog, "Save");

                let submit = {
                    let dialog = dialog.clone();
                    let entry = entry.clone();
                    let input_sender = sender.input_sender().clone();
                    move || {
                        let name = entry.text().trim().to_string();
                        if !name.is_empty() {
                            input_sender.emit(AppMsg::SaveSetToName(name));
                            dialog.close();
                        }
                    }
                };
                wire_submit(&save_btn, &entry, submit);

                dialog.present();
                entry.grab_focus();
            }
            AppMsg::SaveSetToName(name) => {
                let filters: Vec<FilterData> = self
                    .filters
                    .iter()
                    .map(|row| row.data.clone())
                    .collect();
                match session::save_set(&name, &filters) {
                    Ok(path) => {
                        self.status_file =
                            format!("Saved filter set \"{}\" to {}", name, path.display());
                        self.rebuild_menu();
                    }
                    Err(e) => self.status_file = format!("Failed to save filter set: {e}"),
                }
            }
            AppMsg::CopySelectedLine => {
                if let Some(obj) = self.selection_model.selected_item()
                    && let Ok(boxed) = obj.downcast::<BoxedAnyObject>()
                {
                    let entry = *boxed.borrow::<LineEntry>();
                    let session = self.search_session.borrow();
                    if let Some(range) = line_range(session.offsets(), entry.line_num) {
                        let start = range.start;
                        let end = range.end.saturating_sub(1);
                        let text = String::from_utf8_lossy(&session.raw()[start..end]);
                        if let Some(display) = gdk::Display::default() {
                            display.clipboard().set_text(&text);
                        }
                    }
                }
            }
            AppMsg::SetSortOrder(desc) => {
                if self.search_session.borrow_mut().set_sort_order(desc) {
                    self.sync_action_state("toggle-sort", desc);
                    self.sync_store();
                    self.persist_session();
                }
            }
            AppMsg::SetHighlightOnly(active) => {
                if self.highlight_only != active {
                    self.highlight_only = active;
                    self.sync_action_state("toggle-highlight", active);
                    self.persist_session();
                    sender.input(AppMsg::TriggerSearch);
                }
            }
            AppMsg::ResetSplitPane => {
                self.split_paned.set_position(580);
            }
            AppMsg::ShowAboutDialog => {
                crate::components::about::show_about(&self.main_window);
            }
            AppMsg::QuitApp => {
                self.main_window.close();
            }
            AppMsg::TriggerSearch => {
                let mut specs: Vec<crate::core::types::FilterSpec> = Vec::new();
                let mut row_spec: Vec<Option<usize>> = Vec::with_capacity(self.filters.len());
                for row in self.filters.iter() {
                    if row.data.active {
                        row_spec.push(Some(specs.len()));
                        specs.push(row.data.to_spec());
                    } else {
                        row_spec.push(None);
                    }
                }
                sender.input(AppMsg::RunSearch(specs, row_spec));
            }
            AppMsg::RunSearch(specs, row_spec) => {
                let gen_id = self.search_session.borrow_mut().begin_search();
                let session = self.search_session.borrow();
                let raw = session.raw().clone();
                let offsets = session.offsets().clone();
                let highlight_only = self.highlight_only;
                let input_sender = sender.input_sender().clone();

                relm4::tokio::task::spawn_blocking(move || {
                    let filters = rules::compile_filters(&specs);
                    let row_to_include: Vec<Option<usize>> = row_spec
                        .iter()
                        .map(|s| {
                            s.and_then(|si| filters.spec_to_include.get(si).copied().flatten())
                        })
                        .collect();
                    let out = search::run_search(&raw, &offsets, &filters, highlight_only);
                    input_sender.emit(AppMsg::ApplySearchResults(out, row_to_include, gen_id));
                });
            }
            AppMsg::ApplySearchResults(out, row_to_include, gen_id) => {
                let SearchOutput {
                    results,
                    matched_highlights,
                    include_matches,
                    elapsed_ms,
                } = out;
                if !self.search_session.borrow_mut().apply_results(
                    results,
                    include_matches,
                    row_to_include,
                    gen_id,
                ) {
                    return;
                }
                self.sync_store();
                let session = self.search_session.borrow();
                let visible_count = session.results().len();
                let total_lines = session.total_lines();

                for i in 0..self.filters.len() {
                    let avail = session
                        .row_to_include()
                        .get(i)
                        .and_then(|inc| inc.as_ref())
                        .and_then(|inc| session.include_matches().get(*inc))
                        .is_some_and(|m| !m.is_empty());
                    self.filters.send(i, FilterMsg::SetNavAvailable(avail));
                }

                if let Some(path) = session.path() {
                    if self.highlight_only && matched_highlights > 0 {
                        self.current_file_label = format!(
                            "{} ({} / {} visible, {} highlighted, {} ms)",
                            path.display(),
                            visible_count,
                            total_lines,
                            matched_highlights,
                            elapsed_ms
                        );
                    } else {
                        self.current_file_label = format!(
                            "{} ({} / {} lines, {} ms)",
                            path.display(),
                            visible_count,
                            total_lines,
                            elapsed_ms
                        );
                    }
                }

                self.persist_session();
            }
            AppMsg::NavigateFilter(row, next) => {
                let session = self.search_session.borrow();
                let inc = match session
                    .row_to_include()
                    .get(row.current_index())
                    .copied()
                    .flatten()
                {
                    Some(inc) => inc,
                    None => return,
                };
                let matches = match session.include_matches().get(inc) {
                    Some(m) if !m.is_empty() => m,
                    _ => return,
                };
                if self.filtered_store.n_items() == 0 {
                    return;
                }
                let cur = match self.selection_model.selected_item() {
                    Some(obj) => obj
                        .downcast::<BoxedAnyObject>()
                        .ok()
                        .map(|b| b.borrow::<LineEntry>().line_num),
                    None => None,
                }
                .or_else(|| {
                    self.filtered_store
                        .item(0)
                        .and_then(|i| i.downcast::<BoxedAnyObject>().ok())
                        .map(|b| b.borrow::<LineEntry>().line_num)
                });
                let cur = match cur {
                    Some(c) => c,
                    None => return,
                };

                let n = matches.len();
                let mut idx = nav_start_index(matches, cur, next).unwrap();

                for _ in 0..n {
                    let cand = matches[idx];
                    if let Some(&pos) = session.line_pos().get(&cand) {
                        self.selection_model.set_selected(pos as u32);
                        self.list_view
                            .scroll_to(pos as u32, gtk::ListScrollFlags::NONE, None);
                        return;
                    }
                    idx = if next {
                        (idx + 1) % n
                    } else {
                        (idx + n - 1) % n
                    };
                }
            }
        }
    }
}

#[cfg(test)]
mod nav_tests {
    use super::nav_start_index;

    #[test]
    fn next_from_last_wraps_to_first() {
        let m = vec![1, 5, 9, 20];
        assert_eq!(nav_start_index(&m, 20, true), Some(0));
    }

    #[test]
    fn next_past_every_match_wraps_to_first() {
        let m = vec![1, 5, 9];
        assert_eq!(nav_start_index(&m, 99, true), Some(0));
    }

    #[test]
    fn prev_from_first_wraps_to_last() {
        let m = vec![1, 5, 9, 20];
        assert_eq!(nav_start_index(&m, 1, false), Some(3));
    }

    #[test]
    fn prev_before_every_match_wraps_to_last() {
        let m = vec![5, 9, 20];
        assert_eq!(nav_start_index(&m, 0, false), Some(2));
    }

    #[test]
    fn plain_steps() {
        let m = vec![1, 5, 9, 20];
        assert_eq!(nav_start_index(&m, 1, true), Some(1)); 
        assert_eq!(nav_start_index(&m, 5, true), Some(2));
        assert_eq!(nav_start_index(&m, 6, true), Some(2)); 
        assert_eq!(nav_start_index(&m, 6, false), Some(1)); 
        assert_eq!(nav_start_index(&m, 20, false), Some(2));
    }

    #[test]
    fn empty_matches_none() {
        assert_eq!(nav_start_index(&[], 0, true), None);
    }
}
