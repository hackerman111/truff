//! Cross-platform native window runner built on `winit` + `egui-winit` + `softbuffer`.
//!
//! Renders frames via CPU shared-memory buffers (`wl_shm` on Wayland, `MIT-SHM` on X11,
//! GDI on Windows, CoreGraphics/CALayer on macOS) without loading OpenGL/EGL/Vulkan drivers.

use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::Write;
use std::num::NonZeroU32;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use egui::{ViewportBuilder, ViewportCommand, ViewportId, ViewportInfo};
use egui_winit::ActionRequested;
use egui_winit::winit::{
    application::ApplicationHandler,
    event::{DeviceEvent, DeviceId, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
};

use crate::app::Application;
use crate::native::NativeApp;
use crate::native::rasterizer::SoftwareRasterizer;

const EGUI_MEMORY_KEY: &str = "egui";
const WINDOW_KEY: &str = "window";

/// Key-value storage abstraction for persisting `egui` memory and window geometry.
pub trait Storage {
    fn get_string(&self, key: &str) -> Option<String>;
    fn set_string(&mut self, key: &str, value: String);
    fn remove_string(&mut self, key: &str);
    fn flush(&mut self);
}

/// Deserialize a RON-encoded value from `storage`.
pub fn get_value<T: serde::de::DeserializeOwned>(storage: &dyn Storage, key: &str) -> Option<T> {
    storage
        .get_string(key)
        .and_then(|value| ron::from_str(&value).ok())
}

/// Serialize a value as RON into `storage`.
pub fn set_value<T: serde::Serialize>(storage: &mut dyn Storage, key: &str, value: &T) {
    if let Ok(serialized) = ron::ser::to_string(value) {
        storage.set_string(key, serialized);
    }
}

struct FileStorage {
    path: PathBuf,
    kv: BTreeMap<String, String>,
    dirty: bool,
}

impl FileStorage {
    fn from_app_id(app_id: &str) -> Option<Self> {
        let dir = data_dir(app_id)?;
        let path = dir.join("app.ron");
        let kv = fs::read_to_string(&path)
            .ok()
            .and_then(|s| ron::from_str(&s).ok())
            .unwrap_or_default();
        Some(Self {
            path,
            kv,
            dirty: false,
        })
    }
}

impl Storage for FileStorage {
    fn get_string(&self, key: &str) -> Option<String> {
        self.kv.get(key).cloned()
    }

    fn set_string(&mut self, key: &str, value: String) {
        if self.kv.get(key) != Some(&value) {
            self.kv.insert(key.to_owned(), value);
            self.dirty = true;
        }
    }

    fn remove_string(&mut self, key: &str) {
        if self.kv.remove(key).is_some() {
            self.dirty = true;
        }
    }

    fn flush(&mut self) {
        if !self.dirty {
            return;
        }
        let Some(parent) = self.path.parent() else {
            return;
        };
        if fs::create_dir_all(parent).is_err() {
            return;
        }
        let Ok(serialized) = ron::ser::to_string(&self.kv) else {
            return;
        };
        let tmp_path = parent.join(format!(".app.ron.{}.tmp", std::process::id()));
        let write_ok = (|| -> std::io::Result<()> {
            let mut file = File::create(&tmp_path)?;
            file.write_all(serialized.as_bytes())?;
            file.sync_data()?;
            drop(file);
            fs::rename(&tmp_path, &self.path)?;
            Ok(())
        })();
        if write_ok.is_ok() {
            self.dirty = false;
        } else {
            let _ = fs::remove_file(&tmp_path);
        }
    }
}

fn data_dir(app_id: &str) -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .map(|p| p.join(app_id))
    }
    #[cfg(target_os = "macos")]
    {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .map(|p| p.join("Library/Application Support").join(app_id))
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .or_else(|| {
                std::env::var_os("HOME")
                    .map(PathBuf::from)
                    .map(|p| p.join(".local/share"))
            })
            .map(|p| p.join(app_id))
    }
}

#[derive(Debug, Clone, Copy)]
enum UserEvent {
    RequestRepaint(Instant),
}

struct WindowSurface {
    window: Arc<Window>,
    _context: softbuffer::Context<Arc<Window>>,
    surface: softbuffer::Surface<Arc<Window>, Arc<Window>>,
    winit_state: egui_winit::State,
}

struct SoftwareWinitApp {
    title: String,
    egui_ctx: egui::Context,
    app: NativeApp,
    storage: Option<FileStorage>,
    rasterizer: SoftwareRasterizer,
    viewport_info: ViewportInfo,
    window_surface: Option<WindowSurface>,
    next_repaint_time: Option<Instant>,
}

