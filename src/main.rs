mod card;
mod config;
mod convert;
mod formats;
mod i18n;
mod media;
mod prefs;
mod tools;
mod window;

use adw::prelude::*;
use gtk::{gio, glib};
use std::cell::RefCell;
use std::rc::Rc;

const APP_ID: &str = "io.github.wioletowa.Morph";

fn main() -> glib::ExitCode {
    let lang = config::Config::load().lang;
    if lang == "en" || lang == "ru" {
        // So GTK and libadwaita's own strings follow the chosen language too.
        // SAFETY: no other threads exist yet.
        unsafe { std::env::set_var("LANGUAGE", &lang) };
    }
    i18n::init(&lang);
    let app = adw::Application::builder().application_id(APP_ID).flags(gio::ApplicationFlags::HANDLES_OPEN).build();

    let win: Rc<RefCell<Option<Rc<window::App>>>> = Rc::default();
    let get = {
        let win = win.clone();
        move |app: &adw::Application| -> Rc<window::App> {
            if let Some(w) = win.borrow().as_ref() {
                return w.clone();
            }
            let w = window::App::new(app);
            *win.borrow_mut() = Some(w.clone());
            w
        }
    };

    let g = get.clone();
    app.connect_activate(move |app| {
        let w = g(app);
        w.window.present();
        #[cfg(debug_assertions)]
        if let Ok(script) = std::env::var("MORPH_TEST") {
            test::run(w, script);
        }
    });

    app.connect_open(move |app, files, _| {
        let w = get(app);
        w.add_paths(files.iter().filter_map(|f| f.path()).collect());
        w.window.present();
        #[cfg(debug_assertions)]
        if let Ok(script) = std::env::var("MORPH_TEST") {
            test::run(w, script);
        }
    });

    app.run()
}

/// Debug-build driver: MORPH_TEST="add:/a.png,/b.jpg;target:0:avif;convert;idle;dump;shot:/x.png;quit".
#[cfg(debug_assertions)]
mod test {
    use crate::window::App;
    use adw::prelude::*;
    use gtk::glib;
    use std::path::PathBuf;
    use std::rc::Rc;

    pub fn run(app: Rc<App>, script: String) {
        let steps: Vec<String> = script.split(';').map(String::from).collect();
        glib::timeout_add_local_once(std::time::Duration::from_millis(500), move || step(app, steps, 0));
    }

    fn step(app: Rc<App>, steps: Vec<String>, i: usize) {
        let Some(s) = steps.get(i) else { return };
        let (cmd, arg) = s.split_once(':').unwrap_or((s, ""));
        let mut delay = 200;
        let mut next = i + 1;
        match cmd {
            "add" => app.add_paths(arg.split(',').map(PathBuf::from).collect()),
            "target" => {
                let (n, ext) = arg.split_once(':').unwrap_or(("0", arg));
                app.debug_target(n.parse().unwrap_or(0), ext);
            }
            "action" => {
                let (name, param) = match arg.split_once('=') {
                    Some((n, p)) => (n, Some(p.to_variant())),
                    None => (arg, None),
                };
                let _ = WidgetExt::activate_action(&app.window, name, param.as_ref());
            }
            "convert" => {
                let _ = WidgetExt::activate_action(&app.window, "win.convert", None);
            }
            "cancel" => app.debug_cancel(arg.parse().unwrap_or(0)),
            "wait" => delay = arg.parse().unwrap_or(500),
            "idle" => {
                if !app.idle() {
                    next = i;
                }
                delay = 100;
            }
            "dump" => println!("--- state\n{}", app.debug_state()),
            "size" => {
                let (w, h) = arg.split_once('x').unwrap_or(("760", "620"));
                app.window.set_default_size(w.parse().unwrap_or(760), h.parse().unwrap_or(620));
            }
            "shot" => {
                // Dialogs are separate surfaces; shoot the topmost window.
                let target: gtk::Widget = gtk::Window::list_toplevels()
                    .into_iter()
                    .filter_map(|w| w.downcast::<gtk::Window>().ok())
                    .find(|w| w.is_visible() && w.is_active())
                    .map(|w| w.upcast())
                    .unwrap_or_else(|| app.window.clone().upcast());
                let paintable = gtk::WidgetPaintable::new(Some(&target));
                let snap = gtk::Snapshot::new();
                paintable.snapshot(&snap, target.width() as f64, target.height() as f64);
                if let (Some(node), Some(r)) = (snap.to_node(), target.native().and_then(|n| n.renderer())) {
                    let _ = r.render_texture(&node, None).save_to_png(arg);
                }
            }
            "quit" => {
                // An open AdwDialog swallows the first close request.
                app.window.close();
                if let Some(a) = app.window.application() {
                    a.quit();
                }
                return;
            }
            _ => eprintln!("MORPH_TEST: unknown step {s}"),
        }
        glib::timeout_add_local_once(std::time::Duration::from_millis(delay), move || step(app, steps, next));
    }
}
