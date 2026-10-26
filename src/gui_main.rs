#![cfg_attr(
    all(windows, not(feature = "gui-console")),
    windows_subsystem = "windows"
)]

//! Shan Shui Studio — a realtime desktop studio for the procedural landscape
//! generator.
//!
//! The generator is pure CPU code and deliberately kept free of UI concerns.
//! This front end owns a worker thread for previews (reusing one `Scene` and
//! the last shape list between jobs) and a second short-lived worker for
//! exporting full-resolution PNG and SVG files.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender, TryRecvError};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use eframe::egui;
use egui::{
    Align, Align2, Color32, CornerRadius, FontData, FontFamily, FontId, FontInsert, FontPriority,
    Frame, InsertFontFamily, Layout, Margin, Pos2, Rect, RichText, Sense, Shadow, Stroke,
    StrokeKind, TextEdit, TextWrapMode, Vec2,
};

use shan_shui::color::{get_palette, theme_swatch, Color as SceneColor};
use shan_shui::finish::Finish;
use shan_shui::render::{
    render_shapes, render_shapes_when, save_png, save_svg_sized, Bitmap, RenderOptions,
};
use shan_shui::rng::Seed;
use shan_shui::scene::{Scene, SceneOptions};
use shan_shui::shapes::Shape;
use shan_shui::theme_names;
use shan_shui::VERSION;

// ---------------------------------------------------------------------------
// Palette
// ---------------------------------------------------------------------------

const BG: Color32 = Color32::from_rgb(0x0F, 0x0D, 0x0A);
const PANEL: Color32 = Color32::from_rgb(0x17, 0x14, 0x10);
const PANEL_2: Color32 = Color32::from_rgb(0x1D, 0x19, 0x14);
const FIELD: Color32 = Color32::from_rgb(0x22, 0x1D, 0x17);
const FIELD_HOVER: Color32 = Color32::from_rgb(0x2A, 0x24, 0x1C);
const BORDER: Color32 = Color32::from_rgb(0x2E, 0x27, 0x1F);
const BORDER_HOVER: Color32 = Color32::from_rgb(0x44, 0x39, 0x2C);
const TEXT: Color32 = Color32::from_rgb(0xEC, 0xE4, 0xD5);
const MUTED: Color32 = Color32::from_rgb(0x9C, 0x90, 0x7D);
const FAINT: Color32 = Color32::from_rgb(0x6E, 0x64, 0x55);
const ACCENT: Color32 = Color32::from_rgb(0xC4, 0x50, 0x3A);
const ACCENT_HOVER: Color32 = Color32::from_rgb(0xD9, 0x67, 0x4D);
const ACCENT_PRESSED: Color32 = Color32::from_rgb(0xA8, 0x3F, 0x2C);
const ACCENT_SOFT: Color32 = Color32::from_rgb(0x6F, 0x32, 0x26);
const CREAM: Color32 = Color32::from_rgb(0xF8, 0xF1, 0xE3);
const SAGE: Color32 = Color32::from_rgb(0x91, 0xAB, 0x7C);
const ERROR: Color32 = Color32::from_rgb(0xE0, 0x7C, 0x68);

const WORLD_HEIGHT: f64 = 800.0;
const CHUNK_WIDTH: u32 = 512;
const PREVIEW_SUPERSAMPLE: u32 = 2;
const EXPORT_SUPERSAMPLE: u32 = 2;
const DRAFT_DEBOUNCE: Duration = Duration::from_millis(28);
const PREVIEW_DEBOUNCE: Duration = Duration::from_millis(180);
const CAMERA_MIN: f64 = -4000.0;
const CAMERA_MAX: f64 = 4000.0;
const CAMERA_RAIL_SPACE: f32 = 40.0;
/// The panorama is rendered once per composition. It covers every camera
/// position plus the widest viewport, so scrolling only moves a UV window.
const PANORAMA_SCALE: f64 = 0.9;
const PANORAMA_MARGIN: f64 = 64.0;
/// Keep the sun in the opening view and locked to the painting while the
/// camera travels. Birds are scattered across the whole scroll.
const PANORAMA_SUN_ANCHOR: (f64, f64) = (-350.0, 350.0);
const PANORAMA_DEBOUNCE: Duration = Duration::from_millis(420);
const SIDEBAR_WIDTH: f32 = 338.0;
const TOPBAR_HEIGHT: f32 = 70.0;
const EXPORT_FOOTER_HEIGHT: f32 = 286.0;

const DISPLAY_FAMILY: &str = "display";

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Shan Shui Studio")
            .with_inner_size([1480.0, 940.0])
            .with_min_inner_size([1120.0, 760.0])
            .with_icon(make_icon()),
        ..Default::default()
    };

    eframe::run_native(
        "Shan Shui Studio",
        options,
        Box::new(|cc| Ok(Box::new(StudioApp::new(cc)))),
    )
}

// ---------------------------------------------------------------------------
// Small data types
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Aspect {
    ThreeTwo,
    Wide,
    FourThree,
    Square,
    Portrait,
}

impl Aspect {
    const ALL: [Aspect; 5] = [
        Aspect::ThreeTwo,
        Aspect::Wide,
        Aspect::FourThree,
        Aspect::Square,
        Aspect::Portrait,
    ];

    fn label(self) -> &'static str {
        match self {
            Aspect::ThreeTwo => "3:2",
            Aspect::Wide => "16:9",
            Aspect::FourThree => "4:3",
            Aspect::Square => "1:1",
            Aspect::Portrait => "3:4",
        }
    }

    fn ratio(self) -> f64 {
        match self {
            Aspect::ThreeTwo => 3.0 / 2.0,
            Aspect::Wide => 16.0 / 9.0,
            Aspect::FourThree => 4.0 / 3.0,
            Aspect::Square => 1.0,
            Aspect::Portrait => 3.0 / 4.0,
        }
    }

    fn world_size(self) -> (f64, f64) {
        (WORLD_HEIGHT * self.ratio(), WORLD_HEIGHT)
    }

    fn preview_size(self) -> (u32, u32) {
        match self {
            Aspect::ThreeTwo => (1200, 800),
            Aspect::Wide => (1200, 675),
            Aspect::FourThree => (1200, 900),
            Aspect::Square => (1200, 1200),
            Aspect::Portrait => (900, 1200),
        }
    }

    fn export_size(self, long_edge: u32) -> (u32, u32) {
        let ratio = self.ratio();
        if ratio >= 1.0 {
            (long_edge, (long_edge as f64 / ratio).round() as u32)
        } else {
            ((long_edge as f64 * ratio).round() as u32, long_edge)
        }
    }
}

/// The camera is the horizontal centre of the composition in world units.
/// Generated chunks cover the whole painting, so travel only changes the view.
fn clamp_camera_value(camera: f64) -> f64 {
    camera.clamp(CAMERA_MIN, CAMERA_MAX)
}

/// Map a camera position to its 0..1 position on the travel rail.
fn camera_rail_fraction(camera: f64) -> f32 {
    ((clamp_camera_value(camera) - CAMERA_MIN) / (CAMERA_MAX - CAMERA_MIN)).clamp(0.0, 1.0) as f32
}

/// Map a 0..1 position on the travel rail back to world units.
fn camera_from_rail_fraction(fraction: f32) -> f64 {
    let fraction = fraction.clamp(0.0, 1.0) as f64;
    clamp_camera_value(CAMERA_MIN + fraction * (CAMERA_MAX - CAMERA_MIN))
}

/// World-space extent covered by the cached panorama. It spans every camera
/// position and the widest supported viewport, with a little breathing room so
/// fractional zooming never samples outside the texture.
fn panorama_world_range() -> (f64, f64) {
    let half_view = WORLD_HEIGHT * Aspect::Wide.ratio() * 0.5;
    (
        CAMERA_MIN - half_view - PANORAMA_MARGIN,
        CAMERA_MAX + half_view + PANORAMA_MARGIN,
    )
}

/// UV window into the panorama for the current camera and zoom.
fn panorama_uv_for(camera: f64, zoom: f32, world_width: f64) -> Rect {
    let (xmin, xmax) = panorama_world_range();
    let span = (xmax - xmin).max(1.0);
    let zoom = zoom.max(1.0) as f64;
    let center = ((clamp_camera_value(camera) - xmin) / span).clamp(0.0, 1.0);
    let half_u = (world_width / zoom / span * 0.5).clamp(0.0, 0.5);
    let half_v = (0.5 / zoom).clamp(0.0, 0.5);
    Rect::from_min_max(
        egui::pos2((center - half_u) as f32, (0.5 - half_v) as f32),
        egui::pos2((center + half_u) as f32, (0.5 + half_v) as f32),
    )
}

/// True when two specs differ only by camera position. Those changes can be
/// previewed entirely from the panorama, so no draft pass is needed.
fn differs_only_by_camera(a: &PreviewSpec, b: &PreviewSpec) -> bool {
    let mut a = a.clone();
    a.camera = b.camera;
    a == *b
}

fn theme_display(name: &str) -> &'static str {
    match name {
        "ink" => "Ink Wash",
        "qinglv" => "Qinglü",
        "jinbi" => "Jinbi",
        "autumn" => "Autumn",
        "snow" => "Snow",
        "dusk" => "Dusk",
        "pastel" => "Pastel",
        "jade" => "Jade",
        "nocturne" => "Nocturne",
        "sepia" => "Sepia",
        "porcelain" => "Porcelain",
        "blossom" => "Blossom",
        _ => "Ink Wash",
    }
}

fn finish_description(finish: Finish) -> &'static str {
    match finish {
        Finish::Wash => "Untouched paper and ink — the original look.",
        Finish::Crisp => "Lifted contrast, cleaner edges and cooler light.",
        Finish::Vivid => "Deeper pigment and richer shadow.",
        Finish::Vintage => "A warm fade, like a scroll left in the sun.",
        Finish::Noir => "Silver-gelatin monochrome.",
    }
}

/// Resolved state that decides exactly what the preview worker should draw.
#[derive(Clone, PartialEq)]
struct PreviewSpec {
    seed: Seed,
    theme: String,
    finish: Finish,
    aspect: Aspect,
    camera: f64,
    vegetation: bool,
    sky: bool,
    sun: bool,
    mist: bool,
    grain: bool,
    architecture: f64,
    people: f64,
    boats: f64,
    dust: f64,
    birds: i32,
    title: String,
    seal: String,
    title_size: f64,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PreviewQuality {
    Draft,
    Final,
}

impl PreviewSpec {
    fn scene_options(&self) -> SceneOptions {
        SceneOptions {
            seed: Some(self.seed.clone()),
            theme: self.theme.clone(),
            width: 1200,
            height: 800,
            camera: self.camera,
            chunk_width: CHUNK_WIDTH,
            vegetation: self.vegetation,
            architecture: self.architecture,
            people: self.people,
            boats: self.boats,
            dust: self.dust,
            water_style: "river".into(),
            sky: self.sky,
            sun: self.sun,
            mist: self.mist,
            birds: self.birds,
            title: self.title.clone(),
            seal: self.seal.clone(),
            title_size: self.title_size,
        }
    }
}

/// Everything in a preview that changes the painted panorama itself. Camera,
/// inscription and zoom are deliberately absent: camera travel is a UV window,
/// and the title/seal are drawn as a fixed overlay so they do not slide.
#[derive(Clone, PartialEq)]
struct PanoramaKey {
    seed: Seed,
    theme: String,
    finish: Finish,
    vegetation: bool,
    sky: bool,
    sun: bool,
    mist: bool,
    grain: bool,
    architecture: f64,
    people: f64,
    boats: f64,
    dust: f64,
    birds: i32,
}

impl PanoramaKey {
    fn from_spec(spec: &PreviewSpec) -> Self {
        Self {
            seed: spec.seed.clone(),
            theme: spec.theme.clone(),
            finish: spec.finish,
            vegetation: spec.vegetation,
            sky: spec.sky,
            sun: spec.sun,
            mist: spec.mist,
            grain: spec.grain,
            architecture: spec.architecture,
            people: spec.people,
            boats: spec.boats,
            dust: spec.dust,
            birds: spec.birds,
        }
    }

    /// The panorama renderer paints a wide strip without an inscription.
    fn scene_options(&self) -> SceneOptions {
        let (xmin, xmax) = panorama_world_range();
        SceneOptions {
            seed: Some(self.seed.clone()),
            theme: self.theme.clone(),
            width: (xmax - xmin).round().max(1.0) as u32,
            height: WORLD_HEIGHT as u32,
            camera: 0.0,
            chunk_width: CHUNK_WIDTH,
            vegetation: self.vegetation,
            architecture: self.architecture,
            people: self.people,
            boats: self.boats,
            dust: self.dust,
            water_style: "river".into(),
            sky: self.sky,
            sun: self.sun,
            mist: self.mist,
            birds: self.birds,
            title: String::new(),
            seal: String::new(),
            title_size: 24.0,
        }
    }
}

#[derive(Clone, PartialEq)]
struct SceneKey {
    seed: Seed,
    theme: String,
    chunk_width: u32,
    vegetation: bool,
    architecture: f64,
    people: f64,
    boats: f64,
    dust: f64,
}

impl SceneKey {
    fn from_options(options: &SceneOptions) -> Self {
        Self {
            seed: options.seed.clone().unwrap_or(Seed::Int(0)),
            theme: options.theme.clone(),
            chunk_width: options.chunk_width,
            vegetation: options.vegetation,
            architecture: options.architecture,
            people: options.people,
            boats: options.boats,
            dust: options.dust,
        }
    }
}

#[derive(Clone, PartialEq)]
struct ShapeKey {
    scene: SceneKey,
    sky: bool,
    sun: bool,
    mist: bool,
    birds: i32,
    title: String,
    seal: String,
    title_size: f64,
    world_width: u64,
    world_height: u64,
    camera: u64,
}

impl ShapeKey {
    fn new(scene: &SceneKey, options: &SceneOptions, world_w: f64, world_h: f64) -> Self {
        Self {
            scene: scene.clone(),
            sky: options.sky,
            sun: options.sun,
            mist: options.mist,
            birds: options.birds,
            title: options.title.clone(),
            seal: options.seal.clone(),
            title_size: options.title_size,
            world_width: world_w.to_bits(),
            world_height: world_h.to_bits(),
            camera: options.camera.to_bits(),
        }
    }
}

struct PreviewJob {
    seq: u64,
    spec: PreviewSpec,
    scene: SceneOptions,
    world_width: f64,
    world_height: f64,
    output_width: u32,
    output_height: u32,
    supersample: u32,
    draft: bool,
    finish: Finish,
    grain: bool,
    grain_seed: u64,
}

struct PreviewOutput {
    seq: u64,
    spec: PreviewSpec,
    bitmap: Bitmap,
    shapes: usize,
    elapsed_ms: u64,
    camera: f64,
    world_width: f64,
    draft: bool,
}

enum PreviewMsg {
    Ready(PreviewOutput),
    Cancelled,
    Error { seq: u64, message: String },
}

#[derive(Clone)]
struct PreviewInfo {
    shapes: usize,
    elapsed_ms: u64,
    draft: bool,
}

struct PanoramaJob {
    seq: u64,
    key: PanoramaKey,
    scene: SceneOptions,
    xmin: f64,
    xmax: f64,
    world_height: f64,
    output_width: u32,
    output_height: u32,
    finish: Finish,
    grain: bool,
    grain_seed: u64,
}

struct PanoramaOutput {
    seq: u64,
    key: PanoramaKey,
    bitmap: Bitmap,
    shapes: usize,
    elapsed_ms: u64,
}

enum PanoramaMsg {
    Ready(PanoramaOutput),
    Cancelled,
    Error { seq: u64, message: String },
}

struct PanoramaTexture {
    key: PanoramaKey,
    texture: egui::TextureHandle,
    shapes: usize,
    elapsed_ms: u64,
}

#[derive(Clone)]
struct ExportSettings {
    spec: PreviewSpec,
    world_width: f64,
    world_height: f64,
    output_width: u32,
    output_height: u32,
    supersample: u32,
    paths: Vec<PathBuf>,
    png: bool,
    svg: bool,
    grain: bool,
    grain_seed: u64,
}

struct ExportReport {
    paths: Vec<PathBuf>,
    elapsed_ms: u64,
    shapes: usize,
    png: bool,
    svg: bool,
}

enum ExportMsg {
    Stage(String),
    Done(ExportReport),
    Failed(String),
}

enum ExportState {
    Idle,
    Running { stage: String },
    Done(ExportReport),
    Failed(String),
}

// ---------------------------------------------------------------------------
// Application
// ---------------------------------------------------------------------------

struct StudioApp {
    display_font: FontFamily,

