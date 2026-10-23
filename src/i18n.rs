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
        "{} queued" => "в очереди: {}",
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
        "Video" => "Видео",
        "Audio" => "Аудио",
        "Codec" => "Кодек",
        "For MP4, MKV and MOV" => "Для MP4, MKV и MOV",
        "Quality (CRF)" => "Качество (CRF)",
        "Lower means better quality and bigger files; 23 is a good default" => {
            "Меньше — выше качество и больше файл; 23 — хорошее значение"
        }
        "Encoding Speed" => "Скорость кодирования",
        "Slower gives smaller files at the same quality" => "Медленнее — меньше файл при том же качестве",
        "Fast" => "Быстро",
        "Balanced" => "Сбалансировано",
        "Slow" => "Медленно",
        "Maximum Resolution" => "Максимальное разрешение",
        "Original" => "Исходное",
        "Simultaneous Video Conversions" => "Одновременных видео",
        "Bitrate" => "Битрейт",
        "For MP3, AAC, Ogg and Opus; FLAC, WAV and AIFF are lossless" => "Для MP3, AAC, Ogg и Opus; FLAC, WAV и AIFF без потерь",
        "{} left" => "осталось {}",
        "ffmpeg is not installed" => "ffmpeg не установлен",
        "ffmpeg is not installed, so audio and video cannot be converted. Install the ffmpeg package." => {
            "ffmpeg не установлен, конвертировать аудио и видео нельзя. Установите пакет ffmpeg."
        }
        "Images, audio and video are converted on this computer. Nothing is uploaded." => {
            "Изображения, аудио и видео конвертируются на этом компьютере. Ничего никуда не загружается."
        }
        "Convert images, audio and video on your computer. Inspired by VERT." => {
            "Конвертирует изображения, аудио и видео на вашем компьютере. Вдохновлено VERT."
        }
        "Morph runs the ffmpeg and ffprobe programs for audio and video; they are not linked into Morph. Depending on how your copy was built, its license may be the GPL." => {
            "Morph запускает программы ffmpeg и ffprobe для аудио и видео; они не встроены в Morph. В зависимости от сборки их лицензия может быть GPL."
        }
        "the file has no audio track" => "в файле нет звуковой дорожки",
        "the file has no video track" => "в файле нет видеодорожки",
        "Hardware Acceleration" => "Аппаратное ускорение",
        "Video is encoded on the GPU when it supports the chosen codec; otherwise, or if the GPU fails, on the CPU" => {
            "Видео кодируется на видеокарте, если она поддерживает выбранный кодек; иначе или при сбое — на процессоре"
        }
        "Video Encoding" => "Кодирование видео",
        "CPU Only" => "Только процессор",
        "VAAPI Device" => "Устройство VAAPI",
        "Detected" => "Найдено",
        "Checking…" => "Проверка…",
        "no working encoder" => "нет рабочего кодировщика",
        "no working encoder; install intel-media-driver" => "нет рабочего кодировщика; установите intel-media-driver",
        "No supported GPU found" => "Поддерживаемых видеокарт не найдено",
        "GPU failed" => "сбой GPU",
        e => e,
    }
}
