//! The window loop and renderer: egui drawn on the CPU.
//!
//! egui's triangles are rasterized by `hdrshot-egui-backend` (a port of
//! `egui_software_backend`), which caches primitives in tiles and only
//! re-rasterizes what changed; `softbuffer` copies the pixels to the window
//! (GDI); `egui-winit` turns window messages into egui input and applies
//! viewport commands (title-bar drag, resize, maximize, minimize).
//!
//! Why not a GPU renderer (eframe glow/wgpu, removed): glow needs OpenGL 2+,
//! which GPU-less VMs and RDP sessions lack (no window at all), and wgpu there
//! runs on WARP (Direct3D emulated on the CPU): slow clicks and high memory.
//! Drawing on the CPU directly works the same on every machine and opens the
//! window immediately. Same runner as M365 Security Audit / Windows-Configurator.

use std::num::NonZeroU32;
use std::rc::Rc;
use std::time::{Duration, Instant};

use egui::{ViewportCommand, ViewportId};
use egui_software_backend::{BufferMutRef, ColorFieldOrder, EguiSoftwareRender};
use winit::application::ApplicationHandler;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::WindowEvent;
use winit::event_loop::{
    ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy, OwnedDisplayHandle,
};
use winit::window::{Window, WindowId};

use crate::app::LauncherApp as App;

/// Animation repaints (spinners) at most this often; input still repaints at
/// once. Without a cap a spinner keeps a whole core busy (there is no vsync).
const MIN_FRAME: Duration = Duration::from_millis(33);

/// egui asked for a repaint at `when`.
struct Repaint {
    when: Instant,
}

pub type MakeApp = Box<dyn FnOnce(&egui::Context) -> App>;

struct Running {
    window: Rc<Window>,
    surface: softbuffer::Surface<OwnedDisplayHandle, Rc<Window>>,
    input: egui_winit::State,
    app: App,
    renderer: EguiSoftwareRender,
    /// Window state for egui (rect, focus, scale), refreshed every frame.
    info: egui::ViewportInfo,
}

struct Runner {
    ctx: egui::Context,
    viewport: egui::ViewportBuilder,
    softbuffer: softbuffer::Context<OwnedDisplayHandle>,
    make_app: Option<MakeApp>,
    state: Option<Running>,
    next_repaint: Option<Instant>,
    last_paint: Option<Instant>,
    /// The app sent `ViewportCommand::Close` (close button, "close after launch").
    quit: bool,
    error: Option<String>,
}

impl Runner {
    fn start(&mut self, el: &ActiveEventLoop) -> Result<(), String> {
        let window = egui_winit::create_window(&self.ctx, el, &self.viewport)
            .map_err(|e| format!("create window: {e}"))?;
        let window = Rc::new(window);
        egui_winit::apply_viewport_builder_to_window(&self.ctx, &window, &self.viewport);
        let native_corners = platform::prepare_borderless(&window);
        crate::ui::window::set_native_corners(native_corners);
        keep_on_screen(el, &window);

        let surface = softbuffer::Surface::new(&self.softbuffer, window.clone())
            .map_err(|e| format!("softbuffer surface: {e}"))?;
        let input = egui_winit::State::new(
            self.ctx.clone(),
            ViewportId::ROOT,
            &window,
            Some(window.scale_factor() as f32),
            window.theme(),
            None,
        );
        let make_app = self.make_app.take().ok_or("app already created")?;
        let app = make_app(&self.ctx);
        window.set_visible(true);
        window.request_redraw();
        let mut info = egui::ViewportInfo::default();
        egui_winit::update_viewport_info(&mut info, &self.ctx, &window, true);
        self.state = Some(Running {
            window,
            surface,
            input,
            app,
            // softbuffer on Windows is 0RGB little-endian = B,G,R,X bytes
            renderer: EguiSoftwareRender::new(ColorFieldOrder::Bgra),
            info,
        });
        Ok(())
    }