    // Composition settings.
    seed_text: String,
    theme: String,
    finish: Finish,
    aspect: Aspect,
    vegetation: bool,
    sky: bool,
    sun: bool,
    mist: bool,
    grain: bool,
    architecture: f64,
    people: f64,
    boats: f64,
    dust: f64,
    birds: i32,
    title: String,
    seal: String,
    title_size: f64,

    // View.
    zoom: f32,
    camera: f64,

    // Preview pipeline.
    preview_tx: Sender<PreviewJob>,
    preview_rx: Receiver<PreviewMsg>,
    latest_seq: Arc<AtomicU64>,
    preview_seq: u64,
    preview_inflight: bool,
    preview_inflight_quality: Option<PreviewQuality>,
    preview_draft_pending: bool,
    preview_final_pending: bool,
    preview_changed_at: Instant,
    preview_last_spec: Option<PreviewSpec>,
    preview_texture: Option<egui::TextureHandle>,
    texture_spec: Option<PreviewSpec>,
    preview_info: Option<PreviewInfo>,
    preview_error: Option<String>,
    texture_camera: f64,
    texture_world_width: f64,

    // Panorama pipeline. This is generated independently of the preview so
    // camera travel never has to wait for a raster pass.
    panorama_tx: Sender<PanoramaJob>,
    panorama_rx: Receiver<PanoramaMsg>,
    panorama_latest: Arc<AtomicU64>,
    panorama_seq: u64,
    panorama_inflight: bool,
    panorama_inflight_key: Option<PanoramaKey>,
    panorama_desired_key: Option<PanoramaKey>,
    panorama_changed_at: Instant,
    panorama_texture: Option<PanoramaTexture>,
    panorama_failed_key: Option<PanoramaKey>,
    panorama_error: Option<String>,

    // Export pipeline.
    export_base: String,
    export_name_follows_seed: bool,
    export_png: bool,
    export_svg: bool,
    export_long_edge: u32,
    export_state: ExportState,
    export_rx: Option<Receiver<ExportMsg>>,
}

impl StudioApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let display_font = install_fonts(&cc.egui_ctx);
        configure_style(&cc.egui_ctx, display_font.clone());

        let (preview_tx, preview_job_rx) = channel::<PreviewJob>();
        let (preview_out_tx, preview_rx) = channel::<PreviewMsg>();
        let latest_seq = Arc::new(AtomicU64::new(0));
        let worker_latest = latest_seq.clone();
        if let Err(error) = std::thread::Builder::new()
            .name("shan-shui-preview".into())
            .spawn(move || preview_worker(preview_job_rx, preview_out_tx, worker_latest))
        {
            eprintln!("shan-shui-gui: could not start preview worker: {error}");
        }

        let (panorama_tx, panorama_job_rx) = channel::<PanoramaJob>();
        let (panorama_out_tx, panorama_rx) = channel::<PanoramaMsg>();
        let panorama_latest = Arc::new(AtomicU64::new(0));
        let panorama_worker_latest = panorama_latest.clone();
        if let Err(error) = std::thread::Builder::new()
            .name("shan-shui-panorama".into())
            .spawn(move || {
                panorama_worker(panorama_job_rx, panorama_out_tx, panorama_worker_latest)
            })
        {
            eprintln!("shan-shui-gui: could not start panorama worker: {error}");
        }

