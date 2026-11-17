
use relm4::adw::prelude::*;
use relm4::{adw, gtk};

const ICON_SVG: &str = include_str!("../../assets/zerogrep-adaptive.svg");
static ICON_READY: std::sync::OnceLock<()> = std::sync::OnceLock::new();

fn register_icon() {
    ICON_READY.get_or_init(|| {
        let base = std::env::temp_dir()
            .join(format!("zerogrep-icons-{}", std::process::id()));
        let dir = base.join("hicolor/scalable/apps");
        std::fs::create_dir_all(&dir).expect("failed to create icon dir");
        std::fs::write(dir.join("com.zerogrep.app.svg"), ICON_SVG)
            .expect("failed to write icon");
        if let Some(display) = gtk::gdk::Display::default() {
            gtk::IconTheme::for_display(&display)
                .add_search_path(base.to_string_lossy().as_ref());
        }
    });
}

fn icon_image(size: i32) -> gtk::Image {
    register_icon();
    let image = gtk::Image::from_icon_name("com.zerogrep.app");
    image.set_pixel_size(size);
    image
}

pub fn show_about(parent: &impl IsA<gtk::Window>) {
    let window = adw::Window::builder()
        .title("About Zerogrep")
        .transient_for(parent)
        .modal(true)
        .default_width(440)
        .build();

    let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
    root.append(&adw::HeaderBar::new());

    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(10)
        .margin_top(24)
        .margin_bottom(24)
        .margin_start(24)
        .margin_end(24)
        .build();

    let icon = icon_image(90);
    icon.set_halign(gtk::Align::Center);
    icon.set_margin_bottom(6);
    content.append(&icon);

    let title = gtk::Label::builder()
        .label("Zerogrep")
        .css_classes(["title-1"])
        .halign(gtk::Align::Center)
        .build();
    content.append(&title);

    let version = gtk::Label::builder()
        .label(format!("Version {}", env!("CARGO_PKG_VERSION")))
        .css_classes(["dim-label", "caption"])
        .halign(gtk::Align::Center)
        .build();
    content.append(&version);

    let tagline = gtk::Label::builder()
        .label("High-performance, zero-copy log diagnostics for Linux.")
        .css_classes(["dim-label"])
        .halign(gtk::Align::Center)
        .wrap(true)
        .justify(gtk::Justification::Center)
        .build();
    content.append(&tagline);

    let legal = gtk::Label::builder()
        .label("Copyright \u{00a9} 2026 Fahad Wajed \u{00b7} Licensed under Apache-2.0")
        .css_classes(["caption", "dim-label"])
        .halign(gtk::Align::Center)
        .margin_top(14)
        .build();
    content.append(&legal);

    let close = gtk::Button::builder()
        .label("Close")
        .css_classes(["pill"])
        .halign(gtk::Align::Center)
        .margin_top(10)
        .build();
    {
        let window = window.clone();
        close.connect_clicked(move |_| window.close());
    }
    content.append(&close);

    root.append(&content);
    window.set_content(Some(&root));

    window.present();
}
