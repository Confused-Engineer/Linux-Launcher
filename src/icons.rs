//! Resolving and painting app icons.
//!
//! Icon precedence for an app:
//! 1. `icon` set to an image path/URL → loaded through egui's image loaders.
//! 2. `icon` set to any other file (`.exe`, `.dll`, `.lnk`, ...) → that file's
//!    shell icon.
//! 3. No `icon`, target is a local file/folder (or a program on `PATH`) → the
//!    target's own shell icon, exactly as Explorer shows it.
//! 4. Otherwise (URLs, URIs, missing files) → a coloured letter tile.
//!
//! Shell icons are extracted on a background thread and cached as textures.

use crate::config::AppEntry;
use crate::launch::expand_env;
use eframe::egui::{
    self, Color32, ColorImage, Context, FontId, Rect, TextureHandle, TextureOptions, Ui, ecolor,
    load::TexturePoll,
};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, channel};

const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "gif", "bmp", "webp", "svg", "ico"];
const SHELL_ICON_SIZE: i32 = 256;

/// Where an app's icon comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IconSource {
    /// An image URI understood by egui's loaders (`https://`, `file://`, ...).
    Uri(String),
    /// A file whose shell icon should be shown.
    Shell(PathBuf),
    /// Draw a letter tile.
    Fallback,
}

/// Decides which [`IconSource`] to use for an app (see module docs).
pub fn resolve(app: &AppEntry) -> IconSource {
    let icon = app.icon.trim();
    if !icon.is_empty() {
        let icon = expand_env(icon);
        if is_uri(&icon) {
            return IconSource::Uri(icon);
        }
        return if has_image_extension(&icon) {
            IconSource::Uri(format!("file://{icon}"))
        } else {
            IconSource::Shell(PathBuf::from(icon))
        };
    }

    let target = expand_env(app.target.trim());
    if target.is_empty() || is_uri(&target) {
        return IconSource::Fallback;
    }
    let path = Path::new(&target);
    if path.is_absolute() {
        return if path.exists() {
            IconSource::Shell(path.to_path_buf())
        } else {
            IconSource::Fallback
        };
    }
    find_in_path(&target).map_or(IconSource::Fallback, IconSource::Shell)
}

/// True for `scheme:...` strings such as URLs, `ms-settings:` or `mailto:`.
/// Drive letters (`C:\`) are not treated as schemes.
pub fn is_uri(s: &str) -> bool {
    let Some((scheme, _)) = s.split_once(':') else {
        return false;
    };
    scheme.len() >= 2
        && scheme.starts_with(|c: char| c.is_ascii_alphabetic())
        && scheme
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
}

fn has_image_extension(path: &str) -> bool {
    Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| IMAGE_EXTENSIONS.iter().any(|x| x.eq_ignore_ascii_case(e)))
}

/// Looks up a bare program name (`notepad`, `cmd.exe`) on `PATH`.
fn find_in_path(name: &str) -> Option<PathBuf> {
    if name.contains(['\\', '/']) {
        return None;
    }
    let has_ext = Path::new(name).extension().is_some();
    let exts: Vec<String> = if has_ext {
        vec![String::new()]
    } else {
        std::env::var("PATHEXT")
            .unwrap_or_else(|_| ".EXE;.CMD;.BAT;.COM".into())
            .split(';')
            .map(str::to_owned)
            .collect()
    };
    let path_var = std::env::var_os("PATH")?;
    std::env::split_paths(&path_var).find_map(|dir| {
        exts.iter()
            .map(|ext| dir.join(format!("{name}{ext}")))
            .find(|p| p.is_file())
    })
}

enum ShellSlot {
    Pending,
    Ready(TextureHandle),
    Failed,
}

/// Caches resolved icon sources and shell-icon textures.
pub struct IconCache {
    resolved: HashMap<(String, String), IconSource>,
    shell: HashMap<PathBuf, ShellSlot>,
    requests: Sender<PathBuf>,
    results: Receiver<(PathBuf, Option<ColorImage>)>,
}