        Self {
            display_font,
            seed_text: "42".into(),
            theme: "qinglv".into(),
            finish: Finish::Wash,
            aspect: Aspect::ThreeTwo,
            vegetation: true,
            sky: true,
            sun: true,
            mist: true,
            grain: true,
            architecture: 1.0,
            people: 1.0,
            boats: 1.0,
            dust: 1.0,
            birds: 5,
            title: "山水清音".into(),
            seal: "清".into(),
            title_size: 24.0,
            zoom: 1.0,
            camera: 0.0,
            preview_tx,
            preview_rx,
            latest_seq,
            preview_seq: 0,
            preview_inflight: false,
            preview_inflight_quality: None,
            preview_draft_pending: true,
            preview_final_pending: true,
            preview_changed_at: Instant::now(),
            preview_last_spec: None,
            preview_texture: None,
            texture_spec: None,
            preview_info: None,
            preview_error: None,
            texture_camera: 0.0,
            texture_world_width: 1200.0,
            panorama_tx,
            panorama_rx,
            panorama_latest,
            panorama_seq: 0,
            panorama_inflight: false,
            panorama_inflight_key: None,
            panorama_desired_key: None,
            panorama_changed_at: Instant::now(),
            panorama_texture: None,
            panorama_failed_key: None,
            panorama_error: None,
            export_base: default_export_base(&Seed::Int(42)),
            export_name_follows_seed: true,
            export_png: true,
            export_svg: true,
            export_long_edge: 2400,
            export_state: ExportState::Idle,
            export_rx: None,
        }
    }

    fn resolved_seed(&self) -> Seed {
        let trimmed = self.seed_text.trim();
        if trimmed.is_empty() {
            Seed::Int(42)
        } else {
            Seed::from_cli(trimmed)
        }
    }

    fn preview_spec(&self) -> PreviewSpec {
        PreviewSpec {
            seed: self.resolved_seed(),
            theme: self.theme.clone(),
            finish: self.finish,
            aspect: self.aspect,
            camera: self.camera,
            vegetation: self.vegetation,
            sky: self.sky,
            sun: self.sun,
            mist: self.mist,
            grain: self.grain,
            architecture: self.architecture,
            people: self.people,
            boats: self.boats,
            dust: self.dust,
            birds: self.birds,
            title: self.title.clone(),
            seal: self.seal.clone(),
            title_size: self.title_size,
        }
    }

    fn preview_busy(&self) -> bool {
        self.preview_inflight || self.preview_draft_pending || self.preview_final_pending
    }

    fn panorama_key(&self) -> PanoramaKey {
        PanoramaKey::from_spec(&self.preview_spec())
    }

    /// A panorama may be shown only while it still matches the current
    /// composition. Camera, inscription and zoom are handled outside the key.
    fn panorama_ready(&self) -> bool {
        self.panorama_texture
            .as_ref()
            .map(|panorama| panorama.key == self.panorama_key())
            .unwrap_or(false)
    }

    fn should_use_panorama(&self) -> bool {
        if !self.panorama_ready() {
            return false;
        }
        let current = self.preview_spec();
        let sharp_current = self.texture_spec.as_ref() == Some(&current)
            && !self
                .preview_info
                .as_ref()
                .map(|info| info.draft)
                .unwrap_or(true);
        !sharp_current
    }

    fn panorama_uv(&self) -> Rect {
        let (world_width, _) = self.aspect.world_size();
        panorama_uv_for(self.camera, self.zoom, world_width)
    }

    fn sync_export_name(&mut self) {
        if self.export_name_follows_seed {
            let seed = self.resolved_seed();
            self.export_base = default_export_base(&seed);
        }
    }

    fn choose_export_path(&mut self) {
        let extension = if self.export_png { "png" } else { "svg" };
        match pick_export_path(
            &self.export_base,
            extension,
            self.export_png && self.export_svg,
        ) {
            Ok(Some(path)) => {
                self.export_base = export_base_from_dialog(path);
                self.export_name_follows_seed = false;
            }
            Ok(None) => {}
            Err(error) => {
                self.export_state =
                    ExportState::Failed(format!("Could not open the save dialog — {error}"));
            }
        }
    }

    fn randomize_seed(&mut self) {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        let seed = (nanos % 999_979) as i64 + 1;
        self.seed_text = seed.to_string();
        self.sync_export_name();
    }

    fn reset_view(&mut self) {
        self.zoom = 1.0;
    }

    fn clamp_camera(&mut self) {
        self.camera = clamp_camera_value(self.camera);
    }

    fn view_uv(&self) -> Rect {
        let half = 0.5 / self.zoom.max(1.0);
        Rect::from_center_size(egui::pos2(0.5, 0.5), egui::vec2(half * 2.0, half * 2.0))
    }

    // -- background workers -------------------------------------------------

    fn poll_preview(&mut self, ctx: &egui::Context) {
        let mut disconnected = false;
        loop {
            match self.preview_rx.try_recv() {
                Ok(PreviewMsg::Ready(output)) => {
                    // With a single job in flight a finished draft is always the
                    // newest image available, even if the camera moved while it
                    // was being drawn. Showing it beats showing nothing.
                    self.preview_inflight = false;
                    self.preview_inflight_quality = None;
                    self.preview_error = None;
                    let image = egui::ColorImage::from_rgba_unmultiplied(
                        [output.bitmap.width as usize, output.bitmap.height as usize],
                        &output.bitmap.pixels,
                    );
                    self.preview_texture = Some(ctx.load_texture(
                        format!("shan-shui-preview-{}", output.seq),
                        image,
                        egui::TextureOptions::LINEAR,
                    ));
                    self.preview_info = Some(PreviewInfo {
                        shapes: output.shapes,
                        elapsed_ms: output.elapsed_ms,
                        draft: output.draft,
                    });
                    self.texture_spec = Some(output.spec.clone());
                    self.texture_camera = output.camera;
                    self.texture_world_width = output.world_width;
                    ctx.request_repaint();
                }
                Ok(PreviewMsg::Cancelled) => {
                    self.preview_inflight = false;
                    self.preview_inflight_quality = None;
                    ctx.request_repaint();
                }
                Ok(PreviewMsg::Error { seq, message }) => {
                    self.preview_inflight = false;
                    self.preview_inflight_quality = None;
                    if seq == self.preview_seq {
                        self.preview_error = Some(message);
                    }
                    ctx.request_repaint();
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    disconnected = true;
                    break;
                }
            }
        }

        if disconnected {
            self.preview_inflight = false;
            self.preview_inflight_quality = None;
            self.preview_draft_pending = false;
            self.preview_final_pending = false;
            if self.preview_error.is_none() {
                self.preview_error = Some("The preview worker stopped unexpectedly.".into());
            }
        }
    }

    fn poll_panorama(&mut self, ctx: &egui::Context) {
        let mut disconnected = false;
        loop {
            match self.panorama_rx.try_recv() {
                Ok(PanoramaMsg::Ready(output)) => {
                    self.panorama_inflight = false;
                    self.panorama_inflight_key = None;
                    self.panorama_error = None;
                    self.panorama_failed_key = None;
                    if output.key == self.panorama_key() {
                        let image = egui::ColorImage::from_rgba_unmultiplied(
                            [output.bitmap.width as usize, output.bitmap.height as usize],
                            &output.bitmap.pixels,
                        );
                        let texture = ctx.load_texture(
                            format!("shan-shui-panorama-{}", output.seq),
                            image,
                            egui::TextureOptions::LINEAR,
                        );
                        self.panorama_texture = Some(PanoramaTexture {
                            key: output.key,
                            texture,
                            shapes: output.shapes,
                            elapsed_ms: output.elapsed_ms,
                        });
                    }
                    ctx.request_repaint();
                }
                Ok(PanoramaMsg::Cancelled) => {
                    self.panorama_inflight = false;
                    self.panorama_inflight_key = None;
                    ctx.request_repaint();
                }
                Ok(PanoramaMsg::Error { seq, message }) => {
                    self.panorama_inflight = false;
                    self.panorama_inflight_key = None;
                    if seq == self.panorama_seq {
                        self.panorama_failed_key = self.panorama_desired_key.clone();
                        self.panorama_error = Some(message);
                    }
                    ctx.request_repaint();
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    disconnected = true;
                    break;
                }
            }
        }

        if disconnected {
            self.panorama_inflight = false;
            self.panorama_inflight_key = None;
            if self.panorama_error.is_none() {
                self.panorama_error = Some("The panorama worker stopped unexpectedly.".into());
            }
        }
    }

    fn poll_export(&mut self) {
        let Some(rx) = self.export_rx.as_ref() else {
            return;
        };
        let mut messages = Vec::new();
        let mut disconnected = false;
        loop {
            match rx.try_recv() {
                Ok(message) => messages.push(message),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    disconnected = true;
                    break;
                }
            }
        }

        let mut finished = false;
        for message in messages {
            match message {
                ExportMsg::Stage(stage) => {
                    if let ExportState::Running { stage: current } = &mut self.export_state {
                        *current = stage;
                    }
                }
                ExportMsg::Done(report) => {
                    self.export_state = ExportState::Done(report);
                    finished = true;
                }
                ExportMsg::Failed(error) => {
                    self.export_state = ExportState::Failed(error);
                    finished = true;
                }
            }
        }

        if !finished && disconnected {
            if matches!(self.export_state, ExportState::Running { .. }) {
                self.export_state =
                    ExportState::Failed("The export worker stopped unexpectedly.".into());
            }
            finished = true;
        }

        if finished {
            self.export_rx = None;
        }
    }

    fn schedule_preview(&mut self, ctx: &egui::Context) {
        let spec = self.preview_spec();
        if self.preview_last_spec.as_ref() != Some(&spec) {
            let previous = self.preview_last_spec.replace(spec.clone());
            let camera_only = previous
                .as_ref()
                .map(|old| differs_only_by_camera(old, &spec))
                .unwrap_or(false);
            self.preview_changed_at = Instant::now();
            self.preview_final_pending = true;
            // Once the panorama exists, a camera-only change can be shown by
            // sliding the UV window. Skip the low-quality pass and wait for
            // the idle sharpening render instead.
            self.preview_draft_pending = !(camera_only && self.panorama_ready());
            // A high-quality render can take the best part of a second. Abort
            // it as soon as the composition changes so the next draft is not
            // stuck behind stale work. Drafts are allowed to finish: a frame
            // that is a few milliseconds old is still useful feedback.
            if self.preview_inflight_quality == Some(PreviewQuality::Final) {
                self.latest_seq.store(0, Ordering::Relaxed);
            }
        }

        let pointer_down = ctx.input(|input| input.pointer.any_down());
        let elapsed = self.preview_changed_at.elapsed();

        if !self.preview_inflight {
            if self.preview_final_pending && !pointer_down && elapsed >= PREVIEW_DEBOUNCE {
                let spec = self.preview_last_spec.clone().expect("preview spec");
                self.start_preview(spec, PreviewQuality::Final);
            } else if self.preview_draft_pending && (pointer_down || elapsed >= DRAFT_DEBOUNCE) {
                let spec = self.preview_last_spec.clone().expect("preview spec");
                self.start_preview(spec, PreviewQuality::Draft);
            }
        }

        let mut wake_in: Option<Duration> = None;
        let mut consider = |deadline: Duration| {
            let delay = deadline.saturating_sub(elapsed);
            wake_in = Some(wake_in.map_or(delay, |current| current.min(delay)));
        };
        if self.preview_draft_pending && !self.preview_inflight {
            consider(DRAFT_DEBOUNCE);
        }
        if self.preview_final_pending && !self.preview_inflight {
            consider(PREVIEW_DEBOUNCE);
        }
        if self.preview_inflight {
            let delay = Duration::from_millis(30);
            wake_in = Some(wake_in.map_or(delay, |current| current.min(delay)));
        }
        if let Some(delay) = wake_in {
            ctx.request_repaint_after(delay.max(Duration::from_millis(8)));
        }
    }

    fn schedule_panorama(&mut self, ctx: &egui::Context) {
        let key = self.panorama_key();
        let ready = self
            .panorama_texture
            .as_ref()
            .map(|panorama| panorama.key == key)
            .unwrap_or(false);

        if ready {
            if self.panorama_inflight && self.panorama_inflight_key.as_ref() != Some(&key) {
                self.panorama_latest.store(0, Ordering::Relaxed);
            }
            return;
        }

        if self.panorama_failed_key.as_ref() == Some(&key) {
            return;
        }

        if self.panorama_inflight {
            if self.panorama_inflight_key.as_ref() != Some(&key) {
                // Let the current job stop at its next cancellation point.
                self.panorama_latest.store(0, Ordering::Relaxed);
            }
            ctx.request_repaint_after(Duration::from_millis(80));
            return;
        }

        if self.panorama_desired_key.as_ref() != Some(&key) {
            self.panorama_desired_key = Some(key.clone());
            self.panorama_changed_at = Instant::now();
        }

        let elapsed = self.panorama_changed_at.elapsed();
        if elapsed >= PANORAMA_DEBOUNCE {
            self.start_panorama(key);
        } else {
            ctx.request_repaint_after((PANORAMA_DEBOUNCE - elapsed).max(Duration::from_millis(16)));
        }
    }

    fn start_panorama(&mut self, key: PanoramaKey) {
        let (xmin, xmax) = panorama_world_range();
        let world_width = xmax - xmin;
        let output_width = (world_width * PANORAMA_SCALE).round().max(1.0) as u32;
        let output_height = (WORLD_HEIGHT * PANORAMA_SCALE).round().max(1.0) as u32;

        self.panorama_seq += 1;
        let seq = self.panorama_seq;
        let job = PanoramaJob {
            seq,
            scene: key.scene_options(),
            xmin,
            xmax,
            world_height: WORLD_HEIGHT,
            output_width,
            output_height,
            finish: key.finish,
            grain: key.grain,
            grain_seed: grain_seed(&key.seed),
            key: key.clone(),
        };

        self.panorama_latest.store(seq, Ordering::Relaxed);
        self.panorama_inflight = true;
        self.panorama_inflight_key = Some(key.clone());
        self.panorama_error = None;
        if self.panorama_tx.send(job).is_err() {
            self.panorama_inflight = false;
            self.panorama_inflight_key = None;
            self.panorama_failed_key = Some(key);
            self.panorama_error = Some("Could not reach the panorama worker.".into());
        }
    }

    fn start_preview(&mut self, spec: PreviewSpec, quality: PreviewQuality) {
        let (world_width, world_height) = spec.aspect.world_size();
        let (full_width, full_height) = spec.aspect.preview_size();
        let (output_width, output_height, supersample, draft) = match quality {
            PreviewQuality::Draft => ((full_width / 2).max(1), (full_height / 2).max(1), 1, true),
            PreviewQuality::Final => (full_width, full_height, PREVIEW_SUPERSAMPLE, false),
        };
        self.preview_seq += 1;
        let seq = self.preview_seq;
        let grain_seed = grain_seed(&spec.seed);
        let job = PreviewJob {
            seq,
            spec: spec.clone(),
            scene: spec.scene_options(),
            world_width,
            world_height,
            output_width,
            output_height,
            supersample,
            draft,
            finish: spec.finish,
            grain: spec.grain,
            grain_seed,
        };

        self.latest_seq.store(seq, Ordering::Relaxed);
        self.preview_inflight = true;
        self.preview_inflight_quality = Some(quality);
        self.preview_error = None;
        match quality {
            PreviewQuality::Draft => self.preview_draft_pending = false,
            PreviewQuality::Final => {
                self.preview_draft_pending = false;
                self.preview_final_pending = false;
            }
        }
        if self.preview_tx.send(job).is_err() {
            self.preview_inflight = false;
            self.preview_inflight_quality = None;
            self.preview_draft_pending = false;
            self.preview_final_pending = false;
            self.preview_error = Some("Could not reach the preview worker.".into());
        }
    }

    fn start_export(&mut self, ctx: &egui::Context) {
        if matches!(self.export_state, ExportState::Running { .. }) {
            return;
        }
        if !self.export_png && !self.export_svg {
            self.export_png = true;
        }

        let spec = self.preview_spec();
        let (world_width, world_height) = spec.aspect.world_size();
        let (output_width, output_height) = spec.aspect.export_size(self.export_long_edge);
        let paths = output_paths(&self.export_base, self.export_png, self.export_svg);
        if paths.is_empty() {
            self.export_state = ExportState::Failed("Choose at least one output format.".into());
            return;
        }

        let settings = ExportSettings {
            world_width,
            world_height,
            output_width,
            output_height,
            supersample: EXPORT_SUPERSAMPLE,
            paths,
            png: self.export_png,
            svg: self.export_svg,
            grain: self.grain,
            grain_seed: grain_seed(&spec.seed),
            spec,
        };

        let (tx, rx) = channel::<ExportMsg>();
        self.export_rx = Some(rx);
        self.export_state = ExportState::Running {
            stage: "Preparing the paper…".into(),
        };

        if let Err(error) = std::thread::Builder::new()
            .name("shan-shui-export".into())
            .spawn(move || export_worker(settings, tx))
        {
            self.export_state =
                ExportState::Failed(format!("Could not start the export worker: {error}"));
            self.export_rx = None;
        } else {
            ctx.request_repaint();
        }
    }

    // -- keyboard -----------------------------------------------------------

    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        let (randomize, export, fit) = ctx.input(|input| {
            (
                input.modifiers.ctrl && input.key_pressed(egui::Key::R),
                input.modifiers.ctrl && input.key_pressed(egui::Key::E),
                input.key_pressed(egui::Key::F)
                    && !input.modifiers.ctrl
                    && !input.modifiers.alt
                    && !input.modifiers.shift,
            )
        });
        if randomize {
            self.randomize_seed();
        }
        if export {
            self.start_export(ctx);
        }
        if fit {
            self.reset_view();
        }
    }

    // -- top bar ------------------------------------------------------------

    fn top_bar(&mut self, ui: &mut egui::Ui) {
        ui.allocate_ui_with_layout(
            egui::vec2(ui.available_width(), TOPBAR_HEIGHT),
            Layout::left_to_right(Align::Center),
            |ui| {
                paint_brand(ui, &self.display_font);

                if ui.available_width() > 850.0 {
                    ui.add_space(18.0);
                    let divider = Rect::from_center_size(
                        egui::pos2(ui.cursor().left(), ui.cursor().center().y),
                        egui::vec2(1.0, 28.0),
                    );
                    ui.painter().rect_filled(divider, 0, BORDER);
                    ui.add_space(30.0);
                    ui.label(
                        RichText::new("Landscapes grown from a single seed.")
                            .font(FontId::new(13.0, self.display_font.clone()))
                            .italics()
                            .color(FAINT),
                    );
                }

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let running = matches!(self.export_state, ExportState::Running { .. });
                    let export_label = if running {
                        "Exporting…"
                    } else if self.export_png && self.export_svg {
                        "Export painting"
                    } else if self.export_png {
                        "Export PNG"
                    } else {
                        "Export SVG"
                    };

                    if action_button(
                        ui,
                        egui::vec2(152.0, 36.0),
                        export_label,
                        ButtonKind::Primary,
                        Some("Ctrl+E"),
                        !running,
                    )
                    .clicked()
                    {
                        self.start_export(ui.ctx());
                    }

                    ui.add_space(8.0);

                    if action_button(
                        ui,
                        egui::vec2(108.0, 36.0),
                        "Randomise",
                        ButtonKind::Secondary,
                        None,
                        true,
                    )
                    .on_hover_text("New seed  ·  Ctrl+R")
                    .clicked()
                    {
                        self.randomize_seed();
                    }

                    ui.add_space(16.0);
                    paint_status_pill(self, ui);
                });
            },
        );
    }

    fn status_text(&self) -> (String, Color32) {
        if self.preview_error.is_some() {
            ("Preview unavailable".into(), ERROR)
        } else if self.preview_busy() {
            let text = match &self.preview_info {
                None => "Grinding ink…",
                Some(info) if info.draft => "Roughing in…",
                Some(_) => "Refining ink…",
            };
            (text.into(), ACCENT_HOVER)
        } else if let Some(info) = &self.preview_info {
            (
                format!(
                    "{} shapes · {:.2} s{}",
                    thousands(info.shapes),
                    info.elapsed_ms as f64 / 1000.0,
                    if info.draft { " · draft" } else { "" }
                ),
                if info.draft { MUTED } else { SAGE },
            )
        } else {
            ("Preparing paper…".into(), MUTED)
        }
    }

    // -- sidebar ------------------------------------------------------------

    fn sidebar(&mut self, ui: &mut egui::Ui) {
        let available_height = ui.available_height();
        let scroll_height = (available_height - EXPORT_FOOTER_HEIGHT).max(120.0);

        ui.allocate_ui_with_layout(
            egui::vec2(ui.available_width(), scroll_height),
            Layout::top_down(Align::Min),
            |ui| {
                ui.set_min_height(scroll_height);
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .max_height(scroll_height)
                    .show(ui, |ui| {
                        self.sidebar_content(ui);
                    });
            },
        );

        ui.add_space(2.0);
        ui.separator();
        ui.add_space(10.0);
        self.export_footer(ui);
    }

    fn sidebar_content(&mut self, ui: &mut egui::Ui) {
        ui.style_mut().wrap_mode = Some(TextWrapMode::Wrap);
        ui.spacing_mut().item_spacing.y = 0.0;

        // Seed -------------------------------------------------------------
        section_label(ui, "Seed");
        ui.add_space(9.0);
        let mut randomise = false;
        let mut seed_changed = false;
        ui.horizontal(|ui| {
            let field_width = (ui.available_width() - 70.0).max(90.0);
            ui.allocate_ui_with_layout(
                egui::vec2(field_width, 32.0),
                Layout::left_to_right(Align::Center),
                |ui| {
                    seed_changed = text_field(ui, &mut self.seed_text, "42", 48).changed();
                },
            );
            ui.add_space(7.0);
            if action_button(
                ui,
                egui::vec2(ui.available_width(), 32.0),
                "Random",
                ButtonKind::Secondary,
                None,
                true,
            )
            .on_hover_text("Generate a random seed  ·  Ctrl+R")
            .clicked()
            {
                randomise = true;
            }
        });
        if randomise {
            self.randomize_seed();
        } else if seed_changed {
            self.sync_export_name();
        }
        ui.add_space(6.0);
        ui.label(
            RichText::new("Numbers and words are both valid seeds.")
                .font(FontId::proportional(9.5))
                .color(FAINT),
        );

        section_gap(ui);

        // Canvas -----------------------------------------------------------
        section_label(ui, "Canvas");
        ui.add_space(9.0);
        let mut picked_aspect = None;
        let avail = ui.available_width();
        let gap = 5.0;
        let chip_width = ((avail - gap * 4.0) / 5.0).max(38.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            for aspect in Aspect::ALL {
                if chip(
                    ui,
                    aspect.label(),
                    aspect == self.aspect,
                    chip_width,
                    27.0,
                    11.0,
                )
                .clicked()
                {
                    picked_aspect = Some(aspect);
                }
            }
        });
        if let Some(aspect) = picked_aspect {
            if aspect != self.aspect {
                self.aspect = aspect;
                self.reset_view();
            }
        }
        let (preview_w, preview_h) = self.aspect.preview_size();
        ui.add_space(7.0);
        ui.label(
            RichText::new(format!("{preview_w} × {preview_h} preview · live crop"))
                .font(FontId::proportional(9.5))
                .color(FAINT),
        );

        section_gap(ui);

        // Camera -----------------------------------------------------------
        section_label(ui, "Camera");
        ui.add_space(8.0);
        if slider_row(
            ui,
            "Travel",
            &mut self.camera,
            CAMERA_MIN,
            CAMERA_MAX,
            &|value| format!("{:.0}", value.round()),
        ) {
            self.clamp_camera();
        }
        ui.add_space(7.0);
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("Drag the painting or use the rail below.")
                    .font(FontId::proportional(9.5))
                    .color(FAINT),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ghost_button(ui, "Centre").clicked() {
                    self.camera = 0.0;
                }
            });
        });

        section_gap(ui);

        // Palette ----------------------------------------------------------
        section_label(ui, "Palette");
        ui.add_space(9.0);
        let mut picked_theme = None;
        let names = theme_names();
        for pair in names.chunks(2) {
            ui.columns(2, |columns| {
                for (index, name) in pair.iter().enumerate() {
                    if let Some(column) = columns.get_mut(index) {
                        if theme_card(column, name, self.theme == *name) {
                            picked_theme = Some((*name).to_string());
                        }
                    }
                }
            });
            ui.add_space(6.0);
        }
        if let Some(theme) = picked_theme {
            self.theme = theme;
        }

        section_gap(ui);

        // Atmosphere -------------------------------------------------------
        section_label(ui, "Atmosphere");
        ui.add_space(8.0);
        toggle_row(ui, "Vegetation", &mut self.vegetation);
        toggle_row(ui, "Sky gradient", &mut self.sky);
        toggle_row(ui, "Sun", &mut self.sun);
        toggle_row(ui, "Mist", &mut self.mist);
        toggle_row(ui, "Paper grain", &mut self.grain);

        ui.add_space(8.0);
        let mut birds = self.birds as f64;
        if slider_row(ui, "Birds", &mut birds, 0.0, 20.0, &|value| {
            format!("{}", value.round() as i32)
        }) {
            self.birds = birds.round() as i32;
        }

        section_gap(ui);

        // Inscription ------------------------------------------------------
        section_label(ui, "Inscription");
        ui.add_space(8.0);
        field_caption(ui, "Title");
        ui.add_space(3.0);
        text_field(ui, &mut self.title, "山水清音", 48);
        ui.add_space(8.0);
        field_caption(ui, "Seal");
        ui.add_space(3.0);
        text_field(ui, &mut self.seal, "清", 4);
        ui.add_space(9.0);
        let mut title_size = self.title_size;
        if slider_row(ui, "Title size", &mut title_size, 12.0, 48.0, &|value| {
            format!("{:.0}", value.round())
        }) {
            self.title_size = title_size.round();
        }

        section_gap(ui);

        // Densities --------------------------------------------------------
        section_label(ui, "Density");
        ui.add_space(8.0);
        slider_row(
            ui,
            "Architecture",
            &mut self.architecture,
            0.0,
            1.0,
            &percent,
        );
        slider_row(ui, "People", &mut self.people, 0.0, 1.0, &percent);
        slider_row(ui, "Boats", &mut self.boats, 0.0, 1.0, &percent);
        slider_row(ui, "Dust", &mut self.dust, 0.0, 1.0, &percent);

        section_gap(ui);

        // Finish -----------------------------------------------------------
        section_label(ui, "Finish");
        ui.add_space(9.0);
        let avail = ui.available_width();
        let gap = 5.0;
        let chip_width = ((avail - gap * 4.0) / 5.0).max(40.0);
        let mut picked_finish = None;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            for finish in Finish::ALL {
                if chip(
                    ui,
                    finish.name(),
                    finish == self.finish,
                    chip_width,
                    27.0,
                    9.5,
                )
                .clicked()
                {
                    picked_finish = Some(finish);
                }
            }
        });
        if let Some(finish) = picked_finish {
            self.finish = finish;
        }
        ui.add_space(8.0);
        ui.label(
            RichText::new(finish_description(self.finish))
                .font(FontId::proportional(10.0))
                .color(FAINT),
        );

        ui.add_space(24.0);
        ui.label(
            RichText::new("A Rust studio for {Shan, Shui}*")
                .font(FontId::proportional(9.5))
                .color(FAINT),
        );
        ui.add_space(3.0);
        ui.label(
            RichText::new(format!("v{VERSION} · CPU raster · GPU composite"))
                .font(FontId::proportional(9.0))
                .color(Color32::from_rgb(0x55, 0x4D, 0x42)),
        );
        ui.add_space(10.0);
    }

    fn export_footer(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing.y = 0.0;
        ui.horizontal(|ui| {
            section_label_inline(ui, "Export");
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(
                    RichText::new("FULL FRAME")
                        .font(FontId::proportional(8.5))
                        .color(FAINT),
                );
            });
        });
        ui.add_space(8.0);

        // Formats.
        let gap = 6.0;
        let format_width = (ui.available_width() - gap) / 2.0;
        let mut toggle_png = false;
        let mut toggle_svg = false;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            if chip(ui, "PNG", self.export_png, format_width, 27.0, 10.5).clicked() {
                toggle_png = true;
            }
            if chip(ui, "SVG", self.export_svg, format_width, 27.0, 10.5).clicked() {
                toggle_svg = true;
            }
        });
        if toggle_png {
            self.export_png = !self.export_png;
        }
        if toggle_svg {
            self.export_svg = !self.export_svg;
        }
        if !self.export_png && !self.export_svg {
            self.export_png = true;
        }

        ui.add_space(7.0);
        field_caption(ui, "Long edge");
        ui.add_space(4.0);
        let gap = 6.0;
        let edge_width = (ui.available_width() - gap * 2.0) / 3.0;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            for edge in [1600u32, 2400, 3000] {
                if chip(
                    ui,
                    &edge.to_string(),
                    edge == self.export_long_edge,
                    edge_width,
                    26.0,
                    10.0,
                )
                .clicked()
                {
                    self.export_long_edge = edge;
                }
            }
        });

        ui.add_space(7.0);
        let mut use_seed_name = false;
        ui.horizontal(|ui| {
            field_caption(ui, "Save to");
            if !self.export_name_follows_seed {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ghost_button(ui, "Use seed name").clicked() {
                        use_seed_name = true;
                    }
                });
            }
        });
        if use_seed_name {
            self.export_name_follows_seed = true;
            self.sync_export_name();
        }
        ui.add_space(3.0);
        let mut path_edited = false;
        let mut browse = false;
        ui.horizontal(|ui| {
            let gap = 6.0;
            let browse_width = 74.0;
            let field_width = (ui.available_width() - browse_width - gap).max(80.0);
            ui.allocate_ui_with_layout(
                egui::vec2(field_width, 32.0),
                Layout::left_to_right(Align::Center),
                |ui| {
                    path_edited =
                        text_field(ui, &mut self.export_base, "out/shan-shui-42", 200).changed();
                },
            );
            ui.add_space(gap);
            if action_button(
                ui,
                egui::vec2(browse_width, 32.0),
                "Save as…",
                ButtonKind::Secondary,
                None,
                true,
            )
            .on_hover_text(
                "Choose a folder and file name. PNG/SVG suffixes are added automatically.",
            )
            .clicked()
            {
                browse = true;
            }
        });
        if browse {
            self.choose_export_path();
        } else if path_edited {
            self.export_name_follows_seed = false;
        }
        ui.add_space(8.0);

        let running = matches!(self.export_state, ExportState::Running { .. });
        let label = if running {
            "Exporting…".to_string()
        } else {
            match (self.export_png, self.export_svg) {
                (true, true) => "Export PNG + SVG".to_string(),
                (true, false) => "Export PNG".to_string(),
                (false, true) => "Export SVG".to_string(),
                _ => "Export".to_string(),
            }
        };
        if primary_button(
            ui,
            egui::vec2(ui.available_width(), 38.0),
            &label,
            Some("Ctrl+E"),
            !running,
        )
        .clicked()
        {
            self.start_export(ui.ctx());
        }

        ui.add_space(7.0);
        self.export_status_line(ui);

        if running {
            ui.ctx().request_repaint_after(Duration::from_millis(80));
        }
    }

    fn export_status_line(&mut self, ui: &mut egui::Ui) {
        let mut reveal: Option<PathBuf> = None;
        let mut hover_text: Option<String> = None;

        ui.horizontal(|ui| {
            match &self.export_state {
                ExportState::Idle => {
                    ui.label(
                        RichText::new("Full composition · finish applies to PNG")
                            .font(FontId::proportional(9.0))
                            .color(FAINT),
                    );
                }
                ExportState::Running { stage } => {
                    ui.add(egui::Spinner::new().size(11.0).color(ACCENT_HOVER));
                    ui.label(
                        RichText::new(stage)
                            .font(FontId::proportional(9.5))
                            .color(MUTED),
                    );
                }
                ExportState::Done(report) => {
                    let formats = match (report.png, report.svg) {
                        (true, true) => "PNG + SVG",
                        (true, false) => "PNG",
                        (false, true) => "SVG",
                        _ => "file",
                    };
                    ui.label(
                        RichText::new(format!(
                            "Saved {formats} · {} shapes · {:.1} s",
                            thousands(report.shapes),
                            report.elapsed_ms as f64 / 1000.0
                        ))
                        .font(FontId::proportional(9.5))
                        .color(SAGE),
                    );
                    if let Some(path) = report.paths.first() {
                        reveal = Some(path.clone());
                    }
                    hover_text = Some(
                        report
                            .paths
                            .iter()
                            .map(|path| path.display().to_string())
                            .collect::<Vec<_>>()
                            .join("\n"),
                    );
                }
                ExportState::Failed(error) => {
                    ui.label(
                        RichText::new(format!("Export failed — {error}"))
                            .font(FontId::proportional(9.5))
                            .color(ERROR),
                    );
                    hover_text = Some(error.clone());
                }
            }

            if reveal.is_some() {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ghost_button(ui, "Reveal").clicked() {
                        if let Some(path) = &reveal {
                            reveal_path(path);
                        }
                    }
                });
            }
        });
        if let Some(text) = hover_text {
            ui.interact(
                ui.min_rect(),
                ui.scope_id().with("export-status-hover"),
                Sense::hover(),
            )
            .on_hover_text(text);
        }
    }

    // -- canvas -------------------------------------------------------------

    fn canvas(&mut self, ui: &mut egui::Ui) {
        let rect = ui.max_rect();
        let painter = ui.painter().clone();
        painter.rect_filled(rect, 0.0, BG);
        painter.vline(rect.left(), rect.y_range(), Stroke::new(1.0, BORDER));
        paint_dot_field(&painter, rect);

        let content = rect.shrink2(egui::vec2(30.0, 18.0));
        let toolbar_height = 30.0;
        let toolbar = Rect::from_min_size(content.min, egui::vec2(content.width(), toolbar_height));
        let image_area = Rect::from_min_max(
            egui::pos2(content.left(), toolbar.bottom() + 14.0),
            egui::pos2(
                content.right(),
                (content.bottom() - CAMERA_RAIL_SPACE).max(content.top()),
            ),
        );

        self.canvas_toolbar(ui, &painter, toolbar, self.should_use_panorama());

        let image_rect = fit_rect(image_area, self.aspect.ratio() as f32);

        if image_rect.width() < 80.0 || image_rect.height() < 80.0 {
            return;
        }

        let shadow = Shadow {
            offset: [0, 16],
            blur: 42,
            spread: 0,
            color: Color32::from_rgba_unmultiplied(0, 0, 0, 150),
        };
        painter.add(shadow.as_shape(image_rect, CornerRadius::same(2)));

        let using_panorama = self.should_use_panorama();
        if using_panorama || self.preview_texture.is_some() {
            let paper = get_palette(&self.theme)
                .map(|palette| scene_color(palette.paper))
                .unwrap_or(Color32::from_rgb(247, 244, 236));
            painter.rect_filled(image_rect, CornerRadius::ZERO, paper);
            let clipped = painter.with_clip_rect(image_rect);
            if using_panorama {
                if let Some(panorama) = &self.panorama_texture {
                    clipped.image(
                        panorama.texture.id(),
                        image_rect,
                        self.panorama_uv(),
                        Color32::WHITE,
                    );
                    self.paint_panorama_inscription(&clipped, image_rect);
                }
            } else if let Some(texture) = &self.preview_texture {
                let shift = self.texture_shift_px(image_rect.width());
                let draw_rect = image_rect.translate(egui::vec2(shift, 0.0));
                clipped.image(texture.id(), draw_rect, self.view_uv(), Color32::WHITE);
            }
            painter.rect_stroke(
                image_rect,
                CornerRadius::same(1),
                Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 245, 225, 26)),
                StrokeKind::Inside,
            );
            self.preview_overlays(ui, &painter, image_rect);
        } else {
            self.paint_empty_state(ui, &painter, image_rect);
        }

        let rail = Rect::from_min_size(
            egui::pos2(image_rect.left(), image_rect.bottom() + 11.0),
            egui::vec2(image_rect.width(), 18.0),
        );
        self.camera_rail(ui, &painter, rail);
    }

    fn texture_shift_px(&self, width: f32) -> f32 {
        if self.texture_world_width <= 1e-9 || width <= 1.0 {
            return 0.0;
        }
        let shift = (self.texture_camera - self.camera) / self.texture_world_width
            * width as f64
            * self.zoom.max(1.0) as f64;
        if shift.abs() > width as f64 * 0.35 {
            0.0
        } else {
            shift as f32
        }
    }

    /// The panorama strip deliberately has no inscription baked into it. Draw
    /// the title and seal at their classic viewport position instead, so they
    /// stay still while the landscape scrolls underneath.
    fn paint_panorama_inscription(&self, painter: &egui::Painter, image_rect: Rect) {
        let Ok(palette) = get_palette(&self.theme) else {
            return;
        };
        let (world_width, _) = self.aspect.world_size();
        let zoom = self.zoom.max(1.0) as f64;
        let scale = image_rect.width() as f64 * zoom / world_width;
        if scale <= 0.0 {
            return;
        }

        // The overlay is pinned to the visible viewport, not to the unzoomed
        // world. `scale` already contains the zoom factor, so the classic
        // 46 px / 64 px inset simply scales with the painting.
        let x = image_rect.left() as f64 + 46.0 * scale;
        let y = image_rect.top() as f64 + 64.0 * scale;

        if !self.title.trim().is_empty() && y < image_rect.bottom() as f64 {
            let size = (self.title_size * scale).max(1.0) as f32;
            painter.text(
                egui::pos2(x as f32, y as f32),
                Align2::LEFT_TOP,
                &self.title,
                FontId::proportional(size),
                scene_color(palette.ink_at(0.78, 0.9)),
            );
        }

        if !self.seal.trim().is_empty() {
            let size = (30.0 * scale).max(1.0);
            let seal_y = image_rect.top() as f64 + (64.0 + self.title_size + 22.0) * scale;
            if x < image_rect.right() as f64 && seal_y < image_rect.bottom() as f64 {
                let seal_rect = Rect::from_min_size(
                    egui::pos2(x as f32, seal_y as f32),
                    egui::vec2(size as f32, size as f32),
                );
                painter.rect_filled(
                    seal_rect,
                    CornerRadius::same((size * 0.12).max(1.0) as u8),
                    scene_color(palette.accent_at(0.9, 1.0)),
                );
                painter.text(
                    seal_rect.center(),
                    Align2::CENTER_CENTER,
                    &self.seal,
                    FontId::proportional((size * 0.46).max(1.0) as f32),
                    Color32::from_rgb(250, 245, 235),
                );
            }
        }
    }

    fn camera_rail(&mut self, ui: &mut egui::Ui, painter: &egui::Painter, rect: Rect) {
        if rect.width() < 220.0 {
            return;
        }
        let label_font = FontId::proportional(8.5);
        let value_font = FontId::proportional(9.5);
        let track_left = rect.left() + 56.0;
        let track_right = rect.right() - 56.0;
        let track = Rect::from_min_max(
            egui::pos2(track_left, rect.center().y - 1.5),
            egui::pos2(track_right, rect.center().y + 1.5),
        );
        let hit_area = Rect::from_min_max(
            egui::pos2(track_left, rect.top()),
            egui::pos2(track_right, rect.bottom()),
        );
        let response = ui.interact(
            hit_area,
            ui.scope_id().with("camera-rail"),
            Sense::click_and_drag(),
        );
        let hovered = response.hovered();
        let dragging = response.dragged();

        painter.text(
            egui::pos2(rect.left(), rect.center().y),
            Align2::LEFT_CENTER,
            "CAMERA",
            label_font,
            FAINT,
        );
        painter.rect_filled(
            track,
            CornerRadius::same(2),
            Color32::from_rgb(0x32, 0x2A, 0x21),
        );

        for index in 0..=4 {
            let t = index as f32 / 4.0;
            let x = track.left() + track.width() * t;
            let color = if index == 2 {
                Color32::from_rgb(0x6A, 0x5C, 0x4A)
            } else {
                Color32::from_rgb(0x45, 0x3A, 0x2D)
            };
            painter.vline(
                x,
                egui::Rangef::new(rect.center().y - 4.0, rect.center().y + 4.0),
                Stroke::new(1.0, color),
            );
        }

        let t = camera_rail_fraction(self.camera);
        let knob_x = track.left() + track.width() * t;
        let knob_color = if dragging || hovered {
            ACCENT_HOVER
        } else {
            ACCENT
        };
        painter.rect_filled(
            Rect::from_center_size(egui::pos2(knob_x, rect.center().y), egui::vec2(28.0, 7.0)),
            CornerRadius::same(3),
            knob_color,
        );
        if hovered || dragging {
            painter.rect_stroke(
                Rect::from_center_size(egui::pos2(knob_x, rect.center().y), egui::vec2(28.0, 7.0)),
                CornerRadius::same(3),
                Stroke::new(1.0, CREAM),
                StrokeKind::Inside,
            );
        }

        painter.text(
            egui::pos2(rect.right(), rect.center().y),
            Align2::RIGHT_CENTER,
            format!("{:.0}", self.camera.round()),
            value_font,
            if hovered { TEXT } else { MUTED },
        );

        if response.is_pointer_button_down_on() || response.dragged() || response.clicked() {
            if let Some(pointer) = response.interact_pointer_pos() {
                let next = ((pointer.x - track.left()) / track.width()).clamp(0.0, 1.0);
                self.camera = camera_from_rail_fraction(next);
            }
        }
        if response.double_clicked() {
            self.camera = 0.0;
        }
        if hovered {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
    }

    fn canvas_toolbar(
        &mut self,
        ui: &mut egui::Ui,
        painter: &egui::Painter,
        rect: Rect,
        using_panorama: bool,
    ) {
        let time = ui.input(|input| input.time);
        let busy = self.preview_busy();
        let status_color = if self.preview_error.is_some() {
            ERROR
        } else if busy || using_panorama {
            ACCENT_HOVER
        } else {
            SAGE
        };
        let middle = rect.center().y;
        painter.circle_filled(egui::pos2(rect.left() + 3.0, middle), 2.6, status_color);

        let status_word = if self.preview_error.is_some() {
            "PREVIEW ERROR"
        } else if using_panorama && busy {
            "TRAVELLING"
        } else if busy {
            "RENDERING"
        } else if using_panorama {
            "PANORAMA"
        } else if self
            .preview_info
            .as_ref()
            .map(|info| info.draft)
            .unwrap_or(false)
        {
            "DRAFT"
        } else {
            "LIVE PREVIEW"
        };
        let left_text = format!(
            "{}  ·  {}  ·  CAM {:.0}  ·  {}",
            theme_display(&self.theme),
            self.aspect.label(),
            self.camera.round(),
            status_word
        );
        painter.text(
            egui::pos2(rect.left() + 13.0, middle),
            Align2::LEFT_CENTER,
            left_text,
            FontId::proportional(9.5),
            if busy { ACCENT_HOVER } else { MUTED },
        );

        let button_height = 28.0;
        let y = middle - button_height * 0.5;
        let gap = 6.0;
        let fit_width = 42.0;
        let zoom_label_width = 52.0;
        let step_width = 29.0;
        let total = step_width + gap + zoom_label_width + gap + step_width + gap + fit_width;
        let start_x = rect.right() - total;
        let enabled = self.preview_texture.is_some();

        let minus_rect = Rect::from_min_size(
            egui::pos2(start_x, y),
            egui::vec2(step_width, button_height),
        );
        if tool_button(ui, painter, minus_rect, "minus", "–", enabled).clicked() && enabled {
            self.zoom = (self.zoom / 1.25).max(1.0);
            if self.zoom <= 1.0 {
                self.reset_view();
            }
        }

        let label_rect = Rect::from_min_size(
            egui::pos2(minus_rect.right() + gap, y),
            egui::vec2(zoom_label_width, button_height),
        );
        painter.rect_filled(label_rect, CornerRadius::same(7), PANEL_2);
        painter.rect_stroke(
            label_rect,
            CornerRadius::same(7),
            Stroke::new(1.0, BORDER),
            StrokeKind::Inside,
        );
        painter.text(
            label_rect.center(),
            Align2::CENTER_CENTER,
            format!("{:.0}%", self.zoom * 100.0),
            FontId::proportional(10.5),
            MUTED,
        );

        let plus_rect = Rect::from_min_size(
            egui::pos2(label_rect.right() + gap, y),
            egui::vec2(step_width, button_height),
        );
        if tool_button(ui, painter, plus_rect, "plus", "+", enabled).clicked() && enabled {
            self.zoom = (self.zoom * 1.25).min(4.0);
        }

        let fit_rect = Rect::from_min_size(
            egui::pos2(plus_rect.right() + gap, y),
            egui::vec2(fit_width, button_height),
        );
        if tool_button(ui, painter, fit_rect, "fit", "Fit", enabled).clicked() && enabled {
            self.reset_view();
        }

        if busy {
            paint_mini_spinner(
                painter,
                egui::pos2(rect.right() - total - 14.0, middle),
                4.0,
                ACCENT_HOVER,
                time,
            );
        }
    }

    fn preview_overlays(&mut self, ui: &mut egui::Ui, painter: &egui::Painter, image_rect: Rect) {
        let response = ui.interact(
            image_rect,
            ui.scope_id().with("preview-canvas"),
            Sense::click_and_drag(),
        );

        if response.dragged_by(egui::PointerButton::Primary) {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
            let delta = response.drag_delta();
            let world_width = self.aspect.world_size().0;
            let per_pixel =
                world_width / (image_rect.width().max(1.0) as f64 * self.zoom.max(1.0) as f64);
            self.camera -= delta.x as f64 * per_pixel;
            self.clamp_camera();
        } else if response.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
        }

        if response.hovered() {
            let (scroll_x, scroll_y, shift) = ui.input(|input| {
                (
                    input.smooth_scroll_delta.x,
                    input.smooth_scroll_delta.y,
                    input.modifiers.shift,
                )
            });
            let horizontal = if shift { scroll_y } else { scroll_x };
            let vertical_zoom = !shift && scroll_x.abs() < 0.01;
            if horizontal.abs() > 0.01 {
                let world_width = self.aspect.world_size().0;
                let per_point =
                    world_width / (image_rect.width().max(1.0) as f64 * self.zoom.max(1.0) as f64);
                self.camera += horizontal as f64 * per_point;
                self.clamp_camera();
            }
            if vertical_zoom && scroll_y.abs() > 0.01 {
                let old_zoom = self.zoom as f64;
                let new_zoom = (old_zoom * (scroll_y as f64 * 0.0035).exp()).clamp(1.0, 4.0);
                if (new_zoom - old_zoom).abs() > 1e-6 {
                    self.zoom = new_zoom as f32;
                    if self.zoom <= 1.001 {
                        self.reset_view();
                    }
                }
            }
        }

        if response.double_clicked() {
            self.reset_view();
        }

        if self.preview_busy() {
            painter.rect_filled(
                Rect::from_min_size(image_rect.min, egui::vec2(image_rect.width(), 2.0)),
                CornerRadius::ZERO,
                ACCENT,
            );
        }

        let show_hint = image_rect.width() > 560.0 && image_rect.height() > 260.0;
        if show_hint {
            let hint = "Drag or Shift+scroll to travel  ·  Scroll to zoom  ·  F to fit";
            let font = FontId::proportional(9.5);
            let galley = painter.layout_no_wrap(hint.to_owned(), font, MUTED);
            let pill = Rect::from_min_size(
                egui::pos2(image_rect.left() + 14.0, image_rect.bottom() - 38.0),
                egui::vec2(galley.size().x + 22.0, 25.0),
            );
            paint_translucent_pill(painter, pill);
            painter.galley(
                egui::pos2(pill.left() + 11.0, pill.center().y - galley.size().y * 0.5),
                galley,
                MUTED,
            );
        }

        if image_rect.width() > 640.0 {
            let label = if self.grain {
                format!("{} · GRAIN", self.finish.name().to_uppercase())
            } else {
                self.finish.name().to_uppercase()
            };
            let font = FontId::proportional(8.5);
            let galley = painter.layout_no_wrap(label, font, MUTED);
            let pill = Rect::from_min_size(
                egui::pos2(
                    image_rect.right() - galley.size().x - 36.0,
                    image_rect.bottom() - 38.0,
                ),
                egui::vec2(galley.size().x + 22.0, 25.0),
            );
            paint_translucent_pill(painter, pill);
            painter.galley(
                egui::pos2(pill.left() + 11.0, pill.center().y - galley.size().y * 0.5),
                galley,
                MUTED,
            );
        }
    }

    fn paint_empty_state(&mut self, ui: &mut egui::Ui, painter: &egui::Painter, image_rect: Rect) {
        let palette = get_palette(&self.theme).ok();
        let paper = palette
            .as_ref()
            .map(|palette| scene_color(palette.paper))
            .unwrap_or(Color32::from_rgb(247, 244, 236));
        let ink = palette
            .as_ref()
            .map(|palette| scene_color(palette.ink))
            .unwrap_or(Color32::from_rgb(30, 28, 24));

        painter.rect_filled(image_rect, CornerRadius::same(2), paper);

        let alpha_ink =
            |alpha: u8| Color32::from_rgba_unmultiplied(ink.r(), ink.g(), ink.b(), alpha);

        // A loose suggestion of mountains, so the blank state still feels alive.
        let left = image_rect.left();
        let right = image_rect.right();
        let bottom = image_rect.bottom();
        let width = image_rect.width();
        let height = image_rect.height();
        let horizon = bottom - height * 0.10;
        painter.add(egui::Shape::convex_polygon(
            vec![
                egui::pos2(left + width * 0.04, horizon),
                egui::pos2(left + width * 0.28, horizon - height * 0.26),
                egui::pos2(left + width * 0.50, horizon),
            ],
            alpha_ink(16),
            Stroke::NONE,
        ));
        painter.add(egui::Shape::convex_polygon(
            vec![
                egui::pos2(left + width * 0.32, horizon),
                egui::pos2(left + width * 0.60, horizon - height * 0.18),
                egui::pos2(left + width * 0.86, horizon),
            ],
            alpha_ink(12),
            Stroke::NONE,
        ));
        painter.hline(
            (left + width * 0.05)..=(right - width * 0.05),
            horizon,
            Stroke::new(0.8, alpha_ink(18)),
        );

        painter.text(
            egui::pos2(image_rect.center().x, image_rect.center().y - height * 0.08),
            Align2::CENTER_CENTER,
            "山",
            FontId::proportional((height * 0.17).clamp(54.0, 120.0)),
            alpha_ink(34),
        );

        let time = ui.input(|input| input.time);
        if self.preview_error.is_some() {
            painter.text(
                egui::pos2(image_rect.center().x, image_rect.center().y + height * 0.12),
                Align2::CENTER_CENTER,
                "The preview could not be rendered.",
                FontId::proportional(12.0),
                ERROR,
            );
            if let Some(error) = &self.preview_error {
                painter.text(
                    egui::pos2(
                        image_rect.center().x,
                        image_rect.center().y + height * 0.12 + 22.0,
                    ),
                    Align2::CENTER_CENTER,
                    error,
                    FontId::proportional(9.5),
                    FAINT,
                );
            }
        } else {
            paint_spinner(
                painter,
                egui::pos2(image_rect.center().x, image_rect.center().y + height * 0.12),
                8.0,
                ACCENT,
                time,
            );
            painter.text(
                egui::pos2(
                    image_rect.center().x,
                    image_rect.center().y + height * 0.12 + 22.0,
                ),
                Align2::CENTER_CENTER,
                "GRINDING INK",
                FontId::proportional(9.0),
                FAINT,
            );
        }
    }
}

