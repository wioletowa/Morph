//! Tiny ru/en string table. English text is the key.

use std::sync::OnceLock;

static RU: OnceLock<bool> = OnceLock::new();

/// `setting` is "auto", "ru" or "en".
pub fn init(setting: &str) {
    let ru = match setting {
        "ru" => true,
        "en" => false,
        _ => ["LC_ALL", "LC_MESSAGES", "LANG"]
            .iter()
            .filter_map(|v| std::env::var(v).ok())
            .find(|v| !v.is_empty())
            .is_some_and(|v| v.starts_with("ru")),
    };
    let _ = RU.set(ru);
}

pub fn tr(en: &'static str) -> &'static str {
    if !RU.get().copied().unwrap_or(false) {
        return en;
    }
    match en {
        "Add Files" => "Добавить файлы",
        "Preferences" => "Параметры",
        "About Morph" => "О программе",
        "Main Menu" => "Главное меню",
        "Clear List" => "Очистить список",
        "Choose Files…" => "Выбрать файлы…",
        "Files are converted on this computer. Nothing is uploaded." => {
            "Файлы конвертируются на этом компьютере. Ничего никуда не загружается."
        }
        "vips is not installed, so images cannot be converted. Install the libvips package." => {
            "vips не установлен, конвертировать изображения нельзя. Установите пакет libvips."
        }
        "vips is not installed" => "vips не установлен",
        "Drop Files to Convert" => "Перетащите файлы сюда",
        "Set All To" => "Всё в",
        "Convert All" => "Конвертировать всё",
        "Supported Files" => "Поддерживаемые файлы",
        "All Files" => "Все файлы",
        "Skipped {} unsupported file(s)" => "Пропущено неподдерживаемых файлов: {}",
        "{} files" => "Файлов: {}",
        "{} in progress" => "в работе: {}",
        "{} done" => "готово: {}",
        "Cancelled" => "Отменено",
        "Convert to" => "Конвертировать в",
        "Show in folder" => "Показать в папке",
        "Remove" => "Убрать",
        "Cancel" => "Отменить",
        "Waiting…" => "в очереди…",
        "Converting…" => "Конвертация…",
        "Failed" => "Ошибка",
        "the converter exited with an error" => "конвертер завершился с ошибкой",
        "the converter produced no file" => "конвертер не создал файл",
        "Convert images on your computer. Inspired by VERT." => "Конвертирует изображения на вашем компьютере. Вдохновлено VERT.",
        "Morph is an independent program inspired by VERT. It contains no code or artwork from VERT and is not affiliated with its authors." => {
            "Morph — независимая программа, вдохновлённая VERT. В ней нет кода или графики VERT, и она не связана с его авторами."
        }
        "Morph runs the vips command-line tool for image conversion; it is not linked into Morph." => {
            "Morph запускает утилиту vips для конвертации изображений; библиотека не встроена в Morph."
        }
        "Output" => "Сохранение",
        "Save To" => "Куда сохранять",
        "Next to the Original" => "Рядом с исходным файлом",
        "Custom Folder" => "В выбранную папку",
        "Folder" => "Папка",
        "Not Selected" => "Не выбрана",
        "Choose Output Folder" => "Папка для результатов",
        "Images" => "Изображения",
        "Quality" => "Качество",
        "For JPG, WebP, AVIF, HEIC and JPEG XL" => "Для JPG, WebP, AVIF, HEIC и JPEG XL",
        "Keep Metadata" => "Сохранять метаданные",
        "EXIF data such as camera, date and location" => "EXIF: камера, дата, геопозиция",
        "General" => "Общие",
        "Simultaneous Conversions" => "Одновременных конвертаций",
        "Language" => "Язык",
        "Applies after restart" => "Применится после перезапуска",
        "Automatic" => "Автоматически",
        e => e,
    }
}