impl IconCache {
    pub fn new(ctx: &Context) -> Self {
        let (req_tx, req_rx) = channel::<PathBuf>();
        let (res_tx, res_rx) = channel();
        let ctx = ctx.clone();
        let spawned = std::thread::Builder::new()
            .name("shell-icons".into())
            .spawn(move || {
                let _com = platform::ComGuard::init();
                for path in req_rx {
                    let image = platform::shell_icon(&path, SHELL_ICON_SIZE);
                    if res_tx.send((path, image)).is_err() {
                        break;
                    }
                    ctx.request_repaint();
                }
            });
        if let Err(e) = spawned {
            log::error!("could not start icon thread: {e}");
        }
        Self {
            resolved: HashMap::new(),
            shell: HashMap::new(),
            requests: req_tx,
            results: res_rx,
        }
    }

    /// Uploads icons finished by the background thread. Call once per frame.
    pub fn poll(&mut self, ctx: &Context) {
        while let Ok((path, image)) = self.results.try_recv() {
            let slot = match image {
                Some(image) => {
                    let options = TextureOptions {
                        mipmap_mode: Some(egui::TextureFilter::Linear),
                        ..TextureOptions::LINEAR
                    };
                    let name = format!("shell-icon:{}", path.display());
                    ShellSlot::Ready(ctx.load_texture(name, image, options))
                }
                None => ShellSlot::Failed,
            };
            self.shell.insert(path, slot);
        }
    }

    /// Forgets everything so icons are re-resolved and re-extracted.
    pub fn clear(&mut self, ctx: &Context) {
        self.resolved.clear();
        self.shell
            .retain(|_, slot| matches!(slot, ShellSlot::Pending));
        ctx.forget_all_images();
    }

    fn source(&mut self, app: &AppEntry) -> IconSource {
        self.resolved
            .entry((app.icon.clone(), app.target.clone()))
            .or_insert_with(|| resolve(app))
            .clone()
    }

    /// Paints the icon for `app` into `rect`.
    pub fn paint(&mut self, ui: &Ui, app: &AppEntry, rect: Rect) {
        let rounding = rect.width() * 0.18;
        match self.source(app) {
            IconSource::Uri(uri) => {
                match egui::Image::new(uri.as_str()).load_for_size(ui.ctx(), rect.size()) {
                    Ok(TexturePoll::Ready { texture }) => {
                        paint_texture(ui, texture, rect, rounding)
                    }
                    Ok(TexturePoll::Pending { .. }) => {
                        egui::Spinner::new().paint_at(ui, rect.shrink(rect.width() * 0.3));
                    }
                    Err(_) => paint_letter(ui, &app.name, rect),
                }
            }
            IconSource::Shell(path) => match self.shell.get(&path) {
                Some(ShellSlot::Ready(tex)) => {
                    paint_texture(ui, egui::load::SizedTexture::from_handle(tex), rect, 0.0);
                }
                Some(ShellSlot::Pending) => {
                    egui::Spinner::new().paint_at(ui, rect.shrink(rect.width() * 0.3));
                }
                Some(ShellSlot::Failed) => paint_letter(ui, &app.name, rect),
                None => {
                    if self.requests.send(path.clone()).is_ok() {
                        self.shell.insert(path, ShellSlot::Pending);
                    } else {
                        self.shell.insert(path, ShellSlot::Failed);
                    }
                    egui::Spinner::new().paint_at(ui, rect.shrink(rect.width() * 0.3));
                }
            },
            IconSource::Fallback => paint_letter(ui, &app.name, rect),
        }
    }
}

/// Paints a texture centred in `rect`, preserving its aspect ratio.
fn paint_texture(ui: &Ui, texture: egui::load::SizedTexture, rect: Rect, rounding: f32) {
    let size = texture.size;
    if size.x <= 0.0 || size.y <= 0.0 {
        return;
    }
    let scale = (rect.width() / size.x).min(rect.height() / size.y);
    let fitted = Rect::from_center_size(rect.center(), size * scale);
    egui::Image::from_texture(texture)
        .corner_radius(rounding)
        .paint_at(ui, fitted);
}