impl eframe::App for StudioApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.poll_preview(&ctx);
        self.poll_panorama(&ctx);
        self.poll_export();
        self.handle_shortcuts(&ctx);

        egui::Panel::top("studio-topbar")
            .exact_size(TOPBAR_HEIGHT)
            .show_separator_line(false)
            .frame(
                Frame::NONE
                    .fill(PANEL)
                    .inner_margin(Margin::symmetric(22, 0)),
            )
            .show(ui, |ui| {
                self.top_bar(ui);
                let line = Rect::from_min_size(
                    egui::pos2(ui.min_rect().left(), ui.min_rect().bottom() - 1.0),
                    egui::vec2(ui.min_rect().width(), 1.0),
                );
                ui.painter().rect_filled(line, 0, BORDER);
            });

        egui::Panel::left("studio-sidebar")
            .exact_size(SIDEBAR_WIDTH)
            .show_separator_line(false)
            .frame(Frame::NONE.fill(PANEL).inner_margin(Margin {
                left: 20,
                right: 20,
                top: 16,
                bottom: 14,
            }))
            .show(ui, |ui| {
                self.sidebar(ui);
            });

        egui::CentralPanel::no_frame().show(ui, |ui| {
            self.canvas(ui);
        });

        self.schedule_preview(&ctx);
        self.schedule_panorama(&ctx);

        if self.preview_busy()
            || self.panorama_inflight
            || matches!(self.export_state, ExportState::Running { .. })
        {
            ctx.request_repaint_after(Duration::from_millis(80));
        }
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.055, 0.047, 0.038, 1.0]
    }
}

