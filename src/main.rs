mod flasher;
mod progress_data;
mod progress_view;
use eframe::egui::{self, pos2, vec2, Align2, Color32, FontFamily, FontId, Rect, Sense, Stroke};
use flasher::{Cartridge, Catalog, Device, Update};
use progress_data::Dashboard;
use progress_view::ProgressView;
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::mpsc,
    time::{Duration, Instant},
};

const BG: Color32 = Color32::from_rgb(16, 14, 26);
const PANEL: Color32 = Color32::from_rgb(28, 24, 43);
const LINE: Color32 = Color32::from_rgb(57, 49, 78);
const TEXT: Color32 = Color32::from_rgb(241, 234, 255);
const MUTED: Color32 = Color32::from_rgb(156, 145, 177);
const GREEN: Color32 = Color32::from_rgb(185, 242, 132);
const GOLD: Color32 = Color32::from_rgb(255, 213, 121);
const PINK: Color32 = Color32::from_rgb(255, 149, 172);
fn mono(n: f32) -> FontId {
    FontId::monospace(n)
}
fn pixel(n: f32) -> FontId {
    FontId::new(n, FontFamily::Name("pixel".into()))
}
fn label(p: &egui::Painter, at: egui::Pos2, s: &str, n: f32, c: Color32) {
    p.text(at, Align2::LEFT_TOP, s, mono(n), c);
}
fn status_color(c: &Cartridge) -> Color32 {
    if c.ready() {
        GREEN
    } else if c.status == "download_failed" {
        PINK
    } else {
        GOLD
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Screen {
    Collection,
    Progress,
}
#[derive(Clone, Copy, PartialEq, Debug)]
enum View {
    Grid,
    List,
}
#[derive(Clone, Copy, PartialEq, Debug)]
enum Filter {
    All,
    Ready,
    NeedsWork,
    Top200,
}
fn visible(catalog: &Catalog, search: &str, filter: Filter, rank_sort: bool) -> Vec<usize> {
    let q = search.trim().to_lowercase();
    let mut found: Vec<_> = catalog
        .cartridges
        .iter()
        .enumerate()
        .filter(|(_, c)| {
            let matching = q.is_empty()
                || format!("{} {} {}", c.title, c.credits, c.subtitle)
                    .to_lowercase()
                    .contains(&q);
            matching
                && match filter {
                    Filter::All => true,
                    Filter::Ready => c.ready(),
                    Filter::NeedsWork => !c.ready(),
                    Filter::Top200 => c.rank.is_some(),
                }
        })
        .map(|(i, _)| i)
        .collect();
    found.sort_by_key(|i| {
        if rank_sort {
            format!(
                "{:04} {}",
                catalog.cartridges[*i].rank.unwrap_or(0),
                catalog.cartridges[*i].title.to_lowercase()
            )
        } else {
            catalog.cartridges[*i].title.to_lowercase()
        }
    });
    found
}

struct Arcade {
    root: PathBuf,
    catalog: Catalog,
    selected: usize,
    covers: HashMap<String, egui::TextureHandle>,
    device: Device,
    last_detect: Instant,
    receiver: Option<mpsc::Receiver<Update>>,
    progress: f32,
    message: String,
    failed: bool,
    completed: bool,
    show_error: bool,
    import_open: bool,
    import_path: String,
    import_title: String,
    converting: bool,
    reload_needed: bool,
    import_id: Option<String>,
    view: View,
    filter: Filter,
    search: String,
    rank_sort: bool,
    scroll_selected: bool,
    columns: usize,
    hits: Vec<(usize, Rect)>,
    grid_button: Rect,
    list_button: Rect,
    add_button: Rect,
    screen: Screen,
    dashboard: Result<Dashboard, String>,
    progress_view: ProgressView,
    progress_button: Rect,
    collection_button: Rect,
}
impl Arcade {
    fn new(ctx: &egui::Context, root: PathBuf, catalog: Catalog) -> Self {
        let mut fonts = egui::FontDefinitions::default();
        fonts.font_data.insert(
            "pixel".into(),
            egui::FontData::from_static(include_bytes!("../assets/PressStart2P-Regular.ttf"))
                .into(),
        );
        fonts
            .families
            .insert(FontFamily::Name("pixel".into()), vec!["pixel".into()]);
        ctx.set_fonts(fonts);
        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = BG;
        visuals.window_fill = PANEL;
        visuals.override_text_color = Some(TEXT);
        visuals.selection.bg_fill = Color32::from_rgb(61, 76, 57);
        ctx.set_visuals(visuals);
        let mut style = (*ctx.style()).clone();
        style.spacing.item_spacing = vec2(10., 9.);
        style.spacing.button_padding = vec2(12., 8.);
        ctx.set_style(style);
        let mut covers = HashMap::new();
        for (name, data) in [
            ("nomad", include_bytes!("../assets/nomad.png").as_slice()),
            ("prism", include_bytes!("../assets/prism.png").as_slice()),
        ] {
            Self::load_texture(ctx, &mut covers, name, data);
        }
        let grid = std::fs::read_to_string(root.join("view.json"))
            .map(|s| !s.contains("list"))
            .unwrap_or(true);
        let dashboard = Dashboard::load(&root);
        let mut app = Self {
            root,
            catalog,
            selected: 0,
            covers,
            device: flasher::detect(),
            last_detect: Instant::now(),
            receiver: None,
            progress: 0.,
            message: "Choose a cartridge. Ready games can be flashed to your Pico.".into(),
            failed: false,
            completed: false,
            show_error: false,
            import_open: false,
            import_path: String::new(),
            import_title: String::new(),
            converting: false,
            reload_needed: false,
            import_id: None,
            view: if grid { View::Grid } else { View::List },
            filter: Filter::All,
            search: String::new(),
            rank_sort: false,
            scroll_selected: false,
            columns: 4,
            hits: vec![],
            grid_button: Rect::NOTHING,
            list_button: Rect::NOTHING,
            add_button: Rect::NOTHING,
            screen: Screen::Collection,
            dashboard,
            progress_view: ProgressView::default(),
            progress_button: Rect::NOTHING,
            collection_button: Rect::NOTHING,
        };
        app.load_art(ctx);
        app
    }
    fn load_texture(
        ctx: &egui::Context,
        covers: &mut HashMap<String, egui::TextureHandle>,
        key: &str,
        data: &[u8],
    ) {
        if let Ok(img) = image::load_from_memory(data) {
            let img = img.to_rgba8();
            let size = [img.width() as usize, img.height() as usize];
            covers.insert(
                key.into(),
                ctx.load_texture(
                    key,
                    egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw()),
                    egui::TextureOptions::NEAREST,
                ),
            );
        }
    }
    fn load_art(&mut self, ctx: &egui::Context) {
        for c in &self.catalog.cartridges {
            for key in [&c.thumbnail, &c.cart_image, &c.cover] {
                if key.is_empty() || self.covers.contains_key(key) {
                    continue;
                }
                if let Ok(b) = std::fs::read(self.root.join(key)) {
                    Self::load_texture(ctx, &mut self.covers, key, &b);
                }
            }
        }
    }
    fn art_key<'a>(&'a self, c: &'a Cartridge) -> Option<&'a str> {
        [&c.thumbnail, &c.cart_image, &c.cover]
            .into_iter()
            .find(|k| !k.is_empty() && self.covers.contains_key(*k))
            .map(String::as_str)
    }
    fn draw_art(&self, ui: &egui::Ui, bounds: Rect, c: &Cartridge) {
        let p = ui.painter();
        p.rect_filled(bounds, 2., Color32::from_rgb(9, 9, 16));
        if let Some(key) = self.art_key(c) {
            let tex = &self.covers[key];
            let uv_end = if key == "nomad" || key == "prism" {
                0.25
            } else {
                1.
            };
            let size = tex.size_vec2() * vec2(uv_end, 1.);
            let scale = (bounds.width() / size.x).min(bounds.height() / size.y);
            let r = Rect::from_center_size(bounds.center(), size * scale);
            p.image(
                tex.id(),
                r,
                Rect::from_min_max(pos2(0., 0.), pos2(uv_end, 1.)),
                Color32::WHITE,
            );
        } else {
            let middle = bounds.center();
            let s = bounds.width().min(bounds.height()) / 5.;
            p.rect_filled(
                Rect::from_center_size(middle, vec2(s * 2.8, s * 2.8)),
                4.,
                LINE,
            );
            p.rect_filled(
                Rect::from_center_size(middle, vec2(s * 0.4, s * 1.5)),
                1.,
                MUTED,
            );
            p.rect_filled(
                Rect::from_center_size(middle, vec2(s * 1.5, s * 0.4)),
                1.,
                MUTED,
            );
            p.circle_filled(middle + vec2(s * 1.8, -s * 0.5), s * 0.23, PINK);
            p.circle_filled(middle + vec2(s * 1.8, s * 0.5), s * 0.23, GREEN);
            p.text(
                pos2(middle.x, bounds.bottom() - 19.),
                Align2::CENTER_CENTER,
                if c.id == "buttons" {
                    "9 INPUTS"
                } else {
                    "NO IMAGE"
                },
                mono(11.),
                MUTED,
            );
        }
    }
    fn set_view(&mut self, view: View) {
        self.view = view;
        self.scroll_selected = true;
        if !cfg!(test) {
            let _ = std::fs::write(
                self.root.join("view.json"),
                if view == View::Grid {
                    "\"grid\"\n"
                } else {
                    "\"list\"\n"
                },
            );
        }
    }
    fn start(&mut self, ctx: &egui::Context, cart: Cartridge) {
        if self.receiver.is_some() || !cart.ready() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        self.receiver = Some(rx);
        self.converting = false;
        self.failed = false;
        self.completed = false;
        self.progress = 0.;
        self.message = format!("Loading {}...", cart.title);
        let root = self.root.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let result = flasher::flash(&root, &cart, &tx);
            let _ = tx.send(Update::Done(result));
            ctx.request_repaint();
        });
    }
    fn start_conversion(&mut self, ctx: &egui::Context) {
        if self.receiver.is_some() || self.import_path.trim().is_empty() {
            return;
        }
        let source = self.import_path.trim().to_string();
        let title = self.import_title.trim().to_string();
        let root = self.root.clone();
        let ctx = ctx.clone();
        let (tx, rx) = mpsc::channel();
        self.receiver = Some(rx);
        self.converting = true;
        self.failed = false;
        self.completed = false;
        self.progress = 0.2;
        self.message = "Decoding, checking and compiling the cartridge on this computer...".into();
        self.import_open = false;
        std::thread::spawn(move || {
            // Build for the Pico (RP2040) and the Pico 2 / Pico 2 W (RP2350); the
            // flasher picks the image matching the BOOTSEL drive. One success is enough.
            let mut built = Vec::new();
            let mut errors = Vec::new();
            // The Pico 2 build goes first, so a Pico build that does not fit can
            // record why on the cartridge's shelf entry.
            for (board, name) in [("pico2_w", "Pico 2"), ("pico", "Pico")] {
                let _ = tx.send(Update::Stage(
                    if built.is_empty() && errors.is_empty() {
                        0.2
                    } else {
                        0.6
                    },
                    format!("Converting and compiling for the {name}..."),
                ));
                let mut cmd = std::process::Command::new("python3");
                cmd.arg(root.join("converter/build.py"))
                    .arg(&source)
                    .arg("--library")
                    .arg("--board")
                    .arg(board);
                if !title.is_empty() {
                    cmd.arg("--title").arg(&title);
                }
                match cmd.output() {
                    Ok(r) if r.status.success() => {
                        if let Some(id) = String::from_utf8_lossy(&r.stdout)
                            .lines()
                            .find_map(|l| l.strip_prefix("Added to Pocket Arcade: "))
                        {
                            let _ = tx.send(Update::Imported(id.into()));
                        }
                        built.push(name);
                    }
                    Ok(r) => errors.push(format!(
                        "{name}: {}",
                        String::from_utf8_lossy(&r.stderr).trim()
                    )),
                    Err(e) => errors.push(format!("Could not start converter: {e}")),
                }
            }
            let result = if built.is_empty() {
                Err(errors.join("\n"))
            } else {
                Ok(format!(
                    "Cartridge built for the {} and added. Review it, then press FLASH TO PICO.",
                    built.join(" and ")
                ))
            };
            let _ = tx.send(Update::Done(result));
            ctx.request_repaint();
        });
    }
    fn poll(&mut self, ctx: &egui::Context) {
        let mut done = false;
        if let Some(rx) = &self.receiver {
            while let Ok(update) = rx.try_recv() {
                match update {
                    Update::Stage(n, s) => {
                        self.progress = n;
                        self.message = s;
                    }
                    Update::Imported(id) => self.import_id = Some(id),
                    Update::Done(result) => {
                        done = true;
                        self.reload_needed = self.converting && result.is_ok();
                        self.failed = result.is_err();
                        self.completed = result.is_ok();
                        self.show_error = self.failed;
                        self.progress = if result.is_ok() { 1. } else { 0. };
                        self.message = result.unwrap_or_else(|s| s);
                    }
                }
            }
        }
        if done {
            self.receiver = None;
            self.device = flasher::detect();
        }
        if self.reload_needed {
            self.reload_needed = false;
            if let Ok(catalog) = flasher::load_catalog(&self.root) {
                self.catalog = catalog;
                self.selected = self
                    .import_id
                    .take()
                    .and_then(|id| self.catalog.cartridges.iter().position(|c| c.id == id))
                    .unwrap_or(0);
                self.search.clear();
                self.filter = Filter::All;
                self.scroll_selected = true;
                self.load_art(ctx);
            }
        }
        if self.last_detect.elapsed() > Duration::from_secs(1) && self.receiver.is_none() {
            self.device = flasher::detect();
            self.last_detect = Instant::now();
        }
    }
    fn tile(&mut self, ui: &mut egui::Ui, index: usize, width: f32, list: bool, busy: bool) {
        let c = &self.catalog.cartridges[index];
        let height = if list { 78. } else { width + 74. };
        let (r, response) = ui.allocate_exact_size(vec2(width, height), Sense::click());
        self.hits.push((index, r));
        let selected = index == self.selected;
        let p = ui.painter();
        p.rect_filled(
            r,
            3.,
            if selected {
                Color32::from_rgb(47, 41, 63)
            } else if response.hovered() {
                Color32::from_rgb(36, 31, 51)
            } else {
                PANEL
            },
        );
        if selected {
            p.rect_stroke(
                r.shrink(1.),
                3.,
                Stroke::new(2_f32, GREEN),
                egui::StrokeKind::Inside,
            );
        }
        if list {
            let art = Rect::from_min_size(r.min + vec2(10., 10.), vec2(58., 58.));
            self.draw_art(ui, art, c);
            let p = ui.painter();
            let clip = p.with_clip_rect(Rect::from_min_max(
                r.min + vec2(80., 0.),
                r.max - vec2(155., 0.),
            ));
            label(&clip, r.min + vec2(81., 13.), &c.title, 15., TEXT);
            label(&clip, r.min + vec2(81., 40.), &c.credits, 11., MUTED);
            p.text(
                r.right_top() + vec2(-12., 24.),
                Align2::RIGHT_CENTER,
                c.state_label(),
                mono(11.),
                status_color(c),
            );
            if let Some(rank) = c.rank {
                p.text(
                    r.right_top() + vec2(-12., 51.),
                    Align2::RIGHT_CENTER,
                    format!("TOP 200 / #{rank}"),
                    mono(10.),
                    MUTED,
                );
            }
        } else {
            let art = Rect::from_min_size(r.min + vec2(9., 9.), vec2(width - 18., width - 18.));
            self.draw_art(ui, art, c);
            let p = ui.painter();
            if let Some(rank) = c.rank {
                let rank_rect = Rect::from_min_size(art.min + vec2(5., 5.), vec2(47., 18.));
                p.rect_filled(rank_rect, 2., Color32::from_black_alpha(220));
                label(
                    p,
                    rank_rect.min + vec2(4., 2.),
                    &format!("#{rank}"),
                    10.,
                    TEXT,
                );
            }
            let text_rect = Rect::from_min_max(
                pos2(r.left() + 11., r.top() + width),
                pos2(r.right() - 10., r.bottom() - 27.),
            );
            let painter = p.with_clip_rect(text_rect);
            let text = painter.layout(c.title.clone(), mono(13.), TEXT, text_rect.width());
            painter.galley(text_rect.min, text, TEXT);
            label(
                p,
                pos2(r.left() + 11., r.bottom() - 22.),
                c.state_label(),
                10.,
                status_color(c),
            );
        }
        let clicked = response.clicked();
        if self.scroll_selected && selected {
            response.scroll_to_me(Some(egui::Align::Center));
        }
        response
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text(format!(
                "{}\n{}\n{}",
                c.title,
                c.credits,
                if c.ready() {
                    "Prepared firmware"
                } else {
                    &c.reason
                }
            ));
        if clicked && !busy {
            self.selected = index;
        }
    }
    fn details(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, has_results: bool, busy: bool) {
        if !has_results {
            ui.label("No matching cartridges.");
            return;
        }
        let c = self.catalog.cartridges[self.selected].clone();
        let color = status_color(&c);
        egui::ScrollArea::vertical()
            .id_salt("details")
            .show(ui, |ui| {
                ui.label(
                    egui::RichText::new("SELECTED CARTRIDGE")
                        .font(mono(11.))
                        .color(MUTED),
                );
                let w = ui.available_width();
                let (_, art) = ui.allocate_space(vec2(w, 220.));
                self.draw_art(ui, art, &c);
                ui.add_space(8.);
                ui.label(egui::RichText::new(&c.title).font(mono(22.)).strong());
                ui.label(egui::RichText::new(&c.subtitle).color(MUTED));
                ui.horizontal_wrapped(|ui| {
                    ui.label(egui::RichText::new(c.state_label()).color(color).strong());
                    if let Some(rank) = c.rank {
                        ui.label(egui::RichText::new(format!("TOP 200 / #{rank}")).color(MUTED));
                    }
                });
                ui.separator();
                ui.label(&c.description);
                if !c.ready() {
                    ui.add_space(4.);
                    ui.label(
                        egui::RichText::new("WHY IT CANNOT FLASH")
                            .font(mono(11.))
                            .color(GOLD),
                    );
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(&c.reason).font(mono(12.)).color(GOLD),
                        )
                        .wrap()
                        .selectable(true),
                    );
                }
                ui.add_space(5.);
                let caption = if c.ready() {
                    "FLASH TO PICO  >"
                } else {
                    "NO FIRMWARE TO FLASH"
                };
                let flash = ui.add_enabled(
                    c.ready() && self.device.available() && !busy,
                    egui::Button::new(
                        egui::RichText::new(caption)
                            .font(mono(15.))
                            .color(if c.ready() { BG } else { MUTED }),
                    )
                    .fill(if c.ready() { GREEN } else { LINE })
                    .min_size(vec2(ui.available_width(), 44.)),
                );
                if flash.clicked() {
                    self.start(ctx, c.clone());
                }
                if c.status == "source_only" {
                    ui.label(
                        egui::RichText::new("python3 build_cartridges.py")
                            .font(mono(12.))
                            .color(GOLD),
                    );
                }
                if [".p8", ".png", ".rom"]
                    .iter()
                    .any(|ext| c.source_file.to_ascii_lowercase().ends_with(ext))
                    && c.status != "source_only"
                    && ui
                        .add_enabled(
                            !busy,
                            egui::Button::new("TRY CONVERSION AGAIN")
                                .min_size(vec2(ui.available_width(), 32.)),
                        )
                        .clicked()
                {
                    self.import_path = self
                        .root
                        .join(&c.source_file)
                        .to_string_lossy()
                        .into_owned();
                    self.import_title = c.title.clone();
                    self.import_open = true;
                }
                if !c.controls.is_empty() {
                    ui.add_space(8.);
                    ui.label(egui::RichText::new("CONTROLS").font(mono(11.)).color(MUTED));
                    for line in &c.controls {
                        ui.label(egui::RichText::new(line).font(mono(12.)));
                    }
                }
                ui.separator();
                ui.label(egui::RichText::new(&c.credits).font(mono(11.)).color(MUTED));
                if !c.source_url.is_empty() {
                    ui.hyperlink_to("Original game page ↗", &c.source_url);
                }
                ui.add_space(6.);
                ui.label(
                    egui::RichText::new(
                        "RP2040 or RP2350 + WAVESHARE LCD 1.3\"\n240 × 240 / 9 inputs",
                    )
                    .font(mono(10.))
                    .color(MUTED),
                );
            });
    }
    fn render(&mut self, ctx: &egui::Context) {
        self.poll(ctx);
        self.hits.clear();
        if let Some(path) = ctx.input(|i| i.raw.dropped_files.iter().find_map(|f| f.path.clone())) {
            self.screen = Screen::Collection;
            self.import_path = path.to_string_lossy().into_owned();
            self.import_open = true;
        }
        let busy = self.receiver.is_some();
        let ready_count = self.catalog.cartridges.iter().filter(|c| c.ready()).count();
        egui::TopBottomPanel::top("header")
            .frame(
                egui::Frame::NONE
                    .fill(BG)
                    .inner_margin(egui::Margin::symmetric(22, 16)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("POCKET ARCADE")
                            .font(pixel(20.))
                            .color(TEXT),
                    );
                    ui.add_space(24.);
                    let collection =
                        ui.selectable_label(self.screen == Screen::Collection, "CARTRIDGES");
                    self.collection_button = collection.rect;
                    if collection.clicked() {
                        self.screen = Screen::Collection;
                    }
                    let progress = ui.selectable_label(self.screen == Screen::Progress, "PROGRESS");
                    self.progress_button = progress.rect;
                    if progress.clicked() {
                        self.screen = Screen::Progress;
                        self.dashboard = Dashboard::load(&self.root);
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let color = if self.device.available() {
                            GREEN
                        } else {
                            MUTED
                        };
                        let state = if busy {
                            if self.converting {
                                "BUILDING CARTRIDGE"
                            } else {
                                "FLASHING / KEEP USB CONNECTED"
                            }
                        } else {
                            self.device.label()
                        };
                        ui.label(
                            egui::RichText::new(format!("● {state}"))
                                .font(mono(11.))
                                .color(color),
                        );
                    });
                });
                ui.add_space(8.);
                if self.screen == Screen::Collection {
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut self.search)
                                .hint_text("Search games or creators...")
                                .desired_width((ui.available_width() * 0.28).clamp(180., 330.)),
                        );
                        egui::ComboBox::from_id_salt("filter")
                            .selected_text(match self.filter {
                                Filter::All => "All cartridges",
                                Filter::Ready => "Ready to flash",
                                Filter::NeedsWork => "Needs conversion",
                                Filter::Top200 => "Top 200 sample",
                            })
                            .show_ui(ui, |ui| {
                                for (f, s) in [
                                    (Filter::All, "All cartridges"),
                                    (Filter::Ready, "Ready to flash"),
                                    (Filter::NeedsWork, "Needs conversion"),
                                    (Filter::Top200, "Top 200 sample"),
                                ] {
                                    ui.selectable_value(&mut self.filter, f, s);
                                }
                            });
                        egui::ComboBox::from_id_salt("sort")
                            .selected_text(if self.rank_sort {
                                "Sort: rank"
                            } else {
                                "Sort: A–Z"
                            })
                            .width(105.)
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut self.rank_sort, false, "Sort: A–Z");
                                ui.selectable_value(&mut self.rank_sort, true, "Sort: rank");
                            });
                        let grid = ui.selectable_label(self.view == View::Grid, "GRID");
                        self.grid_button = grid.rect;
                        if grid.clicked() {
                            self.set_view(View::Grid);
                        }
                        let list = ui.selectable_label(self.view == View::List, "LIST");
                        self.list_button = list.rect;
                        if list.clicked() {
                            self.set_view(View::List);
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if let Some(recovery) = self.catalog.recovery.clone() {
                                let restore = ui.add_enabled(
                                    !busy && self.device.available(),
                                    egui::Button::new("RESTORE"),
                                );
                                if restore
                                    .on_hover_text("Restore a configured local backup")
                                    .clicked()
                                {
                                    self.start(ctx, recovery);
                                }
                            }
                            let add = ui.add_enabled(
                                !busy,
                                egui::Button::new("+ ADD CARTRIDGE").fill(LINE),
                            );
                            self.add_button = add.rect;
                            if add.clicked() {
                                self.import_open = true;
                            }
                        });
                    });
                } else {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(
                                "TRANSLATOR COVERAGE / IMPLEMENTATION + EXECUTION EVIDENCE",
                            )
                            .font(mono(11.))
                            .color(MUTED),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("REFRESH DATA").clicked() {
                                self.dashboard = Dashboard::load(&self.root);
                            }
                        });
                    });
                }
            });
        egui::TopBottomPanel::bottom("status")
            .frame(
                egui::Frame::NONE
                    .fill(PANEL)
                    .inner_margin(egui::Margin::symmetric(22, 12)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!(
                            "{} CARTRIDGES  /  {} READY",
                            self.catalog.cartridges.len(),
                            ready_count
                        ))
                        .font(mono(11.))
                        .color(MUTED),
                    );
                    ui.separator();
                    let text = if self.failed {
                        "Action needs attention."
                    } else if self.screen==Screen::Progress && !busy {
                        "Implementation, host test execution and game results are separate measures."
                    } else {
                        &self.message
                    };
                    ui.label(
                        egui::RichText::new(text)
                            .font(mono(12.))
                            .color(if self.failed {
                                PINK
                            } else if self.completed {
                                GREEN
                            } else {
                                TEXT
                            }),
                    );
                    if self.failed && ui.small_button("DETAILS").clicked() {
                        self.show_error = true;
                    }
                });
                if busy || self.completed {
                    ui.add(
                        egui::ProgressBar::new(self.progress)
                            .desired_height(4.)
                            .fill(GREEN),
                    );
                }
            });
        if self.screen == Screen::Progress {
            egui::CentralPanel::default().frame(egui::Frame::NONE.fill(BG).inner_margin(20.)).show(ctx,|ui| {
                match &self.dashboard {
                    Ok(data)=>self.progress_view.render(ui,data),
                    Err(error)=>{
                        ui.label(egui::RichText::new("PROGRESS DATA UNAVAILABLE").font(pixel(16.)).color(GOLD));
                        ui.add_space(12.);ui.label(error);
                        ui.label("Refresh after restoring compatibility/coverage reports. Cartridge browsing remains available.");
                    }
                }
            });
        } else {
            let indices = visible(&self.catalog, &self.search, self.filter, self.rank_sort);
            if !indices.is_empty() && !indices.contains(&self.selected) {
                self.selected = indices[0];
                self.scroll_selected = true;
            }
            if !ctx.wants_keyboard_input() && !busy && !indices.is_empty() {
                let current = indices
                    .iter()
                    .position(|i| *i == self.selected)
                    .unwrap_or(0);
                let columns = self.columns;
                let movement = ctx.input(|i| {
                    if i.key_pressed(egui::Key::ArrowRight) {
                        1
                    } else if i.key_pressed(egui::Key::ArrowLeft) {
                        -1
                    } else if i.key_pressed(egui::Key::ArrowDown) {
                        if self.view == View::Grid {
                            columns as isize
                        } else {
                            1
                        }
                    } else if i.key_pressed(egui::Key::ArrowUp) {
                        if self.view == View::Grid {
                            -(columns as isize)
                        } else {
                            -1
                        }
                    } else {
                        0
                    }
                });
                if movement != 0 {
                    self.selected = indices[(current as isize + movement)
                        .clamp(0, indices.len() as isize - 1)
                        as usize];
                    self.scroll_selected = true;
                }
            }
            egui::SidePanel::right("cartridge-details")
                .default_width(330.)
                .min_width(285.)
                .max_width(430.)
                .resizable(true)
                .frame(egui::Frame::NONE.fill(PANEL).inner_margin(18.))
                .show(ctx, |ui| self.details(ui, ctx, !indices.is_empty(), busy));
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE.fill(BG).inner_margin(20.))
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(if self.filter == Filter::Top200 {
                                "RANDOM TOP 200 SAMPLE"
                            } else {
                                "YOUR COLLECTION"
                            })
                            .font(mono(12.))
                            .color(MUTED),
                        );
                        ui.label(
                            egui::RichText::new(format!("{} shown", indices.len()))
                                .font(mono(11.))
                                .color(MUTED),
                        );
                    });
                    ui.add_space(6.);
                    if indices.is_empty() {
                        ui.add_space(50.);
                        ui.vertical_centered(|ui| {
                            ui.label(egui::RichText::new("No cartridges match.").font(mono(20.)));
                            if ui.button("CLEAR SEARCH AND FILTER").clicked() {
                                self.search.clear();
                                self.filter = Filter::All;
                            }
                        });
                        return;
                    }
                    egui::ScrollArea::vertical()
                        .id_salt("collection")
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            if self.view == View::Grid {
                                let available = ui.available_width();
                                let cols = ((available + 12.) / 182.).floor().max(1.) as usize;
                                let width = (available - (cols - 1) as f32 * 12.) / cols as f32;
                                self.columns = cols;
                                for row in indices.chunks(cols) {
                                    ui.horizontal(|ui| {
                                        ui.spacing_mut().item_spacing.x = 12.;
                                        for i in row {
                                            self.tile(ui, *i, width, false, busy);
                                        }
                                    });
                                    ui.add_space(3.);
                                }
                            } else {
                                let width = ui.available_width();
                                for i in &indices {
                                    self.tile(ui, *i, width, true, busy);
                                    ui.add_space(2.);
                                }
                            }
                        });
                });
        }
        self.scroll_selected = false;
        if self.show_error {
            egui::Window::new("Action details")
                .open(&mut self.show_error)
                .default_width(640.)
                .show(ctx, |ui| {
                    egui::ScrollArea::vertical()
                        .max_height(450.)
                        .show(ui, |ui| {
                            ui.add(egui::Label::new(&self.message).wrap().selectable(true));
                        });
                });
        }
        let mut convert = false;
        if self.import_open {
            egui::Window::new("Add a PICO-8 cartridge").open(&mut self.import_open).default_width(650.).show(ctx,|ui|{
   ui.label("Choose a local .p8, .p8.png or .p8.rom cartridge, or drag it into Pocket Arcade.");ui.add_space(6.);
   ui.label("Cartridge path");ui.add(egui::TextEdit::singleline(&mut self.import_path).desired_width(620.));
   ui.label("Title (optional)");ui.add(egui::TextEdit::singleline(&mut self.import_title).desired_width(620.));ui.add_space(8.);
   ui.label("Conversion runs on this computer. Unsupported features are reported. A prepared cartridge is added only after build checks pass.");
   convert=ui.add_enabled(!busy&&!self.import_path.trim().is_empty(),egui::Button::new("CONVERT + ADD TO COLLECTION")).clicked();
  });
        }
        if convert {
            self.start_conversion(ctx);
        }
        ctx.request_repaint_after(Duration::from_millis(if busy { 100 } else { 700 }));
    }
}
impl eframe::App for Arcade {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        self.render(ctx);
    }
}