impl SoftwareWinitApp {
    fn save_state(&mut self) {
        let Some(storage) = self.storage.as_mut() else {
            return;
        };
        if let Some(ws) = &self.window_surface {
            let settings =
                egui_winit::WindowSettings::from_window(self.egui_ctx.zoom_factor(), &ws.window);
            set_value(storage, WINDOW_KEY, &settings);
        }
        if self.app.persist_egui_memory() {
            self.egui_ctx
                .memory(|mem| set_value(storage, EGUI_MEMORY_KEY, mem));
        }
        storage.flush();
    }

    fn render_frame(&mut self, event_loop: &ActiveEventLoop) {
        let Some(ws) = self.window_surface.as_mut() else {
            return;
        };

        egui_winit::update_viewport_info(
            &mut self.viewport_info,
            &self.egui_ctx,
            &ws.window,
            false,
        );

        let mut raw_input = ws.winit_state.take_egui_input(&ws.window);
        raw_input
            .viewports
            .insert(ViewportId::ROOT, self.viewport_info.clone());
        self.viewport_info.events.clear();

        let mut full_output = self.egui_ctx.run_ui(raw_input, |ui| {
            self.app.show(ui);
        });

        ws.winit_state.handle_platform_output_with_event_loop(
            &ws.window,
            event_loop,
            full_output.platform_output,
        );

        let mut should_close = false;
        let mut cancel_close = false;
        let mut actions_requested = Vec::new();

        if let Some(viewport_output) = full_output.viewport_output.get(&ViewportId::ROOT) {
            for cmd in &viewport_output.commands {
                match cmd {
                    ViewportCommand::Close => should_close = true,
                    ViewportCommand::CancelClose => cancel_close = true,
                    _ => {}
                }
            }
            egui_winit::process_viewport_commands(
                &self.egui_ctx,
                &mut self.viewport_info,
                viewport_output.commands.clone(),
                &ws.window,
                &mut actions_requested,
            );
            // `process_viewport_commands` pushes `ViewportEvent::Close` on `ViewportCommand::Close`.
            // Clear it if we handle closing immediately or if `CancelClose` was also issued.
            if cancel_close || should_close {
                self.viewport_info.events.clear();
            }

            if viewport_output.repaint_delay.is_zero() {
                self.next_repaint_time = Some(Instant::now());
            } else if let Some(deadline) = Instant::now().checked_add(viewport_output.repaint_delay)
            {
                self.next_repaint_time = Some(
                    self.next_repaint_time
                        .map_or(deadline, |existing| existing.min(deadline)),
                );
            }
        }

        for action in actions_requested {
            match action {
                ActionRequested::Cut => {
                    ws.winit_state
                        .egui_input_mut()
                        .events
                        .push(egui::Event::Cut);
                }
                ActionRequested::Copy => {
                    ws.winit_state
                        .egui_input_mut()
                        .events
                        .push(egui::Event::Copy);
                }
                ActionRequested::Paste => {
                    if let Some(contents) = ws.winit_state.clipboard_text() {
                        let contents = contents.replace("\r\n", "\n");
                        if !contents.is_empty() {
                            ws.winit_state
                                .egui_input_mut()
                                .events
                                .push(egui::Event::Paste(contents));
                        }
                    }
                }
                ActionRequested::Screenshot(_) => {}
            }
        }

        self.rasterizer
            .set_textures(&mut full_output.textures_delta);

        let clipped_primitives = self
            .egui_ctx
            .tessellate(full_output.shapes, full_output.pixels_per_point);

        let size = ws.window.inner_size();
        if let (Some(width), Some(height)) =
            (NonZeroU32::new(size.width), NonZeroU32::new(size.height))
            && ws.surface.resize(width, height).is_ok()
            && let Ok(mut buffer) = ws.surface.buffer_mut()
        {
            let clear_color =
                crate::theme::style::color(self.app.application.shell.theme().bg_canvas);
            self.rasterizer.paint_primitives(
                &mut buffer,
                width.get() as usize,
                height.get() as usize,
                full_output.pixels_per_point,
                clear_color,
                &clipped_primitives,
            );
            // Notify winit before presenting so Wayland frame callbacks pace redraws
            // non-blockingly without starving xdg_wm_base.ping -> pong in background.
            ws.window.pre_present_notify();
            let _ = buffer.present();
        }

        self.rasterizer
            .free_textures(&mut full_output.textures_delta);

        if should_close && !cancel_close {
            self.save_state();
            event_loop.exit();
        }
    }
}