// ---------------------------------------------------------------------------
// Preview worker
// ---------------------------------------------------------------------------

fn preview_worker(rx: Receiver<PreviewJob>, tx: Sender<PreviewMsg>, latest: Arc<AtomicU64>) {
    let mut scene_cache: Option<(SceneKey, Scene)> = None;
    let mut shape_cache: Option<(ShapeKey, Arc<Vec<Shape>>)> = None;

    while let Ok(job) = rx.recv() {
        if latest.load(Ordering::Relaxed) != job.seq {
            let _ = tx.send(PreviewMsg::Cancelled);
            continue;
        }

        let started = Instant::now();
        let scene_key = SceneKey::from_options(&job.scene);

        let reuse_scene = scene_cache
            .as_ref()
            .map(|(key, _)| key == &scene_key)
            .unwrap_or(false);
        if !reuse_scene {
            match Scene::new(job.scene.clone()) {
                Ok(scene) => scene_cache = Some((scene_key.clone(), scene)),
                Err(error) => {
                    let _ = tx.send(PreviewMsg::Error {
                        seq: job.seq,
                        message: error,
                    });
                    continue;
                }
            }
        }

        let Some((_, scene)) = scene_cache.as_mut() else {
            continue;
        };
        // The cached scene must carry the current render-time options (sky,
        // mist, inscription, …) even when its generated chunks are reused.
        scene.options = job.scene.clone();

        if latest.load(Ordering::Relaxed) != job.seq {
            let _ = tx.send(PreviewMsg::Cancelled);
            continue;
        }

        let shape_key = ShapeKey::new(&scene_key, &job.scene, job.world_width, job.world_height);
        let cached_shapes = shape_cache
            .as_ref()
            .filter(|(key, _)| key == &shape_key)
            .map(|(_, shapes)| shapes.clone());

        let xmin = job.scene.camera - job.world_width * 0.5;
        let shapes = if let Some(shapes) = cached_shapes {
            shapes
        } else {
            let birds_anchor = panorama_world_range();
            let generated = Arc::new(scene.render_window(
                xmin,
                xmin + job.world_width,
                job.world_height,
                PANORAMA_SUN_ANCHOR,
                birds_anchor,
            ));
            shape_cache = Some((shape_key, generated.clone()));
            generated
        };

        if latest.load(Ordering::Relaxed) != job.seq {
            let _ = tx.send(PreviewMsg::Cancelled);
            continue;
        }

        let render_options = RenderOptions {
            xmin,
            width: job.output_width,
            height: job.output_height,
            scale: job.output_width as f64 / job.world_width,
            paper_color: Some(scene.palette.paper),
            supersample: job.supersample,
            paper: true,
            grain: job.grain,
            finish: job.finish,
            grain_seed: job.grain_seed,
        };
        let Some(bitmap) = render_shapes_when(&shapes, &render_options, || {
            latest.load(Ordering::Relaxed) == job.seq
        }) else {
            let _ = tx.send(PreviewMsg::Cancelled);
            continue;
        };

        if latest.load(Ordering::Relaxed) != job.seq {
            let _ = tx.send(PreviewMsg::Cancelled);
            continue;
        }

        let _ = tx.send(PreviewMsg::Ready(PreviewOutput {
            seq: job.seq,
            spec: job.spec.clone(),
            bitmap,
            shapes: shapes.len(),
            elapsed_ms: started.elapsed().as_millis() as u64,
            camera: job.scene.camera,
            world_width: job.world_width,
            draft: job.draft,
        }));
    }
}