/// Paints a rounded square with the app's initial, coloured by its name.
pub fn paint_letter(ui: &Ui, name: &str, rect: Rect) {
    let hash = name.bytes().fold(0x811c_9dc5_u32, |h, b| {
        (h ^ u32::from(b)).wrapping_mul(0x0100_0193)
    });
    let hue = (hash % 360) as f32 / 360.0;
    let fill: Color32 = ecolor::Hsva::new(hue, 0.45, 0.6, 1.0).into();
    let tile = rect.shrink(rect.width() * 0.08);
    let painter = ui.painter();
    painter.rect_filled(tile, tile.width() * 0.2, fill);
    let letter = name
        .chars()
        .find(|c| c.is_alphanumeric())
        .map(|c| c.to_uppercase().to_string())
        .unwrap_or_else(|| "?".into());
    painter.text(
        tile.center(),
        egui::Align2::CENTER_CENTER,
        letter,
        FontId::proportional(tile.height() * 0.55),
        Color32::WHITE,
    );
}

#[cfg(windows)]
mod platform {
    //! Shell icon extraction via `IShellItemImageFactory`, which yields the
    //! same high-resolution icons Explorer shows (including for `.lnk` files).
    use eframe::egui::ColorImage;
    use std::path::Path;
    use windows::Win32::Foundation::SIZE;
    use windows::Win32::Graphics::Gdi::{
        BI_RGB, BITMAP, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, DIB_RGB_COLORS, DeleteDC,
        DeleteObject, GetDIBits, GetObjectW, HBITMAP, HGDIOBJ,
    };
    use windows::Win32::System::Com::{
        COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE, CoInitializeEx, CoUninitialize,
    };
    use windows::Win32::UI::Shell::{
        IShellItemImageFactory, SHCreateItemFromParsingName, SIIGBF_BIGGERSIZEOK, SIIGBF_ICONONLY,
    };
    use windows::core::HSTRING;

    /// Initialises COM for the current thread and uninitialises it on drop.
    pub struct ComGuard(bool);

    impl ComGuard {
        pub fn init() -> Self {
            // SAFETY: called once at the start of a dedicated thread.
            let hr =
                unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) };
            Self(hr.is_ok())
        }
    }

    impl Drop for ComGuard {
        fn drop(&mut self) {
            if self.0 {
                // SAFETY: balanced with the successful CoInitializeEx above.
                unsafe { CoUninitialize() };
            }
        }
    }

    pub fn shell_icon(path: &Path, size: i32) -> Option<ColorImage> {
        let result = extract(path, size);
        if let Err(e) = &result {
            log::debug!("no shell icon for {}: {e}", path.display());
        }
        result.ok()
    }

    fn extract(path: &Path, size: i32) -> windows::core::Result<ColorImage> {
        let wide = HSTRING::from(path);
        // SAFETY: plain COM calls on a thread with COM initialised; the bitmap
        // is owned by us and released below.
        unsafe {
            let factory: IShellItemImageFactory = SHCreateItemFromParsingName(&wide, None)?;
            let bitmap = factory.GetImage(
                SIZE { cx: size, cy: size },
                SIIGBF_ICONONLY | SIIGBF_BIGGERSIZEOK,
            )?;
            let image = bitmap_to_image(bitmap);
            let _ = DeleteObject(HGDIOBJ(bitmap.0));
            image.ok_or_else(windows::core::Error::from_thread)
        }
    }

    /// Copies a 32-bit premultiplied BGRA bitmap into an egui image.
    unsafe fn bitmap_to_image(bitmap: HBITMAP) -> Option<ColorImage> {
        let mut bm = BITMAP::default();
        // SAFETY: `bm` is a valid out-buffer of the size we pass.
        let got = unsafe {
            GetObjectW(
                HGDIOBJ(bitmap.0),
                std::mem::size_of::<BITMAP>() as i32,
                Some((&raw mut bm).cast()),
            )
        };
        if got == 0 || bm.bmWidth <= 0 || bm.bmHeight <= 0 {
            return None;
        }
        let (w, h) = (bm.bmWidth, bm.bmHeight);

        let mut info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w,
                biHeight: -h, // negative = top-down rows
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut pixels = vec![0u8; (w * h * 4) as usize];

        // SAFETY: `pixels` holds exactly w*h 32-bit pixels as described by `info`.
        let lines = unsafe {
            let dc = CreateCompatibleDC(None);
            let lines = GetDIBits(
                dc,
                bitmap,
                0,
                h as u32,
                Some(pixels.as_mut_ptr().cast()),
                &mut info,
                DIB_RGB_COLORS,
            );
            let _ = DeleteDC(dc);
            lines
        };
        if lines == 0 {
            return None;
        }

        // Some icons come back without an alpha channel; treat them as opaque.
        let has_alpha = pixels.as_chunks::<4>().0.iter().any(|p| p[3] != 0);
        for px in pixels.as_chunks_mut::<4>().0 {
            px.swap(0, 2); // BGRA -> RGBA
            if !has_alpha {
                px[3] = 255;
            }
        }
        Some(ColorImage::from_rgba_premultiplied(
            [w as usize, h as usize],
            &pixels,
        ))
    }
}