fn main() -> eframe::Result {
    let root = flasher::root_dir();
    let catalog = flasher::load_catalog(&root).unwrap_or_else(|e| {
        eprintln!("Cannot load Pocket Arcade catalog: {e}");
        std::process::exit(1)
    });
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).is_some_and(|a| a == "--check") {
        for c in catalog.cartridges.iter().chain(catalog.recovery.iter()) {
            if !c.ready() {
                println!("SKIP  {} ({})", c.title, c.state_label());
                continue;
            }
            for chip in [flasher::Chip::Rp2040, flasher::Chip::Rp2350] {
                let Some((file, hash)) = c.image(chip) else {
                    continue;
                };
                let r = std::fs::read(root.join(file))
                    .map_err(|e| e.to_string())
                    .and_then(|b| flasher::verify_image(&b, hash, chip));
                if let Err(e) = r {
                    eprintln!("{} ({}): {e}", c.title, chip.label());
                    std::process::exit(1);
                }
            }
            println!("OK  {}", c.title);
        }
        println!("USB: {}", flasher::detect().label());
        return Ok(());
    }
    if args.get(1).is_some_and(|a| a == "--flash") {
        let id = args.get(2).map(String::as_str).unwrap_or("");
        let c = catalog
            .cartridges
            .iter()
            .chain(catalog.recovery.iter())
            .find(|c| c.id == id)
            .unwrap_or_else(|| {
                eprintln!("Unknown cartridge ID: {id}");
                std::process::exit(2)
            });
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let mut last = String::new();
            for u in rx {
                if let Update::Stage(n, s) = u {
                    if s != last {
                        println!("{:3.0}% {s}", n * 100.);
                        last = s;
                    }
                }
            }
        });
        match flasher::flash(&root, c, &tx) {
            Ok(s) => println!("{s}"),
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(1)
            }
        }
        return Ok(());
    }
    eframe::run_native(
        "Pocket Arcade",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_inner_size([1300., 850.])
                .with_min_inner_size([1100., 720.]),
            ..Default::default()
        },
        Box::new(move |cc| {
            let mut app = Arcade::new(&cc.egui_ctx, root, catalog);
            if let Some(id) = args
                .iter()
                .position(|a| a == "--select")
                .and_then(|i| args.get(i + 1))
            {
                if let Some(i) = app.catalog.cartridges.iter().position(|c| &c.id == id) {
                    app.selected = i;
                }
            }
            if let Some(view) = args
                .iter()
                .position(|a| a == "--view")
                .and_then(|i| args.get(i + 1))
            {
                if view == "progress" {
                    app.screen = Screen::Progress;
                }
                app.view = if view == "list" {
                    View::List
                } else {
                    View::Grid
                };
            }
            if let Some(tab) = args
                .iter()
                .position(|a| a == "--progress-tab")
                .and_then(|i| args.get(i + 1))
            {
                app.screen = Screen::Progress;
                app.progress_view.select_tab(tab);
            }
            Ok(Box::new(app))
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn app() -> (egui::Context, Arcade) {
        // Tests must read this checkout, regardless of a running launcher's environment.
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let mut cat = flasher::load_catalog(&root).unwrap();
        // UI tests use controlled metadata, independent of locally built UF2s.
        for c in &mut cat.cartridges {
            c.status = "source_only".into();
            c.filename.clear();
            c.sha256.clear();
            c.rank = None;
        }
        cat.cartridges[0].status = "ready".into();
        cat.cartridges[0].filename = "fixture-not-flashed.uf2".into();
        cat.cartridges[0].sha256 = "fixture-not-flashed".into();
        cat.cartridges[0].rank = Some(12);
        let ctx = egui::Context::default();
        let mut app = Arcade::new(&ctx, root, cat);
        app.view = View::Grid;
        (ctx, app)
    }
    fn frame(app: &mut Arcade, ctx: &egui::Context, events: Vec<egui::Event>) {
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(pos2(0., 0.), vec2(1300., 850.))),
                events,
                ..Default::default()
            },
            |ctx| app.render(ctx),
        );
    }
    fn click(app: &mut Arcade, ctx: &egui::Context, p: egui::Pos2) {
        for pressed in [true, false] {
            frame(
                app,
                ctx,
                vec![
                    egui::Event::PointerMoved(p),
                    egui::Event::PointerButton {
                        pos: p,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: Default::default(),
                    },
                ],
            );
        }
    }
    #[test]
    fn search_and_status_filters() {
        let (_, a) = app();
        assert_eq!(visible(&a.catalog, "", Filter::Ready, false).len(), 1);
        assert_eq!(visible(&a.catalog, "", Filter::Top200, false).len(), 1);
        let ids = visible(&a.catalog, "prism", Filter::All, false);
        assert_eq!(ids.len(), 1);
        assert_eq!(a.catalog.cartridges[ids[0]].title, "PRISM GARDEN");
    }
    #[test]
    fn grid_selection_list_toggle_and_import_controls() {
        let (ctx, mut a) = app();
        frame(&mut a, &ctx, vec![]);
        frame(&mut a, &ctx, vec![]);
        let (i, r) = a.hits[1];
        click(&mut a, &ctx, r.center());
        assert_eq!(a.selected, i);
        let r = a.list_button;
        click(&mut a, &ctx, r.center());
        assert_eq!(a.view, View::List);
        let r = a.add_button;
        click(&mut a, &ctx, r.center());
        assert!(a.import_open);
    }
    #[test]
    fn thumbnail_falls_back_to_cartridge_when_missing() {
        let (_, mut a) = app();
        let mut c = a.catalog.cartridges[0].clone();
        c.thumbnail = "nomad".into();
        c.cart_image = "prism".into();
        assert_eq!(a.art_key(&c), Some(c.thumbnail.as_str()));
        a.covers.remove(&c.thumbnail);
        assert_eq!(a.art_key(&c), Some(c.cart_image.as_str()));
    }
    #[test]
    fn unsupported_selection_does_not_start_flash() {
        let (ctx, mut a) = app();
        let c = a
            .catalog
            .cartridges
            .iter()
            .find(|c| !c.ready())
            .unwrap()
            .clone();
        a.start(&ctx, c);
        assert!(a.receiver.is_none());
    }
    #[test]
    fn progress_navigation_preserves_cartridge_and_never_starts_a_job() {
        let (ctx, mut a) = app();
        a.selected = 1;
        let selected = a.selected;
        frame(&mut a, &ctx, vec![]);
        frame(&mut a, &ctx, vec![]);
        let button = a.progress_button;
        click(&mut a, &ctx, button.center());
        assert_eq!(a.screen, Screen::Progress);
        let dashboard = a.dashboard.as_ref().unwrap();
        assert_eq!(dashboard.api_counts.total(), 108);
        let inventory: serde_json::Value = serde_json::from_slice(
            &std::fs::read(a.root.join("compatibility/coverage/api-audit.json")).unwrap(),
        )
        .unwrap();
        let implemented = inventory["apis"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|row| row["implemented"].as_bool() == Some(true))
            .count();
        assert_eq!(dashboard.api_counts.functional(), implemented);
        assert_eq!(dashboard.sample_built, 1);
        assert_eq!(dashboard.sample_confirmed, 1);
        assert!(a.receiver.is_none());
        let button = a.collection_button;
        click(&mut a, &ctx, button.center());
        assert_eq!(a.screen, Screen::Collection);
        assert_eq!(a.selected, selected);
        assert!(a.receiver.is_none());
    }
    #[test]
    fn missing_progress_reports_leave_the_cartridge_browser_available() {
        let (ctx, mut a) = app();
        a.dashboard = Err("Missing test report".into());
        a.screen = Screen::Progress;
        frame(&mut a, &ctx, vec![]);
        frame(&mut a, &ctx, vec![]);
        let button = a.collection_button;
        click(&mut a, &ctx, button.center());
        assert_eq!(a.screen, Screen::Collection);
        assert!(a.receiver.is_none());
    }
    #[test]
    fn failed_conversion_preserves_library() {
        let (ctx, mut a) = app();
        let before = std::fs::read(a.root.join("catalog.json")).unwrap();
        a.import_path = a
            .root
            .join("converter/tests/missing-fixture.p8")
            .to_string_lossy()
            .into_owned();
        a.start_conversion(&ctx);
        let started = Instant::now();
        while a.receiver.is_some() && started.elapsed() < Duration::from_secs(5) {
            std::thread::sleep(Duration::from_millis(20));
            a.poll(&ctx);
        }
        assert!(a.receiver.is_none());
        assert!(a.failed && a.show_error);
        assert_eq!(before, std::fs::read(a.root.join("catalog.json")).unwrap());
    }
}