// ---------------------------------------------------------------------------
// Panorama worker
// ---------------------------------------------------------------------------

fn panorama_worker(rx: Receiver<PanoramaJob>, tx: Sender<PanoramaMsg>, latest: Arc<AtomicU64>) {
    let mut scene_cache: Option<(SceneKey, Scene)> = None;

    while let Ok(job) = rx.recv() {
        if latest.load(Ordering::Relaxed) != job.seq {
            let _ = tx.send(PanoramaMsg::Cancelled);
            continue;
        }

        let started = Instant::now();
        let scene_key = SceneKey::from_options(&job.scene);
        let reuse_scene = scene_cache
            .as_ref()
            .map(|(key, _)| key == &scene_key)
            .unwrap_or(false);
        if !reuse_scene {
            match Scene::new(job.scene.clone()) {
                Ok(scene) => scene_cache = Some((scene_key.clone(), scene)),
                Err(error) => {
                    let _ = tx.send(PanoramaMsg::Error {
                        seq: job.seq,
                        message: error,
                    });
                    continue;
                }
            }
        }

        let Some((_, scene)) = scene_cache.as_mut() else {
            continue;
        };
        scene.options = job.scene.clone();

        if latest.load(Ordering::Relaxed) != job.seq {
            let _ = tx.send(PanoramaMsg::Cancelled);
            continue;
        }

        let shapes = scene.render_window(
            job.xmin,
            job.xmax,
            job.world_height,
            PANORAMA_SUN_ANCHOR,
            (job.xmin, job.xmax),
        );

        if latest.load(Ordering::Relaxed) != job.seq {
            let _ = tx.send(PanoramaMsg::Cancelled);
            continue;
        }

        let render_options = RenderOptions {
            xmin: job.xmin,
            width: job.output_width,
            height: job.output_height,
            scale: job.output_width as f64 / (job.xmax - job.xmin).max(1.0),
            paper_color: Some(scene.palette.paper),
            supersample: 1,
            paper: true,
            grain: job.grain,
            finish: job.finish,
            grain_seed: job.grain_seed,
        };
        let Some(bitmap) = render_shapes_when(&shapes, &render_options, || {
            latest.load(Ordering::Relaxed) == job.seq
        }) else {
            let _ = tx.send(PanoramaMsg::Cancelled);
            continue;
        };

        if latest.load(Ordering::Relaxed) != job.seq {
            let _ = tx.send(PanoramaMsg::Cancelled);
            continue;
        }

        let _ = tx.send(PanoramaMsg::Ready(PanoramaOutput {
            seq: job.seq,
            key: job.key,
            bitmap,
            shapes: shapes.len(),
            elapsed_ms: started.elapsed().as_millis() as u64,
        }));
    }
}

// ---------------------------------------------------------------------------
// Export worker
// ---------------------------------------------------------------------------

fn export_worker(settings: ExportSettings, tx: Sender<ExportMsg>) {
    let started = Instant::now();
    let result = (|| -> Result<(Vec<PathBuf>, usize), String> {
        let _ = tx.send(ExportMsg::Stage("Composing the landscape…".into()));
        let mut scene = Scene::new(settings.spec.scene_options())?;
        let xmin = settings.spec.camera - settings.world_width * 0.5;
        let shapes = scene.render(
            Some(xmin),
            Some(xmin + settings.world_width),
            Some(settings.world_width),
            Some(settings.world_height),
        );

        if settings.png {
            let _ = tx.send(ExportMsg::Stage("Rendering the raster…".into()));
            let options = RenderOptions {
                xmin,
                width: settings.output_width,
                height: settings.output_height,
                scale: settings.output_width as f64 / settings.world_width,
                paper_color: Some(scene.palette.paper),
                supersample: settings.supersample,
                paper: true,
                grain: settings.grain,
                finish: settings.spec.finish,
                grain_seed: settings.grain_seed,
            };
            let bitmap = render_shapes(&shapes, &options);
            let path = settings
                .paths
                .iter()
                .find(|path| path.extension().and_then(|value| value.to_str()) == Some("png"))
                .cloned()
                .ok_or_else(|| "No PNG output path was selected.".to_string())?;
            save_png(&path, &bitmap)?;
        }

        if settings.svg {
            let _ = tx.send(ExportMsg::Stage("Writing the vector scroll…".into()));
            let path = settings
                .paths
                .iter()
                .find(|path| path.extension().and_then(|value| value.to_str()) == Some("svg"))
                .cloned()
                .ok_or_else(|| "No SVG output path was selected.".to_string())?;
            save_svg_sized(
                &path,
                &shapes,
                xmin,
                settings.world_width,
                settings.world_height,
                settings.output_width,
                settings.output_height,
                true,
                Some(scene.palette.paper),
                settings.grain,
            )?;
        }

        Ok((settings.paths, shapes.len()))
    })();

    match result {
        Ok((paths, shapes)) => {
            let _ = tx.send(ExportMsg::Done(ExportReport {
                paths,
                elapsed_ms: started.elapsed().as_millis() as u64,
                shapes,
                png: settings.png,
                svg: settings.svg,
            }));
        }
        Err(error) => {
            let _ = tx.send(ExportMsg::Failed(error));
        }
    }
}

// ---------------------------------------------------------------------------
// Styling and fonts
// ---------------------------------------------------------------------------

fn install_fonts(ctx: &egui::Context) -> FontFamily {
    let display = FontFamily::Name(DISPLAY_FAMILY.into());

    let georgia_families = [(display.clone(), FontPriority::Highest)];
    let mut display_loaded = add_font(ctx, "studio-georgia", &["georgia.ttf"], &georgia_families);

    if !display_loaded {
        let families = [(display.clone(), FontPriority::Highest)];
        display_loaded = add_font(ctx, "studio-display", &["segoeui.ttf"], &families);
    }

    let body_families = [(FontFamily::Proportional, FontPriority::Highest)];
    if !add_font(ctx, "studio-body", &["segoeui.ttf"], &body_families) {
        let families = [(FontFamily::Proportional, FontPriority::Highest)];
        let _ = add_font(ctx, "studio-body-serif", &["georgia.ttf"], &families);
    }

    let fallback_families = [
        (FontFamily::Proportional, FontPriority::Lowest),
        (FontFamily::Monospace, FontPriority::Lowest),
        (display.clone(), FontPriority::Lowest),
    ];
    if !add_font(
        ctx,
        "studio-cjk",
        &["msyh.ttc", "simsun.ttc"],
        &fallback_families,
    ) {
        let families = [(display.clone(), FontPriority::Highest)];
        display_loaded = add_font(
            ctx,
            "studio-display-cjk",
            &["msyh.ttc", "simsun.ttc"],
            &families,
        ) || display_loaded;
    }

    if display_loaded {
        display
    } else {
        FontFamily::Proportional
    }
}

fn add_font(
    ctx: &egui::Context,
    name: &str,
    candidates: &[&str],
    families: &[(FontFamily, FontPriority)],
) -> bool {
    let Some(path) = find_font(candidates) else {
        return false;
    };
    let Ok(bytes) = std::fs::read(path) else {
        return false;
    };
    ctx.add_font(FontInsert::new(
        name,
        FontData::from_owned(bytes),
        families
            .iter()
            .map(|(family, priority)| InsertFontFamily {
                family: family.clone(),
                priority: *priority,
            })
            .collect(),
    ));
    true
}

fn find_font(candidates: &[&str]) -> Option<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(windir) = std::env::var_os("WINDIR") {
        dirs.push(PathBuf::from(windir).join("Fonts"));
    }
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        dirs.push(
            PathBuf::from(local)
                .join("Microsoft")
                .join("Windows")
                .join("Fonts"),
        );
    }
    for candidate in candidates {
        for dir in &dirs {
            let path = dir.join(candidate);
            if path.is_file() {
                return Some(path);
            }
        }
    }
    None
}

fn configure_style(ctx: &egui::Context, display: FontFamily) {
    ctx.set_theme(egui::Theme::Dark);
    let mut style = (*ctx.style_of(egui::Theme::Dark)).clone();

    style
        .text_styles
        .insert(egui::TextStyle::Heading, FontId::new(27.0, display.clone()));
    style
        .text_styles
        .insert(egui::TextStyle::Body, FontId::proportional(13.0));
    style
        .text_styles
        .insert(egui::TextStyle::Button, FontId::proportional(12.5));
    style
        .text_styles
        .insert(egui::TextStyle::Small, FontId::proportional(10.5));
    style
        .text_styles
        .insert(egui::TextStyle::Monospace, FontId::monospace(11.5));

    let mut visuals = egui::Visuals::dark();
    visuals.dark_mode = true;
    visuals.override_text_color = Some(TEXT);
    visuals.panel_fill = PANEL;
    visuals.window_fill = PANEL;
    visuals.extreme_bg_color = FIELD;
    visuals.faint_bg_color = PANEL_2;
    visuals.text_edit_bg_color = Some(FIELD);
    visuals.hyperlink_color = ACCENT_HOVER;
    visuals.selection.bg_fill = ACCENT_SOFT;
    visuals.selection.stroke = Stroke::new(1.0, CREAM);
    visuals.window_corner_radius = CornerRadius::same(12);
    visuals.window_stroke = Stroke::new(1.0, BORDER);
    visuals.slider_trailing_fill = true;

    visuals.widgets.noninteractive.bg_fill = PANEL_2;
    visuals.widgets.noninteractive.weak_bg_fill = PANEL_2;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, BORDER);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, MUTED);
    visuals.widgets.noninteractive.corner_radius = CornerRadius::same(8);

    visuals.widgets.inactive.bg_fill = FIELD;
    visuals.widgets.inactive.weak_bg_fill = FIELD;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, BORDER);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT);
    visuals.widgets.inactive.corner_radius = CornerRadius::same(8);

    visuals.widgets.hovered.bg_fill = FIELD_HOVER;
    visuals.widgets.hovered.weak_bg_fill = FIELD_HOVER;
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, BORDER_HOVER);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, CREAM);
    visuals.widgets.hovered.corner_radius = CornerRadius::same(8);

    visuals.widgets.active.bg_fill = ACCENT_SOFT;
    visuals.widgets.active.weak_bg_fill = ACCENT_SOFT;
    visuals.widgets.active.bg_stroke = Stroke::new(1.0, ACCENT);
    visuals.widgets.active.fg_stroke = Stroke::new(1.0, CREAM);
    visuals.widgets.active.corner_radius = CornerRadius::same(8);

    visuals.widgets.open.bg_fill = FIELD_HOVER;
    visuals.widgets.open.weak_bg_fill = FIELD_HOVER;
    visuals.widgets.open.bg_stroke = Stroke::new(1.0, BORDER_HOVER);
    visuals.widgets.open.fg_stroke = Stroke::new(1.0, CREAM);
    visuals.widgets.open.corner_radius = CornerRadius::same(8);

    style.visuals = visuals;
    style.spacing.item_spacing = egui::vec2(8.0, 8.0);
    style.spacing.button_padding = egui::vec2(10.0, 6.0);
    style.spacing.interact_size = egui::vec2(32.0, 26.0);
    style.spacing.indent = 16.0;
    style.animation_time = 0.16;

    ctx.set_style_of(egui::Theme::Dark, style);
}

// ---------------------------------------------------------------------------
// UI building blocks
// ---------------------------------------------------------------------------

enum ButtonKind {
    Primary,
    Secondary,
}

fn paint_brand(ui: &mut egui::Ui, display: &FontFamily) {
    let (logo_rect, _) = ui.allocate_exact_size(egui::vec2(36.0, 36.0), Sense::hover());
    ui.painter()
        .rect_filled(logo_rect, CornerRadius::same(9), ACCENT);
    ui.painter().rect_stroke(
        logo_rect,
        CornerRadius::same(9),
        Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 228, 214, 45)),
        StrokeKind::Inside,
    );
    ui.painter().text(
        logo_rect.center() + egui::vec2(0.0, 1.0),
        Align2::CENTER_CENTER,
        "山",
        FontId::proportional(20.0),
        CREAM,
    );

    ui.add_space(4.0);
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        ui.label(
            RichText::new("Shan Shui")
                .font(FontId::new(23.0, display.clone()))
                .color(CREAM),
        );
        ui.label(
            RichText::new("STUDIO  ·  PROCEDURAL LANDSCAPES")
                .font(FontId::proportional(8.5))
                .color(Color32::from_rgb(0x8A, 0x7D, 0x69)),
        );
    });
}

fn paint_status_pill(app: &StudioApp, ui: &mut egui::Ui) {
    let (text, color) = app.status_text();
    let font = FontId::proportional(10.5);
    let galley = ui.painter().layout_no_wrap(text, font, MUTED);
    let width = galley.size().x + 38.0;
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, 32.0), Sense::hover());
    ui.painter()
        .rect_filled(rect, CornerRadius::same(16), PANEL_2);
    ui.painter().rect_stroke(
        rect,
        CornerRadius::same(16),
        Stroke::new(1.0, BORDER),
        StrokeKind::Inside,
    );

    let time = ui.input(|input| input.time);
    if app.preview_busy() && app.preview_error.is_none() {
        paint_mini_spinner(
            ui.painter(),
            egui::pos2(rect.left() + 14.0, rect.center().y),
            4.5,
            color,
            time,
        );
    } else {
        ui.painter()
            .circle_filled(egui::pos2(rect.left() + 14.0, rect.center().y), 3.0, color);
    }
    ui.painter().galley(
        egui::pos2(rect.left() + 24.0, rect.center().y - galley.size().y * 0.5),
        galley,
        MUTED,
    );

    let mut tips = Vec::new();
    if let Some(error) = &app.preview_error {
        tips.push(error.clone());
    } else if let Some(info) = &app.preview_info {
        tips.push(format!(
            "{} shapes · rendered in {} ms{}",
            thousands(info.shapes),
            info.elapsed_ms,
            if info.draft { " · draft" } else { " · final" }
        ));
    }
    if let Some(panorama) = &app.panorama_texture {
        tips.push(format!(
            "Panorama · {} shapes · ready in {:.1} s\nScrolling reuses this strip; the view sharpens when the camera rests.",
            thousands(panorama.shapes),
            panorama.elapsed_ms as f64 / 1000.0
        ));
    }
    if !tips.is_empty() {
        let _ = response.on_hover_text(tips.join("\n"));
    }
}

