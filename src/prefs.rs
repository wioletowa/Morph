//! Preferences dialog. Every change is written to the config at once.

use crate::config::Config;
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

    let hw = adw::PreferencesGroup::builder()
        .title(tr("Hardware Acceleration"))
        .description(tr("Video is encoded on the GPU when it supports the chosen codec; otherwise, or if the GPU fails, on the CPU"))
        .build();
    hw.add(&choice_row(
        app,
        tr("Video Encoding"),
        None,
        &[("auto", tr("Automatic")), ("nvidia", "NVIDIA NVENC"), ("vaapi", "VAAPI (Intel, AMD)"), ("cpu", tr("CPU Only"))],
        &c.gpu_mode,
        |c, v| c.gpu_mode = v.into(),
    ));
    let nodes: Vec<(String, String)> = crate::gpu::render_nodes()
        .into_iter()
        .map(|(p, vendor)| {
            let node = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            (p.to_string_lossy().into_owned(), format!("{vendor} ({node})"))
        })
        .collect();
    let mut choices: Vec<(&str, &str)> = vec![("", tr("Automatic"))];
    choices.extend(nodes.iter().map(|(p, l)| (p.as_str(), l.as_str())));
    hw.add(&choice_row(app, tr("VAAPI Device"), None, &choices, &c.vaapi_device, |c, v| c.vaapi_device = v.into()));
    let detected = adw::ActionRow::builder().title(tr("Detected")).subtitle(app.gpu_summary()).subtitle_selectable(true).build();
    hw.add(&detected);
    page.add(&hw);

    let video = adw::PreferencesGroup::builder().title(tr("Video")).build();
    let codec = choice_row(app, tr("Codec"), Some(tr("For MP4, MKV and MOV")), &[("h264", "H.264"), ("h265", "H.265 (HEVC)"), ("av1", "AV1")], &c.video_codec, |c, v| c.video_codec = v.into());
    let crf = adw::SpinRow::builder()
        .title(tr("Quality (CRF)"))
        .subtitle(tr("Lower means better quality and bigger files; 23 is a good default"))
        .adjustment(&gtk::Adjustment::new(c.video_crf as f64, 0.0, 51.0, 1.0, 5.0, 0.0))
        .build();
    let speed = choice_row(app, tr("Encoding Speed"), Some(tr("Slower gives smaller files at the same quality")), &[("fast", tr("Fast")), ("medium", tr("Balanced")), ("slow", tr("Slow"))], &c.video_speed, |c, v| c.video_speed = v.into());
    let height = c.video_max_height.to_string();
    let res = choice_row(
        app,
        tr("Maximum Resolution"),
        None,
        &[("0", tr("Original")), ("2160", "2160p (4K)"), ("1440", "1440p"), ("1080", "1080p"), ("720", "720p"), ("480", "480p")],
        &height,
        |c, v| c.video_max_height = v.parse().unwrap_or(0),
    );
    let video_jobs = adw::SpinRow::builder()
        .title(tr("Simultaneous Video Conversions"))
        .adjustment(&gtk::Adjustment::new(c.video_jobs as f64, 1.0, 4.0, 1.0, 1.0, 0.0))
        .build();
    for r in [codec.upcast_ref::<gtk::Widget>(), crf.upcast_ref(), speed.upcast_ref(), res.upcast_ref(), video_jobs.upcast_ref()] {
        video.add(r);
    }
    page.add(&video);
    {
        let app = app.clone();
        crf.connect_value_notify(move |r| {
            let mut c = app.config.borrow_mut();
            c.video_crf = r.value() as u32;
            c.save();
        });
    }
    {
        let app = app.clone();
        video_jobs.connect_value_notify(move |r| {
            let mut c = app.config.borrow_mut();
            c.video_jobs = r.value() as u32;
            c.save();
        });
    }

    let audio = adw::PreferencesGroup::builder().title(tr("Audio")).build();
    let kbps = c.audio_kbps.to_string();
    audio.add(&choice_row(
        app,
        tr("Bitrate"),
        Some(tr("For MP3, AAC, Ogg and Opus; FLAC, WAV and AIFF are lossless")),
        &[("128", "128 kbit/s"), ("192", "192 kbit/s"), ("256", "256 kbit/s"), ("320", "320 kbit/s")],
        &kbps,
        |c, v| c.audio_kbps = v.parse().unwrap_or(192),
    ));
    page.add(&audio);

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

/// A combo row over fixed `(value, label)` pairs that writes the picked value to the config.
fn choice_row(
    app: &Rc<App>,
    title: &str,
    subtitle: Option<&str>,
    choices: &[(&str, &str)],
    current: &str,
    set: fn(&mut Config, &str),
) -> adw::ComboRow {
    let labels: Vec<&str> = choices.iter().map(|(_, l)| *l).collect();
    let row = adw::ComboRow::builder()
        .title(title)
        .model(&gtk::StringList::new(&labels))
        .selected(choices.iter().position(|(v, _)| *v == current).unwrap_or(0) as u32)
        .build();
    if let Some(s) = subtitle {
        row.set_subtitle(s);
    }
    let values: Vec<String> = choices.iter().map(|(v, _)| v.to_string()).collect();
    let app = app.clone();
    row.connect_selected_notify(move |r| {
        let Some(v) = values.get(r.selected() as usize) else { return };
        let mut c = app.config.borrow_mut();
        set(&mut c, v);
        c.save();
    });
    row
}
