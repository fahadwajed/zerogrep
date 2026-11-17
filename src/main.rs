mod app;
mod components;
mod core;

use gtk::prelude::*;
use relm4::RelmApp;

fn main() {
    let application = relm4::main_application();
    application.set_application_id(Some("com.zerogrep.app"));

    application.connect_open(move |_app, files, _count| {
        for file in files {
            if let Some(path) = file.path() {
                app::forward_file(path);
            }
        }
    });

    RelmApp::from_app(application)
        .with_args(std::env::args().skip(1).collect())
        .run::<app::ZerogrepApp>(());
}