fn section_label(ui: &mut egui::Ui, text: &str) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        let (bar, _) = ui.allocate_exact_size(egui::vec2(2.0, 10.0), Sense::hover());
        ui.painter().rect_filled(bar, CornerRadius::same(1), ACCENT);
        ui.label(
            RichText::new(text.to_uppercase())
                .font(FontId::proportional(9.5))
                .color(FAINT),
        );
    });
}

fn section_label_inline(ui: &mut egui::Ui, text: &str) {
    ui.spacing_mut().item_spacing.x = 6.0;
    let (bar, _) = ui.allocate_exact_size(egui::vec2(2.0, 10.0), Sense::hover());
    ui.painter().rect_filled(bar, CornerRadius::same(1), ACCENT);
    ui.label(
        RichText::new(text.to_uppercase())
            .font(FontId::proportional(9.5))
            .color(FAINT),
    );
}

fn field_caption(ui: &mut egui::Ui, text: &str) {
    ui.label(
        RichText::new(text)
            .font(FontId::proportional(10.0))
            .color(MUTED),
    );
}

fn section_gap(ui: &mut egui::Ui) {
    ui.add_space(13.0);
    ui.separator();
    ui.add_space(13.0);
}

fn chip(
    ui: &mut egui::Ui,
    label: &str,
    selected: bool,
    width: f32,
    height: f32,
    font_size: f32,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, height), Sense::click());
    let hovered = response.hovered();
    let fill = if selected {
        ACCENT
    } else if hovered {
        FIELD_HOVER
    } else {
        FIELD
    };
    let border = if selected {
        Color32::from_rgb(0xD9, 0x7D, 0x64)
    } else if hovered {
        BORDER_HOVER
    } else {
        BORDER
    };
    let text_color = if selected {
        CREAM
    } else if hovered {
        TEXT
    } else {
        MUTED
    };

    ui.painter().rect_filled(rect, CornerRadius::same(7), fill);
    ui.painter().rect_stroke(
        rect,
        CornerRadius::same(7),
        Stroke::new(1.0, border),
        StrokeKind::Inside,
    );
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(font_size),
        text_color,
    );

    if hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response
}

fn toggle_row(ui: &mut egui::Ui, label: &str, value: &mut bool) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(label)
                .font(FontId::proportional(12.0))
                .color(TEXT),
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if switch(ui, value) {
                changed = true;
            }
        });
    });
    ui.add_space(5.0);
    changed
}

fn switch(ui: &mut egui::Ui, value: &mut bool) -> bool {
    let (rect, mut response) = ui.allocate_exact_size(egui::vec2(32.0, 18.0), Sense::click());
    let mut changed = false;
    if response.clicked() {
        *value = !*value;
        response.mark_changed();
        changed = true;
    }

    let t = ui
        .ctx()
        .animate_bool_responsive(response.id.with("switch"), *value);
    let track = if *value {
        ACCENT
    } else if response.hovered() {
        Color32::from_rgb(0x3B, 0x32, 0x27)
    } else {
        Color32::from_rgb(0x33, 0x2B, 0x22)
    };
    ui.painter().rect_filled(rect, CornerRadius::same(9), track);
    let left = rect.left() + 9.0;
    let right = rect.right() - 9.0;
    let knob_x = left + (right - left) * t;
    let knob_color = if *value { CREAM } else { MUTED };
    ui.painter()
        .circle_filled(egui::pos2(knob_x, rect.center().y), 6.2, knob_color);

    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    changed
}

fn slider_row(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut f64,
    min: f64,
    max: f64,
    format: &dyn Fn(f64) -> String,
) -> bool {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(label)
                .font(FontId::proportional(12.0))
                .color(TEXT),
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(
                RichText::new(format(*value))
                    .font(FontId::proportional(10.0))
                    .color(FAINT),
            );
        });
    });
    ui.add_space(4.0);

    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, 16.0), Sense::click_and_drag());
    let track = Rect::from_center_size(rect.center(), egui::vec2(width, 4.0));
    let t = ((*value - min) / (max - min)).clamp(0.0, 1.0) as f32;
    let knob_x = track.left() + track.width() * t;

    let hovered = response.hovered();
    ui.painter().rect_filled(
        track,
        CornerRadius::same(2),
        Color32::from_rgb(0x34, 0x2C, 0x22),
    );
    ui.painter().rect_filled(
        Rect::from_min_max(track.min, egui::pos2(knob_x, track.max.y)),
        CornerRadius::same(2),
        if hovered { ACCENT_HOVER } else { ACCENT },
    );
    ui.painter().circle_filled(
        egui::pos2(knob_x, track.center().y),
        if hovered { 6.0 } else { 5.2 },
        CREAM,
    );

    let mut changed = false;
    if (response.dragged() || response.clicked()) && width > 0.0 {
        if let Some(pointer) = response.interact_pointer_pos() {
            let next = ((pointer.x - track.left()) / track.width()).clamp(0.0, 1.0);
            *value = min + next as f64 * (max - min);
            changed = true;
        }
    }

    if hovered || response.dragged() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    ui.add_space(6.0);
    changed
}

fn text_field(ui: &mut egui::Ui, value: &mut String, hint: &str, limit: usize) -> egui::Response {
    Frame::NONE
        .fill(FIELD)
        .stroke(Stroke::new(1.0, BORDER))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::symmetric(10, 6))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.add(
                TextEdit::singleline(value)
                    .frame(Frame::NONE)
                    .hint_text(hint)
                    .char_limit(limit)
                    .desired_width(f32::INFINITY)
                    .margin(Margin::ZERO)
                    .font(FontId::proportional(12.5)),
            )
        })
        .inner
}

fn action_button(
    ui: &mut egui::Ui,
    size: Vec2,
    label: &str,
    kind: ButtonKind,
    shortcut: Option<&str>,
    enabled: bool,
) -> egui::Response {
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(size, sense);
    let hovered = enabled && response.hovered();
    let pressed = enabled && response.is_pointer_button_down_on();

    let (fill, stroke, text_color) = match kind {
        ButtonKind::Primary => (
            if pressed {
                ACCENT_PRESSED
            } else if hovered {
                ACCENT_HOVER
            } else {
                ACCENT
            },
            Stroke::NONE,
            CREAM,
        ),
        ButtonKind::Secondary => (
            if hovered { FIELD_HOVER } else { FIELD },
            Stroke::new(1.0, if hovered { BORDER_HOVER } else { BORDER }),
            if enabled { TEXT } else { FAINT },
        ),
    };

    ui.painter().rect_filled(rect, CornerRadius::same(8), fill);
    if stroke != Stroke::NONE {
        ui.painter()
            .rect_stroke(rect, CornerRadius::same(8), stroke, StrokeKind::Inside);
    }

    let font = FontId::proportional(11.5);
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_owned(), font, text_color);
    let has_shortcut = shortcut.is_some() && rect.width() > 120.0;
    let text_x = if has_shortcut {
        rect.left() + 14.0
    } else {
        rect.center().x - galley.size().x * 0.5
    };
    ui.painter().galley(
        egui::pos2(text_x, rect.center().y - galley.size().y * 0.5),
        galley,
        text_color,
    );

    if let Some(shortcut) = shortcut {
        if has_shortcut {
            let hint_font = FontId::proportional(8.5);
            let hint_color = Color32::from_rgba_unmultiplied(
                CREAM.r(),
                CREAM.g(),
                CREAM.b(),
                if hovered { 190 } else { 120 },
            );
            ui.painter().text(
                egui::pos2(rect.right() - 12.0, rect.center().y),
                Align2::RIGHT_CENTER,
                shortcut,
                hint_font,
                hint_color,
            );
        }
    }

    if hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response
}

fn primary_button(
    ui: &mut egui::Ui,
    size: Vec2,
    label: &str,
    shortcut: Option<&str>,
    enabled: bool,
) -> egui::Response {
    action_button(ui, size, label, ButtonKind::Primary, shortcut, enabled)
}

fn ghost_button(ui: &mut egui::Ui, label: &str) -> egui::Response {
    let font = FontId::proportional(9.5);
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_owned(), font.clone(), MUTED);
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(galley.size().x + 4.0, galley.size().y + 2.0),
        Sense::click(),
    );
    let color = if response.hovered() { TEXT } else { MUTED };
    ui.painter()
        .text(rect.center(), Align2::CENTER_CENTER, label, font, color);
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response
}

fn tool_button(
    ui: &mut egui::Ui,
    painter: &egui::Painter,
    rect: Rect,
    id: &str,
    label: &str,
    enabled: bool,
) -> egui::Response {
    let response = ui.interact(
        rect,
        ui.scope_id().with(("canvas-tool", id)),
        if enabled {
            Sense::click()
        } else {
            Sense::hover()
        },
    );
    let hovered = enabled && response.hovered();
    painter.rect_filled(
        rect,
        CornerRadius::same(7),
        if hovered { FIELD_HOVER } else { PANEL_2 },
    );
    painter.rect_stroke(
        rect,
        CornerRadius::same(7),
        Stroke::new(1.0, if hovered { BORDER_HOVER } else { BORDER }),
        StrokeKind::Inside,
    );
    painter.text(
        rect.center(),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(11.0),
        if enabled { TEXT } else { FAINT },
    );
    if hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response
}

fn theme_card(ui: &mut egui::Ui, name: &str, selected: bool) -> bool {
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 46.0), Sense::click());
    let hovered = response.hovered();
    let fill = if selected {
        FIELD
    } else if hovered {
        FIELD_HOVER
    } else {
        PANEL_2
    };
    let border = if selected {
        ACCENT
    } else if hovered {
        BORDER_HOVER
    } else {
        BORDER
    };
    ui.painter().rect_filled(rect, CornerRadius::same(8), fill);
    ui.painter().rect_stroke(
        rect,
        CornerRadius::same(8),
        Stroke::new(if selected { 1.4 } else { 1.0 }, border),
        StrokeKind::Inside,
    );

    let tile = Rect::from_min_size(rect.min + egui::vec2(7.0, 6.0), egui::vec2(34.0, 34.0));
    if let Some(swatch) = theme_swatch(name) {
        paint_theme_tile(ui.painter(), tile, &swatch);
    } else {
        ui.painter().rect_filled(tile, CornerRadius::same(6), FIELD);
    }

    let color = if selected {
        CREAM
    } else if hovered {
        TEXT
    } else {
        MUTED
    };
    ui.painter().text(
        egui::pos2(tile.right() + 8.0, rect.center().y),
        Align2::LEFT_CENTER,
        theme_display(name),
        FontId::proportional(11.0),
        color,
    );

    if selected {
        ui.painter().circle_filled(
            egui::pos2(rect.right() - 10.0, rect.top() + 10.0),
            2.5,
            ACCENT_HOVER,
        );
    }

    if hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response.clicked()
}

fn paint_theme_tile(painter: &egui::Painter, rect: Rect, swatch: &[SceneColor; 4]) {
    let paper = scene_color(swatch[1]);
    let sky = scene_color(swatch[0]);
    let foliage = scene_color(swatch[2]);
    let accent = scene_color(swatch[3]);

    painter.rect_filled(rect, CornerRadius::same(6), paper);
    let inset = rect.shrink(2.5);
    let sky_rect = Rect::from_min_size(inset.min, egui::vec2(inset.width(), inset.height() * 0.54));
    painter.rect_filled(sky_rect, CornerRadius::same(3), sky);

    let horizon = inset.bottom() - inset.height() * 0.13;
    painter.add(egui::Shape::convex_polygon(
        vec![
            egui::pos2(inset.left(), horizon),
            egui::pos2(
                inset.left() + inset.width() * 0.30,
                horizon - inset.height() * 0.42,
            ),
            egui::pos2(inset.left() + inset.width() * 0.56, horizon),
        ],
        foliage.gamma_multiply(0.72),
        Stroke::NONE,
    ));
    painter.add(egui::Shape::convex_polygon(
        vec![
            egui::pos2(inset.left() + inset.width() * 0.36, horizon),
            egui::pos2(
                inset.left() + inset.width() * 0.68,
                horizon - inset.height() * 0.28,
            ),
            egui::pos2(inset.right(), horizon),
        ],
        foliage.gamma_multiply(0.56),
        Stroke::NONE,
    ));
    painter.hline(
        inset.left()..=inset.right(),
        horizon,
        Stroke::new(0.7, foliage.gamma_multiply(0.45)),
    );
    painter.circle_filled(
        egui::pos2(inset.right() - 5.0, inset.top() + 5.0),
        2.6,
        accent,
    );
}

fn paint_translucent_pill(painter: &egui::Painter, rect: Rect) {
    painter.rect_filled(
        rect,
        CornerRadius::same(13),
        Color32::from_rgba_unmultiplied(11, 9, 7, 190),
    );
    painter.rect_stroke(
        rect,
        CornerRadius::same(13),
        Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 244, 226, 18)),
        StrokeKind::Inside,
    );
}

// ---------------------------------------------------------------------------
// Painting helpers
// ---------------------------------------------------------------------------

fn paint_dot_field(painter: &egui::Painter, rect: Rect) {
    let spacing = 32.0;
    let color = Color32::from_rgba_unmultiplied(255, 240, 220, 7);
    let mut y = rect.top() + 16.0;
    while y < rect.bottom() {
        let mut x = rect.left() + 16.0;
        while x < rect.right() {
            painter.circle_filled(egui::pos2(x, y), 0.75, color);
            x += spacing;
        }
        y += spacing;
    }
}

fn paint_mini_spinner(
    painter: &egui::Painter,
    center: Pos2,
    radius: f32,
    color: Color32,
    time: f64,
) {
    paint_spinner(painter, center, radius, color, time);
}

fn paint_spinner(painter: &egui::Painter, center: Pos2, radius: f32, color: Color32, time: f64) {
    let phase = (time * 2.6) as f32;
    let count = 8;
    for index in 0..count {
        let angle = phase + index as f32 * std::f32::consts::TAU / count as f32;
        let strength = index as f32 / (count - 1) as f32;
        let alpha = (35.0 + strength * 220.0) as u8;
        let dot = Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha);
        painter.circle_filled(
            center + egui::vec2(angle.cos(), angle.sin()) * radius,
            if index == count - 1 { 2.0 } else { 1.5 },
            dot,
        );
    }
}

fn fit_rect(area: Rect, aspect: f32) -> Rect {
    let aspect = aspect.max(0.01);
    let area_aspect = area.width() / area.height().max(1.0);
    if area_aspect > aspect {
        let height = area.height();
        let width = height * aspect;
        Rect::from_center_size(area.center(), egui::vec2(width, height))
    } else {
        let width = area.width();
        let height = width / aspect;
        Rect::from_center_size(area.center(), egui::vec2(width, height))
    }
}

// ---------------------------------------------------------------------------
// Utility
// ---------------------------------------------------------------------------