    fn paint(&mut self) -> Result<(), String> {
        let Some(s) = self.state.as_mut() else {
            return Ok(());
        };
        let started = Instant::now();
        self.last_paint = Some(started);
        let size = s.window.inner_size();
        let (Some(w), Some(h)) = (NonZeroU32::new(size.width), NonZeroU32::new(size.height)) else {
            return Ok(()); // minimized
        };
        s.surface
            .resize(w, h)
            .map_err(|e| format!("softbuffer resize: {e}"))?;

        egui_winit::update_viewport_info(&mut s.info, &self.ctx, &s.window, false);
        let mut raw = s.input.take_egui_input(&s.window);
        raw.viewports.insert(ViewportId::ROOT, s.info.clone());
        let app = &mut s.app;
        let out = self.ctx.run_ui(raw, |ui| {
            app.logic(ui.ctx());
            app.ui(ui);
        });
        s.input
            .handle_platform_output(&s.window, out.platform_output);

        if let Some(vp) = out.viewport_output.get(&ViewportId::ROOT) {
            let (close, commands): (Vec<_>, Vec<_>) = vp
                .commands
                .iter()
                .cloned()
                .partition(|c| matches!(c, ViewportCommand::Close));
            self.quit |= !close.is_empty();
            // Drag/resize/maximize/minimize from our own title bar.
            let mut actions = Vec::new();
            egui_winit::process_viewport_commands(
                &self.ctx,
                &mut s.info,
                commands,
                &s.window,
                &mut actions,
            );
            if vp.repaint_delay < Duration::MAX {
                let when = (Instant::now() + vp.repaint_delay).max(started + MIN_FRAME);
                self.next_repaint = Some(self.next_repaint.map_or(when, |t| t.min(when)));
            }
        }

        let prims = self.ctx.tessellate(out.shapes, out.pixels_per_point);
        let mut deltas = out.textures_delta;
        let buffer = s.surface.buffer_mut();
        let result = buffer.map(|mut buffer| {
            buffer.fill(0);
            let pixels: &mut [[u8; 4]] = bytemuck::cast_slice_mut(&mut buffer);
            s.renderer.render(
                &mut BufferMutRef::new(pixels, w.get() as usize, h.get() as usize),
                &prims,
                &deltas,
                out.pixels_per_point,
            );
            buffer.present()
        });
        // epaint panics on dropping an unacknowledged delta; the renderer has applied it
        deltas.clear();
        result
            .map_err(|e| format!("softbuffer buffer: {e}"))?
            .map_err(|e| format!("softbuffer present: {e}"))?;
        Ok(())
    }

    /// Programmatic repaint (animation, background event): paced to `MIN_FRAME`.
    fn schedule(&mut self, when: Instant) {
        let when = self.last_paint.map_or(when, |t| when.max(t + MIN_FRAME));
        self.next_repaint = Some(self.next_repaint.map_or(when, |t| t.min(when)));
    }

    fn fail(&mut self, el: &ActiveEventLoop, e: String) {
        self.error = Some(e);
        el.exit();
    }
}

impl ApplicationHandler<Repaint> for Runner {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.state.is_none()
            && self.make_app.is_some()
            && let Err(e) = self.start(el)
        {
            self.fail(el, e);
        }
    }

    fn user_event(&mut self, _el: &ActiveEventLoop, ev: Repaint) {
        self.schedule(ev.when);
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(s) = self.state.as_mut() else {
            return;
        };
        match event {
            WindowEvent::RedrawRequested => {
                if let Err(e) = self.paint() {
                    self.fail(el, e);
                } else if self.quit {
                    el.exit();
                }
            }
            WindowEvent::CloseRequested => el.exit(),
            other => {
                if matches!(
                    other,
                    WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. }
                ) {
                    s.window.request_redraw();
                }
                if s.input.on_window_event(&s.window, &other).repaint {
                    s.window.request_redraw();
                }
            }
        }
    }

    fn about_to_wait(&mut self, el: &ActiveEventLoop) {
        match self.next_repaint {
            Some(t) if t <= Instant::now() => {
                self.next_repaint = None;
                if let Some(s) = &self.state {
                    s.window.request_redraw();
                }
                el.set_control_flow(ControlFlow::Wait);
            }
            Some(t) => el.set_control_flow(ControlFlow::WaitUntil(t)),
            None => el.set_control_flow(ControlFlow::Wait),
        }
    }
}

/// A restored position can point at a monitor that is no longer connected;
/// in that case centre the window on the primary monitor.
fn keep_on_screen(el: &ActiveEventLoop, window: &Window) {
    let Ok(pos) = window.outer_position() else {
        return;
    };
    let size = window.outer_size();
    let visible = el.available_monitors().any(|m| {
        let (mp, ms) = (m.position(), m.size());
        // At least the title bar's top-left 100x40 px must be on this monitor.
        pos.x + 100 > mp.x
            && pos.x < mp.x + ms.width as i32
            && pos.y + 40 > mp.y
            && pos.y < mp.y + ms.height as i32
    });
    if visible {
        return;
    }
    if let Some(m) = el
        .primary_monitor()
        .or_else(|| el.available_monitors().next())
    {
        let (mp, ms) = (m.position(), m.size());
        let size = PhysicalSize::new(size.width.min(ms.width), size.height.min(ms.height));
        window.set_outer_position(PhysicalPosition::new(
            mp.x + (ms.width - size.width) as i32 / 2,
            mp.y + (ms.height - size.height) as i32 / 2,
        ));
    }
}

