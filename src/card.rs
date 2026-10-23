//! One file in the queue: its row widgets and conversion state.

use crate::convert::human_size;
use crate::formats::{self, Kind};
use crate::i18n::tr;
use adw::prelude::*;
use gtk::{gio, glib};
use std::cell::{Cell, RefCell};
use std::path::PathBuf;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum State {
    Idle,
    Queued,
    Running,
    Done,
    Failed,
}

pub struct Item {
    pub id: u64,
    pub path: PathBuf,
    pub kind: Kind,
    pub size: u64,
    pub row: gtk::ListBoxRow,
    pub icon: gtk::Image,
    pub subtitle: gtk::Label,
    pub target: gtk::DropDown,
    pub spinner: adw::Spinner,
    pub done_icon: gtk::Image,
    pub open_btn: gtk::Button,
    pub remove_btn: gtk::Button,
    pub state: Cell<State>,
    pub cancelled: Cell<bool>,
    pub output: RefCell<Option<PathBuf>>,
    pub proc: RefCell<Option<gio::Subprocess>>,
    targets: &'static [&'static str],
}

impl Item {
    pub fn new(id: u64, path: PathBuf, kind: Kind, target: &str) -> Self {
        let size = path.metadata().map_or(0, |m| m.len());
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let targets = formats::targets(kind);

        let icon = gtk::Image::builder().pixel_size(48).css_classes(["morph-thumb"]).build();
        let (ctype, _) = gio::content_type_guess(Some(&path), None);
        icon.set_from_gicon(&gio::content_type_get_icon(&ctype));

        let title = gtk::Label::builder()
            .label(&name)
            .xalign(0.0)
            .ellipsize(gtk::pango::EllipsizeMode::Middle)
            .css_classes(["heading"])
            .tooltip_text(path.to_string_lossy().as_ref())
            .build();
        let subtitle = gtk::Label::builder()
            .xalign(0.0)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .css_classes(["dim-label", "caption"])
            .build();
        let text = gtk::Box::builder().orientation(gtk::Orientation::Vertical).spacing(2).hexpand(true).valign(gtk::Align::Center).build();
        text.append(&title);
        text.append(&subtitle);

        let labels: Vec<String> = targets.iter().map(|t| formats::label(t)).collect();
        let labels: Vec<&str> = labels.iter().map(String::as_str).collect();
        let target_dd = gtk::DropDown::from_strings(&labels);
        target_dd.set_valign(gtk::Align::Center);
        target_dd.set_width_request(112);
        target_dd.set_tooltip_text(Some(tr("Convert to")));

        let spinner = adw::Spinner::builder().visible(false).width_request(20).height_request(20).build();
        let done_icon = gtk::Image::builder().icon_name("object-select-symbolic").visible(false).css_classes(["success"]).build();
        let open_btn = gtk::Button::builder()
            .icon_name("folder-open-symbolic")
            .tooltip_text(tr("Show in folder"))
            .valign(gtk::Align::Center)
            .visible(false)
            .css_classes(["flat", "circular"])
            .build();
        let remove_btn = gtk::Button::builder()
            .icon_name("window-close-symbolic")
            .tooltip_text(tr("Remove"))
            .valign(gtk::Align::Center)
            .css_classes(["flat", "circular"])
            .build();

        let arrow = gtk::Image::builder().icon_name("go-next-symbolic").css_classes(["dim-label"]).build();
        let hbox = gtk::Box::builder().spacing(12).margin_top(8).margin_bottom(8).margin_start(10).margin_end(8).build();
        for w in [icon.upcast_ref::<gtk::Widget>(), text.upcast_ref(), spinner.upcast_ref(), done_icon.upcast_ref(), arrow.upcast_ref(), target_dd.upcast_ref(), open_btn.upcast_ref(), remove_btn.upcast_ref()] {
            hbox.append(w);
        }
        let row = gtk::ListBoxRow::builder().child(&hbox).activatable(false).selectable(false).build();

        let item = Item {
            id,
            path,
            kind,
            size,
            row,
            icon,
            subtitle,
            target: target_dd,
            spinner,
            done_icon,
            open_btn,
            remove_btn,
            state: Cell::new(State::Idle),
            cancelled: Cell::new(false),
            output: RefCell::new(None),
            proc: RefCell::new(None),
            targets,
        };
        let src = formats::canonical(&formats::ext_of(&item.path)).to_string();
        if !item.set_target(target) || item.target() == src {
            // Converting to the same format is allowed, but not a useful default.
            let other = targets.iter().position(|t| *t != src).unwrap_or(0);
            item.target.set_selected(other as u32);
        }
        item.set_state(State::Idle);
        item
    }

