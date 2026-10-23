//! Main window: drop zone, file list, conversion queue.

use crate::card::{Item, State};
use crate::config::{self, Config};
use crate::convert::{self, ImageOptions};
use crate::formats::{self, Kind};
use crate::i18n::tr;
use crate::tools::Tools;
use adw::prelude::*;
use gtk::{gdk, gio, glib};
use std::cell::{Cell, RefCell};
use std::collections::{HashSet, VecDeque};
use std::ffi::{OsStr, OsString};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::rc::{Rc, Weak};

const THUMB_SIZE: u32 = 96;
const THUMB_JOBS: usize = 2;

pub struct App {
    pub window: adw::ApplicationWindow,
    toasts: adw::ToastOverlay,
    stack: gtk::Stack,
    list: gtk::ListBox,
    count: gtk::Label,
    convert_btn: gtk::Button,
    clear_btn: gtk::Button,
    set_all: gtk::MenuButton,
    pub config: RefCell<Config>,
    pub tools: Tools,
    items: RefCell<Vec<Rc<Item>>>,
    next_id: Cell<u64>,
    running: Cell<usize>,
    /// Output paths claimed by running jobs, so two inputs never race for one name.
    reserved: RefCell<HashSet<PathBuf>>,
    thumbs: RefCell<VecDeque<Weak<Item>>>,
    thumbs_running: Cell<usize>,
}

impl App {
    pub fn new(app: &adw::Application) -> Rc<Self> {
        let config = Config::load();
        let tools = Tools::detect();

        let window = adw::ApplicationWindow::builder()
            .application(app)
            .title("Morph")
            .default_width(config.width)
            .default_height(config.height)
            .width_request(360)
            .height_request(400)
            .build();

        let header = adw::HeaderBar::new();
        let add_btn = gtk::Button::builder().icon_name("list-add-symbolic").tooltip_text(tr("Add Files")).build();
        add_btn.set_action_name(Some("win.add"));
        header.pack_start(&add_btn);
        let menu = gio::Menu::new();
        menu.append(Some(tr("Preferences")), Some("win.preferences"));
        menu.append(Some(tr("About Morph")), Some("win.about"));
        let menu_btn = gtk::MenuButton::builder().icon_name("open-menu-symbolic").menu_model(&menu).tooltip_text(tr("Main Menu")).build();
        header.pack_end(&menu_btn);
        let clear_btn = gtk::Button::builder().icon_name("edit-clear-all-symbolic").tooltip_text(tr("Clear List")).build();
        clear_btn.set_action_name(Some("win.clear"));
        header.pack_end(&clear_btn);

        let choose = gtk::Button::builder().label(tr("Choose Files…")).halign(gtk::Align::Center).css_classes(["pill", "suggested-action"]).build();
        choose.set_action_name(Some("win.add"));
        let description = if tools.vips.is_some() {
            tr("Files are converted on this computer. Nothing is uploaded.").to_string()
        } else {
            tr("vips is not installed, so images cannot be converted. Install the libvips package.").to_string()
        };
        let empty = adw::StatusPage::builder()
            .icon_name("document-send-symbolic")
            .title(tr("Drop Files to Convert"))
            .description(&description)
            .child(&choose)
            .build();

        let list = gtk::ListBox::builder().selection_mode(gtk::SelectionMode::None).css_classes(["boxed-list"]).valign(gtk::Align::Start).build();
        let clamp = adw::Clamp::builder().maximum_size(860).child(&list).margin_top(12).margin_bottom(12).margin_start(12).margin_end(12).build();
        let scroller = gtk::ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::Never).child(&clamp).vexpand(true).build();

        let set_all = gtk::MenuButton::builder().label(tr("Set All To")).always_show_arrow(true).build();
        let count = gtk::Label::builder().hexpand(true).xalign(1.0).css_classes(["dim-label"]).build();
        let convert_btn = gtk::Button::builder().label(tr("Convert All")).css_classes(["pill", "suggested-action"]).build();
        convert_btn.set_action_name(Some("win.convert"));
        let bottom = gtk::Box::builder().spacing(12).margin_top(8).margin_bottom(8).margin_start(12).margin_end(12).build();
        bottom.append(&set_all);
        bottom.append(&count);
        bottom.append(&convert_btn);
        let list_page = gtk::Box::new(gtk::Orientation::Vertical, 0);
        list_page.append(&scroller);
        list_page.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        list_page.append(&bottom);

