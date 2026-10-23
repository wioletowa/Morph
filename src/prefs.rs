//! Preferences dialog. Every change is written to the config at once.

use crate::i18n::tr;
use crate::window::App;
use adw::prelude::*;
use gtk::{gio, glib};
use std::rc::Rc;

pub fn show(app: &Rc<App>) {
    let c = app.config.borrow().clone();
    let dialog = adw::PreferencesDialog::builder().title(tr("Preferences")).build();
    let page = adw::PreferencesPage::new();
    dialog.add(&page);

    let output = adw::PreferencesGroup::builder().title(tr("Output")).build();
    let place = adw::ComboRow::builder()
        .title(tr("Save To"))
        .model(&gtk::StringList::new(&[tr("Next to the Original"), tr("Custom Folder")]))
        .selected(c.output_dir.is_some() as u32)
        .build();
    let folder_label = gtk::Label::builder().ellipsize(gtk::pango::EllipsizeMode::Start).max_width_chars(28).css_classes(["dim-label"]).build();
    let folder = adw::ActionRow::builder().title(tr("Folder")).activatable(true).sensitive(c.output_dir.is_some()).build();
    folder.add_suffix(&folder_label);
    folder.add_suffix(&gtk::Image::from_icon_name("folder-open-symbolic"));
    let show_folder = {
        let label = folder_label.clone();
        move |p: Option<&std::path::Path>| label.set_label(&p.map(|p| p.display().to_string()).unwrap_or_else(|| tr("Not Selected").into()))
    };
    show_folder(c.output_dir.as_deref());
    output.add(&place);
    output.add(&folder);
    page.add(&output);

    {
        let app = app.clone();
        let folder = folder.clone();
        let show_folder = show_folder.clone();
        place.connect_selected_notify(move |row| {
            let custom = row.selected() == 1;
            folder.set_sensitive(custom);
            let mut c = app.config.borrow_mut();
            if !custom {
                c.output_dir = None;
            } else if c.output_dir.is_none() {
                c.output_dir = glib::user_special_dir(glib::UserDirectory::Downloads);
            }
            show_folder(c.output_dir.as_deref());
            c.save();
        });
    }
    {
        let app = app.clone();
        let dialog_ref = dialog.downgrade();
        folder.connect_activated(move |_| {
            let app = app.clone();
            let show_folder = show_folder.clone();
            let parent = dialog_ref.upgrade().and_then(|d| d.root()).and_downcast::<gtk::Window>();
            let chooser = gtk::FileDialog::builder().title(tr("Choose Output Folder")).modal(true).build();
            if let Some(d) = app.config.borrow().output_dir.as_ref() {
                chooser.set_initial_folder(Some(&gio::File::for_path(d)));
            }
            glib::spawn_future_local(async move {
                let Ok(f) = chooser.select_folder_future(parent.as_ref()).await else { return };
                let mut c = app.config.borrow_mut();
                c.output_dir = f.path();
                show_folder(c.output_dir.as_deref());
                c.save();
            });
        });
    }

    let images = adw::PreferencesGroup::builder().title(tr("Images")).build();
    let quality = adw::SpinRow::builder()
        .title(tr("Quality"))
        .subtitle(tr("For JPG, WebP, AVIF, HEIC and JPEG XL"))
        .adjustment(&gtk::Adjustment::new(c.image_quality as f64, 1.0, 100.0, 1.0, 10.0, 0.0))
        .build();
    let keep = adw::SwitchRow::builder()
        .title(tr("Keep Metadata"))
        .subtitle(tr("EXIF data such as camera, date and location"))
        .active(c.keep_metadata)
        .build();
    images.add(&quality);
    images.add(&keep);
    page.add(&images);
    {
        let app = app.clone();
        quality.connect_value_notify(move |r| {
            let mut c = app.config.borrow_mut();
            c.image_quality = r.value() as u32;
            c.save();
        });
    }
    {
        let app = app.clone();
        keep.connect_active_notify(move |r| {
            let mut c = app.config.borrow_mut();
            c.keep_metadata = r.is_active();
            c.save();
        });
    }

    let general = adw::PreferencesGroup::builder().title(tr("General")).build();
    let max_jobs = std::thread::available_parallelism().map_or(4, |n| n.get()) as f64;
    let jobs = adw::SpinRow::builder()
        .title(tr("Simultaneous Conversions"))
        .adjustment(&gtk::Adjustment::new(c.jobs as f64, 1.0, max_jobs.max(1.0), 1.0, 1.0, 0.0))
        .build();
    let langs = ["auto", "en", "ru"];
    let lang = adw::ComboRow::builder()
        .title(tr("Language"))
        .subtitle(tr("Applies after restart"))
        .model(&gtk::StringList::new(&[tr("Automatic"), "English", "Русский"]))
        .selected(langs.iter().position(|l| *l == c.lang).unwrap_or(0) as u32)
        .build();
    general.add(&jobs);
    general.add(&lang);
    page.add(&general);
    {
        let app = app.clone();
        jobs.connect_value_notify(move |r| {
            let mut c = app.config.borrow_mut();
            c.jobs = r.value() as u32;
            c.save();
        });
    }
    {
        let app = app.clone();
        lang.connect_selected_notify(move |r| {
            let mut c = app.config.borrow_mut();
            c.lang = langs.get(r.selected() as usize).unwrap_or(&"auto").to_string();
            c.save();
        });
    }

    dialog.present(Some(&app.window));
}