    pub fn target(&self) -> &'static str {
        self.targets.get(self.target.selected() as usize).copied().unwrap_or(self.targets[0])
    }

    /// Returns false when this kind of file cannot become `ext`.
    pub fn set_target(&self, ext: &str) -> bool {
        match self.targets.iter().position(|t| *t == ext) {
            Some(i) => {
                self.target.set_selected(i as u32);
                true
            }
            None => false,
        }
    }

    pub fn busy(&self) -> bool {
        matches!(self.state.get(), State::Queued | State::Running)
    }

    pub fn set_state(&self, s: State) {
        self.state.set(s);
        self.spinner.set_visible(s == State::Running);
        self.done_icon.set_visible(s == State::Done);
        self.open_btn.set_visible(s == State::Done);
        self.target.set_sensitive(!self.busy());
        self.subtitle.remove_css_class("error");
        self.subtitle.set_tooltip_text(None);
        let running = s == State::Running;
        self.remove_btn.set_icon_name(if running { "process-stop-symbolic" } else { "window-close-symbolic" });
        self.remove_btn.set_tooltip_text(Some(if running { tr("Cancel") } else { tr("Remove") }));
        let src = formats::label(&formats::ext_of(&self.path));
        let base = format!("{src} · {}", human_size(self.size));
        match s {
            State::Idle => self.subtitle.set_label(&base),
            State::Queued => self.subtitle.set_label(&format!("{base} · {}", tr("Waiting…"))),
            State::Running => self.subtitle.set_label(tr("Converting…")),
            State::Done => {
                let out = self.output.borrow();
                let Some(out) = out.as_ref() else { return };
                let name = out.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                let new = out.metadata().map_or(0, |m| m.len());
                let pct = if self.size > 0 { (new as f64 / self.size as f64 - 1.0) * 100.0 } else { 0.0 };
                let sign = if pct > 0.0 { "+" } else { "−" };
                self.subtitle.set_label(&format!("{name} · {} ({sign}{:.0}%)", human_size(new), pct.abs()));
                self.subtitle.set_tooltip_text(Some(&out.to_string_lossy()));
            }
            State::Failed => {}
        }
    }

    pub fn fail(&self, msg: &str) {
        self.set_state(State::Failed);
        // Converters quote the full input path; the row already shows the name.
        let name = self.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let msg = msg.replace(&*self.path.to_string_lossy(), &name);
        let first = msg.lines().next().unwrap_or(&msg);
        self.subtitle.set_label(&format!("{}: {first}", tr("Failed")));
        self.subtitle.add_css_class("error");
        self.subtitle.set_tooltip_text(Some(&msg));
    }

    pub fn cancel(&self) {
        match self.state.get() {
            State::Queued => self.set_state(State::Idle),
            State::Running => {
                self.cancelled.set(true);
                if let Some(p) = self.proc.borrow().as_ref() {
                    p.force_exit();
                }
            }
            _ => {}
        }
    }

    pub fn set_thumbnail(&self, file: &std::path::Path) {
        match gtk::gdk::Texture::from_filename(file) {
            Ok(t) => self.icon.set_paintable(Some(&t)),
            Err(e) => glib::g_debug!("morph", "thumbnail {}: {e}", file.display()),
        }
    }
}