impl ApplicationHandler<UserEvent> for SoftwareWinitApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window_surface.is_some() {
            return;
        }

        let mut viewport_builder = ViewportBuilder::default()
            .with_title(self.title.clone())
            .with_app_id("truff")
            .with_inner_size([1100.0, 700.0])
            .with_min_inner_size([640.0, 400.0]);

        if let Some(storage) = &self.storage
            && let Some(mut window_settings) =
                get_value::<egui_winit::WindowSettings>(storage, WINDOW_KEY)
        {
            window_settings.clamp_size_to_sane_values(egui::vec2(1920.0, 1080.0));
            window_settings.clamp_position_to_monitors(self.egui_ctx.zoom_factor(), event_loop);
            viewport_builder = window_settings.initialize_viewport_builder(
                self.egui_ctx.zoom_factor(),
                event_loop,
                viewport_builder,
            );
        }

        let window = match egui_winit::create_window(&self.egui_ctx, event_loop, &viewport_builder)
        {
            Ok(w) => Arc::new(w),
            Err(err) => {
                eprintln!("Failed to create window: {err}");
                event_loop.exit();
                return;
            }
        };

        let sb_context = match softbuffer::Context::new(window.clone()) {
            Ok(ctx) => ctx,
            Err(err) => {
                eprintln!("Failed to initialize software framebuffer context: {err}");
                event_loop.exit();
                return;
            }
        };

        let surface = match softbuffer::Surface::new(&sb_context, window.clone()) {
            Ok(surf) => surf,
            Err(err) => {
                eprintln!("Failed to create software framebuffer surface: {err}");
                event_loop.exit();
                return;
            }
        };

        egui_winit::update_viewport_info(&mut self.viewport_info, &self.egui_ctx, &window, true);

        let winit_state = egui_winit::State::new(
            self.egui_ctx.clone(),
            ViewportId::ROOT,
            event_loop,
            Some(window.scale_factor() as f32),
            window.theme(),
            Some(8192),
        );

        window.request_redraw();

        self.window_surface = Some(WindowSurface {
            window,
            _context: sb_context,
            surface,
            winit_state,
        });
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: UserEvent) {
        match event {
            UserEvent::RequestRepaint(deadline) => {
                if deadline <= Instant::now() {
                    self.next_repaint_time = None;
                    if let Some(ws) = &self.window_surface {
                        ws.window.request_redraw();
                    }
                } else {
                    self.next_repaint_time = Some(
                        self.next_repaint_time
                            .map_or(deadline, |existing| existing.min(deadline)),
                    );
                }
            }
        }
    }

    fn device_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _device_id: DeviceId,
        event: DeviceEvent,
    ) {
        if let DeviceEvent::MouseMotion { delta } = event
            && let Some(ws) = self.window_surface.as_mut()
        {
            ws.winit_state.on_mouse_motion(delta);
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        let Some(ws) = self.window_surface.as_mut() else {
            return;
        };
        if ws.window.id() != window_id {
            return;
        }

        match event {
            WindowEvent::CloseRequested => {
                self.viewport_info.events.push(egui::ViewportEvent::Close);
                ws.window.request_redraw();
            }
            WindowEvent::RedrawRequested => {
                self.render_frame(event_loop);
            }
            WindowEvent::Resized(_) => {
                let response = ws.winit_state.on_window_event(&ws.window, &event);
                let _ = response;
                ws.window.request_redraw();
            }
            _ => {
                let response = ws.winit_state.on_window_event(&ws.window, &event);
                if response.repaint {
                    ws.window.request_redraw();
                }
            }
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(deadline) = self.next_repaint_time {
            if Instant::now() >= deadline {
                self.next_repaint_time = None;
                if let Some(ws) = &self.window_surface {
                    ws.window.request_redraw();
                }
                event_loop.set_control_flow(ControlFlow::Wait);
            } else {
                event_loop.set_control_flow(ControlFlow::WaitUntil(deadline));
            }
        } else {
            event_loop.set_control_flow(ControlFlow::Wait);
        }
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        self.save_state();
        self.window_surface = None;
    }
}

/// Runs the native `truff` application window using pure-CPU shared-memory rendering.
pub fn run_native(title: &str, application: Application) -> Result<(), String> {
    let event_loop = EventLoop::<UserEvent>::with_user_event()
        .build()
        .map_err(|e| e.to_string())?;
    event_loop.set_control_flow(ControlFlow::Wait);

    let egui_ctx = egui::Context::default();
    let storage = FileStorage::from_app_id("truff");
    if let Some(ref storage) = storage
        && let Some(memory) = get_value::<egui::Memory>(storage, EGUI_MEMORY_KEY)
    {
        egui_ctx.memory_mut(|mem| *mem = memory);
    }

    let proxy = event_loop.create_proxy();
    egui_ctx.set_request_repaint_callback(move |info| {
        let deadline = Instant::now()
            .checked_add(info.delay.min(Duration::from_secs(86400)))
            .unwrap_or_else(Instant::now);
        let _ = proxy.send_event(UserEvent::RequestRepaint(deadline));
    });

    let app = NativeApp::new(application, &egui_ctx);

    let mut runner = SoftwareWinitApp {
        title: title.to_owned(),
        egui_ctx,
        app,
        storage,
        rasterizer: SoftwareRasterizer::new(),
        viewport_info: ViewportInfo::default(),
        window_surface: None,
        next_repaint_time: None,
    };

    event_loop.run_app(&mut runner).map_err(|e| e.to_string())
}