#[cfg(not(windows))]
mod platform {
    use eframe::egui::ColorImage;
    use std::path::Path;

    pub struct ComGuard;

    impl ComGuard {
        pub fn init() -> Self {
            Self
        }
    }

    pub fn shell_icon(_path: &Path, _size: i32) -> Option<ColorImage> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_uris_but_not_drive_letters() {
        assert!(is_uri("https://example.com"));
        assert!(is_uri("ms-settings:apps-volume"));
        assert!(is_uri("ms-settings:"));
        assert!(is_uri("steam://rungameid/1"));
        assert!(is_uri("mailto:me@example.com"));
        assert!(!is_uri(r"C:\Windows\notepad.exe"));
        assert!(!is_uri("notepad.exe"));
        assert!(!is_uri(r"\\server\share"));
    }

    #[test]
    fn explicit_icons() {
        let mut app = AppEntry::new("x", "https://example.com");
        app.icon = "https://example.com/i.png".into();
        assert_eq!(
            resolve(&app),
            IconSource::Uri("https://example.com/i.png".into())
        );

        app.icon = r"C:\icons\x.PNG".into();
        assert_eq!(
            resolve(&app),
            IconSource::Uri(r"file://C:\icons\x.PNG".into())
        );

        app.icon = r"C:\Program Files\x\x.exe".into();
        assert_eq!(
            resolve(&app),
            IconSource::Shell(PathBuf::from(r"C:\Program Files\x\x.exe"))
        );
    }

    #[test]
    fn implicit_icons() {
        assert_eq!(
            resolve(&AppEntry::new("x", "https://example.com")),
            IconSource::Fallback
        );
        assert_eq!(
            resolve(&AppEntry::new("x", "ms-settings:")),
            IconSource::Fallback
        );

        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("tool.exe");
        std::fs::write(&file, b"").unwrap();
        assert_eq!(
            resolve(&AppEntry::new("x", file.to_string_lossy())),
            IconSource::Shell(file.clone())
        );
        assert_eq!(
            resolve(&AppEntry::new(
                "x",
                dir.path().join("missing.exe").to_string_lossy()
            )),
            IconSource::Fallback
        );
    }

    #[cfg(windows)]
    #[test]
    fn finds_programs_on_path() {
        assert!(matches!(
            resolve(&AppEntry::new("cmd", "cmd.exe")),
            IconSource::Shell(_)
        ));
        assert!(matches!(
            resolve(&AppEntry::new("cmd", "cmd")),
            IconSource::Shell(_)
        ));
    }

    #[cfg(windows)]
    #[test]
    fn extracts_a_real_shell_icon() {
        let _com = platform::ComGuard::init();
        let cmd = find_in_path("cmd.exe").expect("cmd.exe on PATH");
        let image = platform::shell_icon(&cmd, 64).expect("icon for cmd.exe");
        assert!(image.size[0] >= 16 && image.size[1] >= 16);
        assert!(image.pixels.iter().any(|p| p.a() > 0));
    }
}
