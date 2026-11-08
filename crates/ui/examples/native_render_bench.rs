//! Full native CPU frame benchmark: egui UI pass, tessellation, and software rasterization
//! into a 1920x1080 framebuffer. Excludes the platform window and compositor presentation.
#[path = "support/counting_allocator.rs"]
mod counting_allocator;

use counting_allocator::CountingAllocator;
use egui::{Event, Modifiers, MouseWheelUnit, TouchPhase, vec2};
use std::time::Instant;
use truff_ui::native::rasterizer::SoftwareRasterizer;
use truff_ui::{app::Application, native::NativeApp, perf_scenarios::generate_bench_note};

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

const WIDTH: usize = 1920;
const HEIGHT: usize = 1080;
const PPP: f32 = 1.0;

#[derive(Default)]
struct Stage {
    ui: Vec<f64>,
    tessellate: Vec<f64>,
    raster: Vec<f64>,
    total: Vec<f64>,
    damaged_px: Vec<f64>,
    meshes: usize,
    triangles: usize,
}

struct Bench {
    context: egui::Context,
    app: NativeApp,
    rasterizer: SoftwareRasterizer,
    /// Simulated swapchain: `buffers[0]` is presented next; age rotates like a compositor.
    buffers: Vec<Vec<u32>>,
    ages: Vec<u8>,
    max_texture_side: usize,
}

impl Bench {
    fn new(swapchain_len: usize) -> Self {
        let context = egui::Context::default();
        let mut application = Application::new();
        application
            .shell
            .set_editor_text(&generate_bench_note(256 * 1024));
        let app = NativeApp::new(application, &context);
        Self {
            context,
            app,
            rasterizer: SoftwareRasterizer::new(),
            buffers: (0..swapchain_len)
                .map(|_| vec![0u32; WIDTH * HEIGHT])
                .collect(),
            ages: vec![0; swapchain_len],
            max_texture_side: std::env::var("TEXTURE_SIDE")
                .ok()
                .and_then(|side| side.parse().ok())
                .unwrap_or(truff_ui::native::runner::MAX_TEXTURE_SIDE),
        }
    }

    fn frame(&mut self, events: Vec<Event>, stage: &mut Stage) {
        let start = Instant::now();
        let mut output = self.context.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    vec2(WIDTH as f32 / PPP, HEIGHT as f32 / PPP),
                )),
                max_texture_side: Some(self.max_texture_side),
                events,
                ..Default::default()
            },
            |ui| self.app.show(ui),
        );
        let ui_done = Instant::now();
        self.rasterizer.set_textures(&mut output.textures_delta);
        let primitives = self
            .context
            .tessellate(output.shapes, output.pixels_per_point);
        let tess_done = Instant::now();
        stage.meshes = primitives.len();
        stage.triangles = primitives
            .iter()
            .map(|p| match &p.primitive {
                egui::epaint::Primitive::Mesh(mesh) => mesh.indices.len() / 3,
                egui::epaint::Primitive::Callback(_) => 0,
            })
            .sum();
        let clear = egui::Color32::from_rgb(0x2e, 0x34, 0x40);
        let age = self.ages[0];
        let damage = self.rasterizer.paint_frame(
            &mut self.buffers[0],
            WIDTH,
            HEIGHT,
            output.pixels_per_point,
            clear,
            &primitives,
            age,
        );
        let damaged_px: u64 = damage
            .iter()
            .map(|r| r.width.get() as u64 * r.height.get() as u64)
            .sum();
        let presented = !damage.is_empty() || age == 0;
        let raster_done = Instant::now();
        self.rasterizer.free_textures(&mut output.textures_delta);

        if presented {
            for a in &mut self.ages {
                if *a != 0 {
                    *a = a.saturating_add(1);
                }
            }
            self.ages[0] = 1;
            self.buffers.rotate_left(1);
            self.ages.rotate_left(1);
            stage.damaged_px.push(damaged_px as f64);
        } else {
            stage.damaged_px.push(0.0);
        }
        stage.ui.push(ms(ui_done - start));
        stage.tessellate.push(ms(tess_done - ui_done));
        stage.raster.push(ms(raster_done - tess_done));
        stage.total.push(ms(raster_done - start));
    }
}

fn ms(duration: std::time::Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}

fn summary(values: &mut [f64]) -> (f64, f64) {
    values.sort_by(f64::total_cmp);
    let p50 = values[values.len() / 2];
    let p95 = values[((values.len() as f64 * 0.95) as usize).min(values.len() - 1)];
    (p50, p95)
}

fn report(name: &str, stage: &mut Stage) {
    let (ui50, ui95) = summary(&mut stage.ui);
    let (te50, te95) = summary(&mut stage.tessellate);
    let (ra50, ra95) = summary(&mut stage.raster);
    let (to50, to95) = summary(&mut stage.total);
    let (px50, _) = summary(&mut stage.damaged_px);
    println!(
        "{name:<10} ui p50={ui50:6.3} p95={ui95:6.3} | tess p50={te50:6.3} p95={te95:6.3} | raster p50={ra50:6.3} p95={ra95:6.3} | total p50={to50:6.3} p95={to95:6.3} ms | damaged_px p50={px50:.0} meshes={} tris={}",
        stage.meshes, stage.triangles
    );
}

fn run(swapchain_len: usize) {
    println!("--- swapchain buffers: {swapchain_len} ---");
    let mut bench = Bench::new(swapchain_len);
    let mut warm = Stage::default();
    for _ in 0..20 {
        bench.frame(vec![], &mut warm);
    }
    let center = egui::pos2(WIDTH as f32 / PPP * 0.5, HEIGHT as f32 / PPP * 0.5);
    bench.frame(vec![Event::PointerMoved(center)], &mut warm);

    let mut idle = Stage::default();
    for _ in 0..200 {
        bench.frame(vec![], &mut idle);
    }
    report("idle", &mut idle);

    let mut cursor = Stage::default();
    for step in 0..200 {
        let text = if step % 2 == 0 { "j" } else { "k" };
        bench.frame(vec![Event::Text(text.into())], &mut cursor);
    }
    report("cursor", &mut cursor);

    let mut typing = Stage::default();
    bench.frame(vec![Event::Text("i".into())], &mut warm);
    for _ in 0..200 {
        bench.frame(vec![Event::Text("x".into())], &mut typing);
    }
    report("typing", &mut typing);
    bench.frame(
        vec![Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }],
        &mut warm,
    );

    let mut scroll = Stage::default();
    for _ in 0..200 {
        bench.frame(
            vec![Event::MouseWheel {
                unit: MouseWheelUnit::Point,
                delta: vec2(0.0, -24.0),
                phase: TouchPhase::Move,
                modifiers: Modifiers::NONE,
            }],
            &mut scroll,
        );
    }
    report("scroll", &mut scroll);

    let mut redraw = Stage::default();
    for _ in 0..100 {
        bench.ages.fill(0);
        bench.frame(vec![], &mut redraw);
    }
    report("full", &mut redraw);

    println!(
        "heap_live_mib={:.2} raster_texture_kib={}",
        counting_allocator::live() as f64 / (1024.0 * 1024.0),
        bench.rasterizer.texture_bytes() / 1024
    );
}

fn main() {
    run(1);
    run(2);
}