/// Opens the window and runs the app until it closes.
pub fn run(viewport: egui::ViewportBuilder, make_app: MakeApp) -> Result<(), String> {
    let event_loop = EventLoop::<Repaint>::with_user_event()
        .build()
        .map_err(|e| format!("event loop: {e}"))?;
    let softbuffer = softbuffer::Context::new(event_loop.owned_display_handle())
        .map_err(|e| format!("softbuffer: {e}"))?;
    let ctx = egui::Context::default();
    let proxy: EventLoopProxy<Repaint> = event_loop.create_proxy();
    ctx.set_request_repaint_callback(move |info| {
        let _ = proxy.send_event(Repaint {
            when: Instant::now() + info.delay,
        });
    });
    let mut runner = Runner {
        ctx,
        // start hidden, shown once the first frame can be drawn (no white flash)
        viewport: viewport.with_visible(false),
        softbuffer,
        make_app: Some(make_app),
        state: None,
        next_repaint: None,
        last_paint: None,
        quit: false,
        error: None,
    };
    let result = event_loop
        .run_app(&mut runner)
        .map_err(|e| format!("event loop: {e}"));
    if let Some(mut s) = runner.state.take() {
        s.app.on_exit();
    }
    result?;
    match runner.error {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

#[cfg(windows)]
mod platform {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Dwm::{
        DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND, DwmSetWindowAttribute,
    };
    use winit::platform::windows::WindowExtWindows as _;
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use winit::window::Window;

    /// Prepares our undecorated window; returns true if Windows rounds its corners.
    ///
    /// * Turns off winit's undecorated-window shadow, which egui-winit enables:
    ///   it shifts the client area down 1 px, leaving a strip along the top
    ///   edge (egui issue #8399).
    /// * softbuffer presents with GDI, so there is no per-pixel transparency
    ///   to paint rounded corners ourselves. Windows 11 rounds them instead
    ///   (and adds its thin native border). On Windows 10 the attribute does
    ///   not exist, the call fails, and the corners stay square.
    pub fn prepare_borderless(window: &Window) -> bool {
        window.set_undecorated_shadow(false);
        let Ok(handle) = window.window_handle() else {
            return false;
        };
        let RawWindowHandle::Win32(win32) = handle.as_raw() else {
            return false;
        };
        let hwnd = HWND(win32.hwnd.get() as *mut core::ffi::c_void);
        let pref = DWMWCP_ROUND;
        // SAFETY: `hwnd` is our live top-level window and `pref` is a local
        // of exactly the size passed.
        let result = unsafe {
            DwmSetWindowAttribute(
                hwnd,
                DWMWA_WINDOW_CORNER_PREFERENCE,
                (&raw const pref).cast(),
                size_of_val(&pref) as u32,
            )
        };
        if let Err(e) = &result {
            log::debug!("no native rounded corners (pre-Windows 11?): {e}");
        }
        result.is_ok()
    }
}

#[cfg(not(windows))]
mod platform {
    pub fn prepare_borderless(_window: &winit::window::Window) -> bool {
        false
    }
}

/// Offscreen frames for tests: the same rasterizer as the window, into an RGBA image.
#[cfg(test)]
pub struct Offscreen {
    pub ctx: egui::Context,
    renderer: EguiSoftwareRender,
    size: [u32; 2],
    start: Instant,
}

#[cfg(test)]
impl Offscreen {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            ctx: egui::Context::default(),
            renderer: EguiSoftwareRender::new(ColorFieldOrder::Rgba),
            size: [width, height],
            start: Instant::now(),
        }
    }

    /// One frame of `app` at 1 point = 1 pixel; returns the pixels.
    pub fn frame(&mut self, app: &mut App) -> image::RgbaImage {
        let [w, h] = self.size;
        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(w as f32, h as f32),
            )),
            time: Some(self.start.elapsed().as_secs_f64()),
            ..Default::default()
        };
        let out = self.ctx.run_ui(raw, |ui| {
            app.logic(ui.ctx());
            app.ui(ui);
        });
        let prims = self.ctx.tessellate(out.shapes, out.pixels_per_point);
        let mut deltas = out.textures_delta;
        let mut pixels = vec![[0u8; 4]; (w * h) as usize];
        self.renderer.render(
            &mut BufferMutRef::new(&mut pixels, w as usize, h as usize),
            &prims,
            &deltas,
            out.pixels_per_point,
        );
        deltas.clear();
        image::RgbaImage::from_raw(w, h, pixels.into_flattened()).expect("buffer size")
    }
}