        let stack = gtk::Stack::builder().transition_type(gtk::StackTransitionType::Crossfade).build();
        stack.add_named(&empty, Some("empty"));
        stack.add_named(&list_page, Some("list"));

        let toasts = adw::ToastOverlay::new();
        toasts.set_child(Some(&stack));
        let view = adw::ToolbarView::builder().content(&toasts).css_classes(["morph-drop"]).build();
        view.add_top_bar(&header);
        window.set_content(Some(&view));

        let this = Rc::new(App {
            window,
            toasts,
            stack,
            list,
            count,
            convert_btn,
            clear_btn,
            set_all,
            config: RefCell::new(config),
            tools,
            items: RefCell::new(Vec::new()),
            next_id: Cell::new(1),
            running: Cell::new(0),
            reserved: RefCell::new(HashSet::new()),
            thumbs: RefCell::new(VecDeque::new()),
            thumbs_running: Cell::new(0),
        });
        this.load_css();
        this.setup_actions();
        this.setup_drop(&view);
        this.build_set_all_menu();
        this.update();

        let w = Rc::downgrade(&this);
        this.window.connect_close_request(move |win| {
            if let Some(a) = w.upgrade() {
                for it in a.items.borrow().iter() {
                    it.cancel();
                }
                let mut c = a.config.borrow_mut();
                if !win.is_maximized() {
                    (c.width, c.height) = win.default_size();
                }
                c.save();
            }
            glib::Propagation::Proceed
        });
        this
    }

    fn load_css(&self) {
        let css = gtk::CssProvider::new();
        css.load_from_string(
            ".morph-drop:drop(active) { box-shadow: inset 0 0 0 3px alpha(var(--accent-color), 0.6); }
             .morph-thumb { border-radius: 6px; }",
        );
        gtk::style_context_add_provider_for_display(&WidgetExt::display(&self.window), &css, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
    }

    fn setup_actions(self: &Rc<Self>) {
        let add = |name: &str, f: Box<dyn Fn(&Rc<App>, Option<&glib::Variant>)>, ty: Option<&glib::VariantTy>| {
            let a = gio::SimpleAction::new(name, ty);
            let w = Rc::downgrade(self);
            a.connect_activate(move |_, p| {
                if let Some(app) = w.upgrade() {
                    f(&app, p);
                }
            });
            gio::prelude::ActionMapExt::add_action(&self.window, &a);
        };
        add("add", Box::new(|a, _| a.choose_files()), None);
        add("clear", Box::new(|a, _| a.clear()), None);
        add("convert", Box::new(|a, _| a.convert_all()), None);
        add("preferences", Box::new(|a, _| crate::prefs::show(a)), None);
        add("about", Box::new(|a, _| a.about()), None);
        add(
            "set-all",
            Box::new(|a, p| {
                if let Some(ext) = p.and_then(|p| p.get::<String>()) {
                    a.set_all(&ext);
                }
            }),
            Some(glib::VariantTy::STRING),
        );
    }

    fn setup_drop(self: &Rc<Self>, target: &impl IsA<gtk::Widget>) {
        let drop = gtk::DropTarget::new(gdk::FileList::static_type(), gdk::DragAction::COPY);
        let w = Rc::downgrade(self);
        drop.connect_drop(move |_, value, _, _| {
            let (Some(app), Ok(list)) = (w.upgrade(), value.get::<gdk::FileList>()) else { return false };
            app.add_paths(list.files().iter().filter_map(|f| f.path()).collect());
            true
        });
        target.add_controller(drop);
    }

    fn build_set_all_menu(&self) {
        let menu = gio::Menu::new();
        for t in formats::targets(Kind::Image) {
            let item = gio::MenuItem::new(Some(&formats::label(t)), None);
            item.set_action_and_target_value(Some("win.set-all"), Some(&t.to_variant()));
            menu.append_item(&item);
        }
        self.set_all.set_menu_model(Some(&menu));
    }

    fn choose_files(self: &Rc<Self>) {
        let filter = gtk::FileFilter::new();
        filter.set_name(Some(tr("Supported Files")));
        for ext in formats::all_inputs() {
            filter.add_suffix(ext);
        }
        let all = gtk::FileFilter::new();
        all.set_name(Some(tr("All Files")));
        all.add_pattern("*");
        let filters = gio::ListStore::new::<gtk::FileFilter>();
        filters.append(&filter);
        filters.append(&all);
        let dialog = gtk::FileDialog::builder().title(tr("Add Files")).filters(&filters).default_filter(&filter).modal(true).build();
        let w = Rc::downgrade(self);
        let win = self.window.clone();
        glib::spawn_future_local(async move {
            let Ok(files) = dialog.open_multiple_future(Some(&win)).await else { return };
            let paths = files.iter::<gio::File>().filter_map(|f| f.ok()?.path()).collect();
            if let Some(app) = w.upgrade() {
                app.add_paths(paths);
            }
        });
    }

    pub fn add_paths(self: &Rc<Self>, paths: Vec<PathBuf>) {
        let mut skipped = 0;
        let mut added = 0;
        for p in paths {
            let kind = formats::kind_of(&formats::ext_of(&p)).filter(|_| p.is_file());
            let Some(kind) = kind else {
                skipped += 1;
                continue;
            };
            if self.items.borrow().iter().any(|i| i.path == p) {
                continue;
            }
            let id = self.next_id.get();
            self.next_id.set(id + 1);
            let target = match kind {
                Kind::Image => self.config.borrow().image_target.clone(),
            };
            let item = Rc::new(Item::new(id, p, kind, &target));
            self.wire(&item);
            self.list.append(&item.row);
            self.thumbs.borrow_mut().push_back(Rc::downgrade(&item));
            self.items.borrow_mut().push(item);
            added += 1;
        }
        if skipped > 0 {
            let msg = tr("Skipped {} unsupported file(s)").replace("{}", &skipped.to_string());
            self.toast(&msg);
        }
        if added > 0 && self.tools.vips.is_none() {
            self.toast(tr("vips is not installed"));
        }
        self.pump_thumbs();
        self.update();
    }

    fn wire(self: &Rc<Self>, item: &Rc<Item>) {
        let w = Rc::downgrade(self);
        let it = Rc::downgrade(item);
        item.remove_btn.connect_clicked(move |_| {
            let (Some(app), Some(item)) = (w.upgrade(), it.upgrade()) else { return };
            if item.state.get() == State::Running {
                item.cancel();
            } else {
                app.remove(&item);
            }
        });
        let w = Rc::downgrade(self);
        let it = Rc::downgrade(item);
        item.open_btn.connect_clicked(move |_| {
            let (Some(app), Some(item)) = (w.upgrade(), it.upgrade()) else { return };
            let Some(out) = item.output.borrow().clone() else { return };
            let launcher = gtk::FileLauncher::new(Some(&gio::File::for_path(out)));
            let win = app.window.clone();
            glib::spawn_future_local(async move {
                let _ = launcher.open_containing_folder_future(Some(&win)).await;
            });
        });
        let it = Rc::downgrade(item);
        let w = Rc::downgrade(self);
        item.target.connect_selected_notify(move |_| {
            let (Some(app), Some(item)) = (w.upgrade(), it.upgrade()) else { return };
            // A new target makes a finished or failed file convertible again.
            if matches!(item.state.get(), State::Done | State::Failed) {
                item.output.take();
                item.set_state(State::Idle);
                app.update();
            }
        });
    }

    fn remove(&self, item: &Rc<Item>) {
        item.cancel();
        self.list.remove(&item.row);
        self.items.borrow_mut().retain(|i| !Rc::ptr_eq(i, item));
        self.update();
    }

    fn clear(&self) {
        let items: Vec<_> = self.items.borrow().clone();
        for it in items.iter().filter(|i| i.state.get() != State::Running) {
            self.remove(it);
        }
    }

    fn set_all(&self, ext: &str) {
        for it in self.items.borrow().iter().filter(|i| !i.busy()) {
            it.set_target(ext);
        }
        let mut c = self.config.borrow_mut();
        if formats::targets(Kind::Image).contains(&ext) {
            c.image_target = ext.into();
        }
    }

    fn convert_all(self: &Rc<Self>) {
        if self.tools.vips.is_none() {
            self.toast(tr("vips is not installed"));
            return;
        }
        for it in self.items.borrow().iter().filter(|i| matches!(i.state.get(), State::Idle | State::Failed)) {
            it.set_state(State::Queued);
        }
        self.pump();
        self.update();
    }

    fn pump(self: &Rc<Self>) {
        let jobs = self.config.borrow().jobs as usize;
        while self.running.get() < jobs {
            let next = self.items.borrow().iter().find(|i| i.state.get() == State::Queued).cloned();
            let Some(item) = next else { break };
            self.start(item);
        }
    }

    fn start(self: &Rc<Self>, item: Rc<Item>) {
        let Some(vips) = self.tools.vips.clone() else { return };
        let target = item.target();
        let (dir, opts) = {
            let c = self.config.borrow();
            let dir = c.output_dir.clone().filter(|d| d.is_dir()).or_else(|| item.path.parent().map(Path::to_path_buf)).unwrap_or_default();
            (dir, ImageOptions { quality: c.image_quality, keep_metadata: c.keep_metadata })
        };
        let dst = {
            let reserved = self.reserved.borrow();
            convert::output_path(&dir, &item.path, target, &|p| reserved.contains(p))
        };
        self.reserved.borrow_mut().insert(dst.clone());
        let tmp = convert::temp_path(&dst, item.id);
        let argv = match item.kind {
            Kind::Image => convert::image_argv(&vips, &item.path, &tmp, target, &opts),
        };

        item.cancelled.set(false);
        item.set_state(State::Running);
        self.running.set(self.running.get() + 1);
        self.update();

        let app = self.clone();
        glib::spawn_future_local(async move {
            let result = run(&item, &argv).await.and_then(|()| finish(&tmp, &dst));
            if tmp.exists() {
                let _ = std::fs::remove_file(&tmp);
            }
            app.reserved.borrow_mut().remove(&dst);
            app.running.set(app.running.get() - 1);
            match result {
                Ok(out) => {
                    item.output.replace(Some(out));
                    item.set_state(State::Done);
                }
                Err(_) if item.cancelled.get() => {
                    item.set_state(State::Idle);
                    item.subtitle.set_label(tr("Cancelled"));
                }
                Err(e) => item.fail(&e),
            }
            app.pump();
            app.update();
        });
    }

    fn pump_thumbs(self: &Rc<Self>) {
        let Some(vips) = self.tools.vips.clone() else { return };
        while self.thumbs_running.get() < THUMB_JOBS {
            let next = self.thumbs.borrow_mut().pop_front();
            let Some(weak) = next else { break };
            let Some(item) = weak.upgrade() else { continue };
            let cache = thumb_path(&item.path);
            if cache.exists() {
                item.set_thumbnail(&cache);
                continue;
            }
            if let Some(d) = cache.parent() {
                let _ = std::fs::create_dir_all(d);
            }
            let tmp = convert::temp_path(&cache, item.id);
            let argv = convert::thumbnail_argv(&vips, &item.path, &tmp, THUMB_SIZE);
            self.thumbs_running.set(self.thumbs_running.get() + 1);
            let app = self.clone();
            glib::spawn_future_local(async move {
                if run(&item, &argv).await.and_then(|()| finish_exact(&tmp, &cache)).is_ok() {
                    item.set_thumbnail(&cache);
                }
                let _ = std::fs::remove_file(&tmp);
                app.thumbs_running.set(app.thumbs_running.get() - 1);
                app.pump_thumbs();
            });
        }
    }

    fn update(&self) {
        let items = self.items.borrow();
        self.stack.set_visible_child_name(if items.is_empty() { "empty" } else { "list" });
        self.clear_btn.set_visible(!items.is_empty());
        let todo = items.iter().filter(|i| matches!(i.state.get(), State::Idle | State::Failed)).count();
        let done = items.iter().filter(|i| i.state.get() == State::Done).count();
        let busy = items.iter().filter(|i| i.busy()).count();
        self.convert_btn.set_sensitive(todo > 0);
        self.set_all.set_sensitive(items.iter().any(|i| !i.busy()));
        let mut parts = vec![tr("{} files").replace("{}", &items.len().to_string())];
        if busy > 0 {
            parts.push(tr("{} in progress").replace("{}", &busy.to_string()));
        }
        if done > 0 {
            parts.push(tr("{} done").replace("{}", &done.to_string()));
        }
        self.count.set_label(&parts.join(" · "));
    }

    pub fn toast(&self, msg: &str) {
        self.toasts.add_toast(adw::Toast::new(msg));
    }

    fn about(&self) {
        let about = adw::AboutDialog::builder()
            .application_name("Morph")
            // TODO(stage 5): own icon, installed as io.github.wioletowa.Morph.
            .application_icon("document-send-symbolic")
            .developer_name("wioletowa")
            .version(env!("CARGO_PKG_VERSION"))
            .license_type(gtk::License::MitX11)
            .comments(tr("Convert images on your computer. Inspired by VERT."))
            .build();
        about.add_legal_section(
            "VERT",
            Some("vert.sh"),
            gtk::License::Custom,
            Some(tr("Morph is an independent program inspired by VERT. It contains no code or artwork from VERT and is not affiliated with its authors.")),
        );
        about.add_legal_section(
            "libvips",
            Some("© John Cupitt and others"),
            gtk::License::Lgpl21,
            Some(tr("Morph runs the vips command-line tool for image conversion; it is not linked into Morph.")),
        );
        about.present(Some(&self.window));
    }

    #[cfg(debug_assertions)]
    pub fn debug_state(&self) -> String {
        self.items
            .borrow()
            .iter()
            .map(|i| {
                let out = i.output.borrow().as_ref().map(|p| p.display().to_string()).unwrap_or_default();
                format!("{} {:?} {} {} | {}", i.path.display(), i.state.get(), i.target(), out, i.subtitle.label())
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[cfg(debug_assertions)]
    pub fn idle(&self) -> bool {
        self.running.get() == 0 && self.thumbs_running.get() == 0 && !self.items.borrow().iter().any(|i| i.busy())
    }

    #[cfg(debug_assertions)]
    pub fn debug_target(&self, index: usize, ext: &str) {
        if let Some(i) = self.items.borrow().get(index) {
            i.set_target(ext);
        }
    }

    #[cfg(debug_assertions)]
    pub fn debug_cancel(&self, index: usize) {
        if let Some(i) = self.items.borrow().get(index) {
            i.cancel();
        }
    }
}

async fn run(item: &Item, argv: &[OsString]) -> Result<(), String> {
    let launcher = gio::SubprocessLauncher::new(gio::SubprocessFlags::STDOUT_SILENCE | gio::SubprocessFlags::STDERR_PIPE);
    let args: Vec<&OsStr> = argv.iter().map(OsString::as_os_str).collect();
    let p = launcher.spawn(&args).map_err(|e| e.message().to_string())?;
    item.proc.replace(Some(p.clone()));
    let out = p.communicate_utf8_future(None).await;
    item.proc.take();
    let (_, err) = out.map_err(|e| e.message().to_string())?;
    if p.is_successful() {
        Ok(())
    } else {
        let msg = convert::clean_stderr(err.as_deref().unwrap_or(""));
        Err(if msg.is_empty() { tr("the converter exited with an error").to_string() } else { msg })
    }
}

/// Moves the finished temp file to `dst`, or to the next free name if something appeared there meanwhile.
fn finish(tmp: &Path, dst: &Path) -> Result<PathBuf, String> {
    let dst = if dst.exists() {
        let ext = dst.extension().and_then(|e| e.to_str()).unwrap_or("");
        convert::output_path(dst.parent().unwrap_or(Path::new(".")), dst, ext, &|_| false)
    } else {
        dst.to_path_buf()
    };
    finish_exact(tmp, &dst).map(|()| dst)
}

fn finish_exact(tmp: &Path, dst: &Path) -> Result<(), String> {
    if !tmp.is_file() {
        return Err(tr("the converter produced no file").to_string());
    }
    std::fs::rename(tmp, dst).map_err(|e| e.to_string())
}

fn thumb_path(src: &Path) -> PathBuf {
    let mut h = std::hash::DefaultHasher::new();
    src.hash(&mut h);
    if let Ok(m) = src.metadata() {
        m.len().hash(&mut h);
        m.modified().ok().hash(&mut h);
    }
    config::cache_dir().join("thumbs").join(format!("{:016x}.png", h.finish()))
}