fn scene_color(color: SceneColor) -> Color32 {
    Color32::from_rgba_unmultiplied(
        color.r,
        color.g,
        color.b,
        (color.a.clamp(0.0, 1.0) * 255.0).round() as u8,
    )
}

fn percent(value: f64) -> String {
    format!("{:.0}%", value * 100.0)
}

fn thousands(value: usize) -> String {
    let digits = value.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, character) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index) % 3 == 0 {
            out.push(',');
        }
        out.push(character);
    }
    out
}

fn grain_seed(seed: &Seed) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in seed.raw().as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

/// Path logic shared with the command line tool: a `.png` or `.svg` suffix is
/// replaced for multi-format exports, and kept when it matches the only format.
fn output_paths(base: &str, png: bool, svg: bool) -> Vec<PathBuf> {
    let trimmed = base.trim();
    let base = if trimmed.is_empty() {
        "out/shan-shui-42"
    } else {
        trimmed
    };
    let mut path = PathBuf::from(base);
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase());

    let count = png as u8 + svg as u8;
    if count == 1 {
        let wanted = if png { "png" } else { "svg" };
        if extension.as_deref() == Some(wanted) {
            return vec![path];
        }
    }
    if matches!(extension.as_deref(), Some("png") | Some("svg")) {
        path.set_extension("");
    }

    let mut paths = Vec::new();
    if png {
        let mut png_path = path.clone();
        png_path.set_extension("png");
        paths.push(png_path);
    }
    if svg {
        let mut svg_path = path;
        svg_path.set_extension("svg");
        paths.push(svg_path);
    }
    paths
}

/// Turn a seed into a short, readable file-name fragment. Numeric seeds stay
/// exactly as typed, so `shan-shui-845789` can be regenerated at a glance.
/// Text seeds keep letters and digits; runs of other characters become single
/// dashes.
fn seed_slug(seed: &Seed) -> String {
    let mut slug = String::new();
    let mut pending_dash = false;
    let mut count = 0usize;
    for character in seed.raw().chars() {
        if character.is_alphanumeric() || character == '-' || character == '_' {
            if pending_dash && !slug.is_empty() {
                slug.push('-');
                count += 1;
            }
            pending_dash = false;
            slug.push(character);
            count += 1;
        } else {
            pending_dash = !slug.is_empty();
        }
        if count >= 48 {
            break;
        }
    }
    while slug.ends_with('-') {
        slug.pop();
    }
    if slug.is_empty() {
        "seed".into()
    } else {
        slug
    }
}

fn default_export_base(seed: &Seed) -> String {
    format!("out/shan-shui-{}", seed_slug(seed))
}

fn export_base_from_dialog(path: PathBuf) -> String {
    let mut path = path;
    if matches!(
        path.extension()
            .and_then(|value| value.to_str())
            .map(|value| value.to_ascii_lowercase())
            .as_deref(),
        Some("png") | Some("svg")
    ) {
        path.set_extension("");
    }
    path.to_string_lossy().into_owned()
}

/// Native Windows Save dialog. The caller edits a base path, so `.png`/`.svg`
/// suffixes are stripped before the response reaches the studio.
#[cfg(target_os = "windows")]
fn pick_export_path(
    base: &str,
    default_extension: &str,
    both_formats: bool,
) -> Result<Option<PathBuf>, String> {
    use std::os::windows::ffi::OsStrExt;

    use windows_sys::Win32::UI::Controls::Dialogs::{
        CommDlgExtendedError, GetSaveFileNameW, OFN_NOCHANGEDIR, OFN_OVERWRITEPROMPT,
        OFN_PATHMUSTEXIST, OPENFILENAMEW,
    };
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::GetActiveWindow;

    fn wide(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(std::iter::once(0)).collect()
    }

    let mut base_path = PathBuf::from(base.trim());
    let known_extension = base_path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase());
    if matches!(known_extension.as_deref(), Some("png") | Some("svg")) {
        base_path.set_extension("");
    }
    let mut file_name = base_path
        .file_name()
        .map(|value| value.to_string_lossy().into_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "shan-shui-42".into());
    if known_extension.is_none() {
        file_name.push('.');
        file_name.push_str(default_extension);
    }
    if known_extension.as_deref() == Some("png") {
        file_name.push_str(".png");
    } else if known_extension.as_deref() == Some("svg") {
        file_name.push_str(".svg");
    }

    let mut buffer = vec![0u16; 4096];
    let name: Vec<u16> = file_name.encode_utf16().collect();
    if name.len() + 1 > buffer.len() {
        return Err("The file name is too long for the save dialog.".into());
    }
    buffer[..name.len()].copy_from_slice(&name);

    let cwd = std::env::current_dir().unwrap_or_default();
    let directory = base_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .map(|parent| {
            if parent.is_absolute() {
                parent.to_path_buf()
            } else {
                cwd.join(parent)
            }
        })
        .filter(|path| path.is_dir())
        .unwrap_or(cwd);
    let mut directory_wide: Vec<u16> = directory.as_os_str().encode_wide().collect();
    directory_wide.push(0);

    let filter: Vec<u16> =
        "Painting (PNG + SVG)\0*.png;*.svg\0PNG image\0*.png\0SVG vector\0*.svg\0\0"
            .encode_utf16()
            .collect();
    let title = wide("Save Shan Shui painting");
    let extension = wide(default_extension);
    let filter_index = if both_formats {
        1u32
    } else if default_extension == "svg" {
        3u32
    } else {
        2u32
    };

    let mut dialog = OPENFILENAMEW::default();
    dialog.lStructSize = std::mem::size_of::<OPENFILENAMEW>() as u32;
    dialog.hwndOwner = unsafe { GetActiveWindow() };
    dialog.lpstrFilter = filter.as_ptr();
    dialog.nFilterIndex = filter_index;
    dialog.lpstrFile = buffer.as_mut_ptr();
    dialog.nMaxFile = buffer.len() as u32;
    dialog.lpstrInitialDir = directory_wide.as_ptr();
    dialog.lpstrTitle = title.as_ptr();
    dialog.lpstrDefExt = extension.as_ptr();
    dialog.Flags = OFN_OVERWRITEPROMPT | OFN_PATHMUSTEXIST | OFN_NOCHANGEDIR;

    let accepted = unsafe { GetSaveFileNameW(&mut dialog) };
    if accepted == 0 {
        let error = unsafe { CommDlgExtendedError() };
        if error == 0 {
            return Ok(None);
        }
        return Err(format!("Windows dialog error {error}"));
    }

    let length = buffer
        .iter()
        .position(|character| *character == 0)
        .unwrap_or(buffer.len());
    Ok(Some(PathBuf::from(String::from_utf16_lossy(
        &buffer[..length],
    ))))
}

#[cfg(not(target_os = "windows"))]
fn pick_export_path(
    _base: &str,
    _default_extension: &str,
    _both_formats: bool,
) -> Result<Option<PathBuf>, String> {
    Err("The native save dialog is only available on Windows; edit the path directly.".into())
}

fn reveal_path(path: &Path) {
    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("explorer")
            .arg(format!("/select,{}", path.display()))
            .spawn();
    }
    #[cfg(not(target_os = "windows"))]
    {
        let target = path.parent().unwrap_or(path);
        let _ = std::process::Command::new("xdg-open").arg(target).spawn();
    }
}

// ---------------------------------------------------------------------------
// Window icon
// ---------------------------------------------------------------------------

fn make_icon() -> egui::IconData {
    const SIZE: u32 = 64;
    let mut rgba = vec![0u8; (SIZE * SIZE * 4) as usize];

    for y in 0..SIZE {
        for x in 0..SIZE {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            let index = ((y * SIZE + x) * 4) as usize;

            if !inside_rounded_square(px, py, 0.0, 0.0, 64.0, 14.0) {
                continue;
            }

            let mut color = (196, 80, 58, 255);

            let far_mountain =
                point_in_triangle((px, py), (11.0, 49.0), (30.0, 23.0), (43.0, 49.0));
            let near_mountain =
                point_in_triangle((px, py), (28.0, 49.0), (43.0, 31.0), (57.0, 49.0));
            if far_mountain {
                color = (243, 232, 212, 255);
            }
            if near_mountain {
                color = (250, 242, 227, 255);
            }
            let sun = {
                let dx = px - 48.0;
                let dy = py - 15.0;
                dx * dx + dy * dy <= 16.0
            };
            if sun {
                color = (243, 205, 150, 255);
            }
            if py > 49.0 && py < 52.0 {
                color = (243, 232, 212, 230);
            }

            rgba[index] = color.0;
            rgba[index + 1] = color.1;
            rgba[index + 2] = color.2;
            rgba[index + 3] = color.3;
        }
    }

    egui::IconData {
        rgba,
        width: SIZE,
        height: SIZE,
    }
}

fn inside_rounded_square(px: f32, py: f32, left: f32, top: f32, size: f32, radius: f32) -> bool {
    let right = left + size;
    let bottom = top + size;
    if px < left || py < top || px > right || py > bottom {
        return false;
    }
    let inner_left = left + radius;
    let inner_right = right - radius;
    let inner_top = top + radius;
    let inner_bottom = bottom - radius;
    let cx = px.clamp(inner_left, inner_right);
    let cy = py.clamp(inner_top, inner_bottom);
    let dx = px - cx;
    let dy = py - cy;
    dx * dx + dy * dy <= radius * radius
}

fn point_in_triangle(point: (f32, f32), a: (f32, f32), b: (f32, f32), c: (f32, f32)) -> bool {
    let sign = |p1: (f32, f32), p2: (f32, f32), p3: (f32, f32)| {
        (p1.0 - p3.0) * (p2.1 - p3.1) - (p2.0 - p3.0) * (p1.1 - p3.1)
    };
    let d1 = sign(point, a, b);
    let d2 = sign(point, b, c);
    let d3 = sign(point, c, a);
    let has_negative = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
    let has_positive = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
    !(has_negative && has_positive)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_sizes_match_their_aspects() {
        for aspect in Aspect::ALL {
            let (width, height) = aspect.preview_size();
            let ratio = width as f64 / height as f64;
            assert!(
                (ratio - aspect.ratio()).abs() < 0.002,
                "{} preview is {width}x{height}",
                aspect.label()
            );
            assert!(width.max(height) <= 1200);
        }
    }

    #[test]
    fn export_sizes_keep_the_requested_long_edge() {
        for aspect in Aspect::ALL {
            for edge in [1600u32, 2400, 3000] {
                let (width, height) = aspect.export_size(edge);
                assert_eq!(width.max(height), edge, "{} at {edge}", aspect.label());
                let ratio = width as f64 / height as f64;
                assert!(
                    (ratio - aspect.ratio()).abs() < 0.002,
                    "{} exported at {width}x{height}",
                    aspect.label()
                );
                assert!(width > 0 && height > 0);
            }
        }
    }

    #[test]
    fn export_paths_match_the_cli_rules() {
        let paths = |base: &str, png: bool, svg: bool| {
            output_paths(base, png, svg)
                .into_iter()
                .map(|path| path.to_string_lossy().replace('\\', "/"))
                .collect::<Vec<_>>()
        };

        assert_eq!(
            paths("out/landscape", true, true),
            ["out/landscape.png", "out/landscape.svg"]
        );
        assert_eq!(
            paths("out/landscape.png", true, true),
            ["out/landscape.png", "out/landscape.svg"]
        );
        assert_eq!(
            paths("out/landscape.SVG", true, true),
            ["out/landscape.png", "out/landscape.svg"]
        );
        assert_eq!(
            paths("out/landscape.png", true, false),
            ["out/landscape.png"]
        );
        assert_eq!(
            paths("out/landscape.svg", false, true),
            ["out/landscape.svg"]
        );
        assert_eq!(
            paths("out/landscape.txt", true, true),
            ["out/landscape.png", "out/landscape.svg"]
        );
        assert_eq!(paths("", false, true), ["out/shan-shui-42.svg"]);
        assert!(paths("out/landscape", false, false).is_empty());
    }

    #[test]
    fn seed_names_are_readable_and_safe() {
        assert_eq!(seed_slug(&Seed::Int(845789)), "845789");
        assert_eq!(
            default_export_base(&Seed::Int(845789)),
            "out/shan-shui-845789"
        );
        assert_eq!(
            seed_slug(&Seed::Text("A quiet valley".into())),
            "A-quiet-valley"
        );
        assert_eq!(seed_slug(&Seed::Text("山海 / 42".into())), "山海-42");
        assert_eq!(seed_slug(&Seed::Text("  ...  ".into())), "seed");
        assert_eq!(seed_slug(&Seed::Text("a//b".into())), "a-b");
        assert!(!seed_slug(&Seed::Text("x".repeat(200))).contains('/'));
        assert!(seed_slug(&Seed::Text("x".repeat(200))).chars().count() <= 48);
    }

    #[test]
    fn panorama_window_covers_every_camera_and_zoom() {
        let (xmin, xmax) = panorama_world_range();
        let span = xmax - xmin;
        for aspect in Aspect::ALL {
            let (world_width, _) = aspect.world_size();
            for camera in [CAMERA_MIN, -2200.0, 0.0, 2200.0, CAMERA_MAX] {
                for zoom in [1.0f32, 1.25, 2.0, 4.0] {
                    let uv = panorama_uv_for(camera, zoom, world_width);
                    let tolerance = 1e-5;
                    assert!(uv.min.x >= -tolerance, "{} {:?}", aspect.label(), uv);
                    assert!(uv.max.x <= 1.0 + tolerance, "{} {:?}", aspect.label(), uv);
                    assert!(uv.min.y >= -tolerance);
                    assert!(uv.max.y <= 1.0 + tolerance);
                    let expected_half = world_width / zoom as f64 / span * 0.5;
                    assert!(
                        (uv.width() as f64 - expected_half * 2.0).abs() < 1e-5,
                        "{} at {camera} / {zoom}",
                        aspect.label()
                    );
                    assert!(
                        (uv.height() as f64 - 1.0 / zoom as f64).abs() < 1e-5,
                        "{} vertical crop",
                        aspect.label()
                    );
                }
            }
        }
        let center = panorama_uv_for(0.0, 1.0, 1200.0);
        assert!((center.center().x - 0.5).abs() < 1e-6);
        assert!((center.center().y - 0.5).abs() < 1e-6);
    }

    #[test]
    fn camera_is_bounded_and_round_trips_through_the_rail() {
        assert_eq!(clamp_camera_value(-9999.0), CAMERA_MIN);
        assert_eq!(clamp_camera_value(9999.0), CAMERA_MAX);
        assert_eq!(clamp_camera_value(805.0), 805.0);

        for camera in [CAMERA_MIN, -2400.0, -17.5, 0.0, 805.0, 3999.0, CAMERA_MAX] {
            let round_trip = camera_from_rail_fraction(camera_rail_fraction(camera));
            assert!(
                (round_trip - camera).abs() < 0.5,
                "{camera} became {round_trip}"
            );
        }
        assert_eq!(camera_from_rail_fraction(-1.0), CAMERA_MIN);
        assert_eq!(camera_from_rail_fraction(2.0), CAMERA_MAX);
        assert!((camera_rail_fraction(0.0) - 0.5).abs() < 1e-6);
    }
}
