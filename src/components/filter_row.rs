use gtk::gdk;
use relm4::factory::{DynamicIndex, FactoryComponent, FactorySender};
use relm4::gtk;
use relm4::gtk::prelude::*;
use relm4::RelmWidgetExt;

use super::{build_pango_attrs};
use crate::core::filter_set::FilterData;

#[derive(Debug)]
pub struct FilterRow {
    pub data: FilterData,
    nav_available: bool,
    index: DynamicIndex,
}

#[derive(Debug)]
pub enum FilterOutput {
    Remove(DynamicIndex),
    Toggle(DynamicIndex, bool),
    Edit(DynamicIndex, FilterData),
    Updated,
    Navigate(DynamicIndex, bool),
}

#[derive(Debug)]
pub enum FilterMsg {
    Remove,
    Toggle(bool),
    RequestEdit,
    UpdateConfig(FilterData),
    SetNavAvailable(bool),
    Navigate(bool),
}

#[relm4::factory(pub)]
impl FactoryComponent for FilterRow {
    type Init = FilterData;
    type Input = FilterMsg;
    type Output = FilterOutput;
    type CommandOutput = ();
    type ParentWidget = gtk::ListBox;

    view! {
        gtk::ListBoxRow {
            gtk::Box {
                set_orientation: gtk::Orientation::Horizontal,
                set_spacing: 12,
                set_margin_all: 6,
                set_tooltip_text: Some("Double-click to edit filter"),

                add_controller = gtk::GestureClick {
                    set_button: gdk::BUTTON_PRIMARY,
                    connect_pressed[sender] => move |gesture, n_press, _, _| {
                        if n_press == 2 {
                            gesture.set_state(gtk::EventSequenceState::Claimed);
                            sender.input(FilterMsg::RequestEdit);
                        }
                    }
                },
                
                gtk::CheckButton {
                    #[watch]
                    set_active: self.data.active,
                    connect_toggled[sender] => move |btn| {
                        sender.input(FilterMsg::Toggle(btn.is_active()));
                    }
                },
                gtk::Label {
                    #[watch]
                    set_label: if self.data.mode == "Exclude" {
                        " EXCLUDE "
                    } else {
                        " INCLUDE "
                    },
                    add_css_class: "monospace",
                    #[watch]
                    set_attributes: build_pango_attrs(self.data.style()).as_ref(),
                },
                gtk::Label {
                    #[watch]
                    set_label: &if self.data.case_insensitive {
                        self.data.filter_type.clone()
                    } else {
                        format!("{} (s)", self.data.filter_type)
                    },
                },
                gtk::Label {
                    #[watch]
                    set_label: &self.data.mode,
                },
                gtk::Label {
                    #[watch]
                    set_label: &format!("\"{}\"", self.data.query),
                    add_css_class: "monospace",
                    set_hexpand: true,
                    set_halign: gtk::Align::Start,
                },
                gtk::Label {
                    #[watch]
                    set_visible: !self.data.context.is_absent(),
                    #[watch]
                    set_label: &match self.data.context {
                        crate::core::types::ContextSpec::None => String::new(),
                        crate::core::types::ContextSpec::Before(n) => format!("\u{2191}{n}"),
                        crate::core::types::ContextSpec::After(n) => format!("\u{2193}{n}"),
                        crate::core::types::ContextSpec::Both(n) => format!("\u{00b1}{n}"),
                    },
                    add_css_class: "monospace",
                    add_css_class: "dim-label",
                    set_tooltip_text: Some("Context lines around matches (edit the filter to change)"),
                },
                gtk::Button {
                    #[watch]
                    set_visible: self.nav_available,
                    set_icon_name: "go-previous-symbolic",
                    set_tooltip_text: Some("Previous matching line"),
                    set_css_classes: &["flat"],
                    connect_clicked[sender] => move |_| {
                        sender.input(FilterMsg::Navigate(false));
                    }
                },
                gtk::Button {
                    #[watch]
                    set_visible: self.nav_available,
                    set_icon_name: "go-next-symbolic",
                    set_tooltip_text: Some("Next matching line"),
                    set_css_classes: &["flat"],
                    connect_clicked[sender] => move |_| {
                        sender.input(FilterMsg::Navigate(true));
                    }
                },
                gtk::Button {
                    set_icon_name: "document-edit-symbolic",
                    set_tooltip_text: Some("Edit Filter"),
                    set_css_classes: &["flat"],
                    connect_clicked[sender] => move |_| {
                        sender.input(FilterMsg::RequestEdit);
                    }
                },
                gtk::Button {
                    set_icon_name: "user-trash-symbolic",
                    set_tooltip_text: Some("Remove Filter"),
                    set_css_classes: &["destructive-action", "flat"],
                    connect_clicked[sender] => move |_| {
                        sender.input(FilterMsg::Remove);
                    }
                }
            }
        }
    }

    fn init_model(
        data: Self::Init,
        index: &DynamicIndex,
        _sender: FactorySender<Self>,
    ) -> Self {
        Self {
            data,
            nav_available: false,
            index: index.clone(),
        }
    }

    fn update(&mut self, msg: Self::Input, sender: FactorySender<Self>) {
        match msg {
            FilterMsg::Remove => {
                let _ = sender.output(FilterOutput::Remove(self.index.clone()));
            }
            FilterMsg::Toggle(active) => {
                if self.data.active != active {
                    self.data.active = active;
                    let _ = sender.output(FilterOutput::Toggle(self.index.clone(), active));
                }
            }
            FilterMsg::RequestEdit => {
                let _ = sender.output(FilterOutput::Edit(self.index.clone(), self.data.clone()));
            }
            FilterMsg::UpdateConfig(new_config) => {
                let active = self.data.active;
                self.data = new_config;
                self.data.active = active;
                let _ = sender.output(FilterOutput::Updated);
            }
            FilterMsg::Navigate(next) => {
                let _ = sender.output(FilterOutput::Navigate(self.index.clone(), next));
            }
            FilterMsg::SetNavAvailable(avail) => {
                self.nav_available = avail;
            }
        }
    }
}
