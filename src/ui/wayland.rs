use fontdue::{Font, FontSettings};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::config::{ColorSchemeRgb, Config, PasswordWindowConfig, PickerWindowConfig};
use crate::database::{open_database, Entry};

/// UI mode - password entry or entry picker
#[derive(Debug, Clone, PartialEq)]
pub enum Mode {
    Password,
    Picker,
}

use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    delegate_compositor, delegate_keyboard, delegate_layer, delegate_output, delegate_pointer,
    delegate_registry, delegate_seat, delegate_shm,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        keyboard::{KeyEvent, KeyboardHandler, Keysym, Modifiers},
        pointer::{PointerEvent, PointerHandler},
        Capability, SeatHandler, SeatState,
    },
    shell::{
        wlr_layer::{
            Anchor, KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface,
            LayerSurfaceConfigure,
        },
        WaylandSurface,
    },
    shm::{slot::SlotPool, Shm, ShmHandler},
};
use wayland_client::{
    globals::registry_queue_init,
    protocol::{wl_keyboard, wl_output, wl_pointer, wl_seat, wl_shm, wl_surface},
    Connection, EventQueue, QueueHandle,
};

/// Attempts to load a font by family name from system directories
fn load_font_by_family(family: &str) -> Font {
    let search_dirs = [
        "/usr/share/fonts",
        "/usr/local/share/fonts",
        "/run/current-system/sw/share/X11/fonts",
    ];

    // Also check ~/.local/share/fonts
    let home_fonts = dirs::home_dir().map(|h| h.join(".local/share/fonts"));

    let family_lower = family.to_lowercase();

    // Search for font matching family name
    for dir in search_dirs.iter().map(PathBuf::from).chain(home_fonts) {
        if let Some(font) = search_font_dir(&dir, &family_lower) {
            return font;
        }
    }

    // Fallback: try to find any TTF
    for dir in search_dirs.iter().map(PathBuf::from) {
        if let Some(font) = find_any_font(&dir) {
            return font;
        }
    }

    panic!("No fonts found. Please install DejaVu Sans or another TTF font.");
}

fn search_font_dir(dir: &Path, family_lower: &str) -> Option<Font> {
    let entries = fs::read_dir(dir).ok()?;

    for entry in entries.flatten() {
        let path = entry.path();

        if path.is_dir() {
            if let Some(font) = search_font_dir(&path, family_lower) {
                return Some(font);
            }
        } else if path.extension().map_or(false, |e| e == "ttf" || e == "otf") {
            let name = path.file_stem()?.to_string_lossy().to_lowercase();
            // Match if filename contains family name (handles "DejaVuSans", "DejaVu-Sans", etc.)
            let family_normalized = family_lower.replace(' ', "");
            if name.contains(&family_normalized) || name.contains(family_lower) {
                if let Ok(data) = fs::read(&path) {
                    if let Ok(font) = Font::from_bytes(data, FontSettings::default()) {
                        return Some(font);
                    }
                }
            }
        }
    }
    None
}

fn find_any_font(dir: &Path) -> Option<Font> {
    let entries = fs::read_dir(dir).ok()?;

    for entry in entries.flatten() {
        let path = entry.path();

        if path.is_dir() {
            if let Some(font) = find_any_font(&path) {
                return Some(font);
            }
        } else if path.extension().map_or(false, |e| e == "ttf") {
            if let Ok(data) = fs::read(&path) {
                if let Ok(font) = Font::from_bytes(data, FontSettings::default()) {
                    return Some(font);
                }
            }
        }
    }
    None
}

pub struct AppState {
    pub running: bool,
    pub width: u32,
    pub height: u32,

    registry_state: RegistryState,
    seat_state: SeatState,
    output_state: OutputState,
    shm: Shm,
    pool: Option<SlotPool>,

    #[allow(dead_code)]
    layer_shell: LayerShell,
    layer_surface: Option<LayerSurface>,
    keyboard: Option<wl_keyboard::WlKeyboard>,
    pointer: Option<wl_pointer::WlPointer>,

    // Font for text rendering
    font: Font,

    // Color scheme
    colors: ColorSchemeRgb,

    // Current mode
    pub mode: Mode,

    // Password mode state
    password: String,
    password_error: Option<String>,
    last_keypress: Option<Instant>,  // Time of last password keypress for flash
    db_path: PathBuf,

    // Picker mode state
    pub query: String,
    pub selected_index: usize,
    pub entries: Vec<(String, String)>,

    // Keyboard modifier state
    modifiers: Modifiers,

    // Callbacks for when user makes a selection
    // Parameters: (index, copy_username) - if copy_username is true, copy username instead of password
    pub on_select: Option<Box<dyn FnMut(usize, bool)>>,
    pub on_escape: Option<Box<dyn FnMut()>>,
    pub on_query_change: Option<Box<dyn FnMut(&str)>>,
    pub on_unlock: Option<Box<dyn FnMut(Vec<Entry>)>>,

    // Track if we need to redraw
    needs_redraw: bool,

    // Track if we're waiting for a frame callback (prevents drawing too fast)
    frame_pending: bool,

    // Track if we've sized to the output yet
    sized_to_output: bool,

    // Screen dimensions for resizing
    screen_width: u32,
    screen_height: u32,

    // Config values
    flash_duration_ms: u64,
    font_size: f32,
    hints_font_size: f32,
    password_window: PasswordWindowConfig,
    picker_window: PickerWindowConfig,
    max_entries: usize,
}

impl AppState {
    /// Create a new AppState and its associated event queue
    pub fn new(conn: &Connection, config: &Config, db_path: PathBuf) -> (Self, EventQueue<Self>) {
        let (globals, event_queue) = registry_queue_init::<Self>(conn).unwrap();
        let qh = event_queue.handle();
        let registry_state = RegistryState::new(&globals);

        let shm = Shm::bind(&globals, &qh).expect("wl_shm not available");
        let compositor = CompositorState::bind(&globals, &qh).expect("wl_compositor not available");
        let layer_shell = LayerShell::bind(&globals, &qh).expect("layer shell not available");
        let seat_state = SeatState::new(&globals, &qh);
        let output_state = OutputState::new(&globals, &qh);

        // Load our font for text rendering
        let font = load_font_by_family(&config.font.family);

        // Convert colors to RGB
        let colors = config.colors.to_rgb();

        // Create our surface
        let surface = compositor.create_surface(&qh);

        // Create layer surface with temporary size (will resize after roundtrip)
        let layer_surface =
            layer_shell.create_layer_surface(&qh, surface, Layer::Overlay, Some("kpick"), None);

        // Configure layer surface - no anchor means centered
        layer_surface.set_anchor(Anchor::empty());
        layer_surface.set_size(400, 172); // Temporary - will resize based on mode
        layer_surface.set_keyboard_interactivity(KeyboardInteractivity::Exclusive);
        layer_surface.commit();

        let state = Self {
            running: true,
            width: 400,
            height: 172,
            registry_state,
            seat_state,
            output_state,
            shm,
            pool: None,
            layer_shell,
            layer_surface: Some(layer_surface),
            keyboard: None,
            pointer: None,
            font,
            colors,
            mode: Mode::Password,
            password: String::new(),
            password_error: None,
            last_keypress: None,
            db_path,
            query: String::new(),
            selected_index: 0,
            entries: Vec::new(),
            modifiers: Modifiers::default(),
            on_select: None,
            on_escape: None,
            on_query_change: None,
            on_unlock: None,
            needs_redraw: true,
            frame_pending: false,
            sized_to_output: false,
            screen_width: 0,
            screen_height: 0,
            flash_duration_ms: config.flash_duration,
            font_size: config.font.size,
            hints_font_size: config.font.hints_size,
            password_window: config.window.password.clone(),
            picker_window: config.window.picker.clone(),
            max_entries: config.window.picker.max_entries,
        };

        (state, event_queue)
    }

    /// Run the event loop until the user makes a selection or presses escape
    #[allow(dead_code)]
    pub fn run(&mut self, conn: &Connection, event_queue: &mut EventQueue<Self>) {
        let qh = event_queue.handle();

        while self.running {
            // Flush pending requests
            conn.flush().unwrap();

            // Blocking dispatch - waits for events
            event_queue.blocking_dispatch(self).unwrap();

            // Redraw after processing events
            self.request_redraw(&qh);
        }
    }

    /// Update the list of entries to display
    pub fn set_entries(&mut self, entries: Vec<(String, String)>) {
        self.entries = entries;
        self.needs_redraw = true;
    }

    /// Mark that we need to redraw
    #[allow(dead_code)]
    pub fn mark_dirty(&mut self) {
        self.needs_redraw = true;
    }
}

// Implement all the required handlers
impl CompositorHandler for AppState {
    fn scale_factor_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_factor: i32,
    ) {
    }

    fn transform_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_transform: wl_output::Transform,
    ) {
    }

    fn frame(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _time: u32,
    ) {
        // Frame callback fires when compositor displays our frame
        // Clear the pending flag so we can draw again
        self.frame_pending = false;
    }

    fn surface_enter(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        output: &wl_output::WlOutput,
    ) {
        // Only resize once when we first enter an output
        if self.sized_to_output {
            return;
        }
        self.sized_to_output = true;

        // Get and store output dimensions
        if let Some(info) = self.output_state.info(output) {
            if let Some((screen_width, screen_height)) = info.logical_size {
                self.screen_width = screen_width as u32;
                self.screen_height = screen_height as u32;

                // Size based on current mode
                let (width, height) = self.size_for_mode();

                self.width = width;
                self.height = height;

                if let Some(ref layer) = self.layer_surface {
                    layer.set_size(width, height);
                    layer.commit();
                }

                // Need to recreate the buffer pool for new size
                self.pool = None;
                self.needs_redraw = true;
            }
        }
    }

    fn surface_leave(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
    }
}

impl OutputHandler for AppState {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }

    fn new_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }

    fn update_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }

    fn output_destroyed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }
}

impl LayerShellHandler for AppState {
    fn closed(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _layer: &LayerSurface) {
        self.running = false;
    }

    fn configure(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        layer: &LayerSurface,
        configure: LayerSurfaceConfigure,
        _serial: u32,
    ) {
        if configure.new_size.0 > 0 {
            self.width = configure.new_size.0;
        }
        if configure.new_size.1 > 0 {
            self.height = configure.new_size.1;
        }

        // Create buffer pool if needed
        if self.pool.is_none() {
            self.pool = Some(
                SlotPool::new(self.width as usize * self.height as usize * 4, &self.shm)
                    .expect("Failed to create pool"),
            );
        }

        // Initial draw - clone layer to avoid borrow issues
        self.needs_redraw = true;
        let layer_clone = layer.clone();
        self.draw(qh, &layer_clone);
    }
}

impl SeatHandler for AppState {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seat_state
    }

    fn new_seat(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _seat: wl_seat::WlSeat) {}

    fn new_capability(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Keyboard && self.keyboard.is_none() {
            self.keyboard = Some(self.seat_state.get_keyboard(qh, &seat, None).unwrap());
        }
        if capability == Capability::Pointer && self.pointer.is_none() {
            self.pointer = Some(self.seat_state.get_pointer(qh, &seat).unwrap());
        }
    }

    fn remove_capability(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _seat: wl_seat::WlSeat,
        _capability: Capability,
    ) {
    }

    fn remove_seat(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _seat: wl_seat::WlSeat) {
    }
}

impl KeyboardHandler for AppState {
    fn enter(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _surface: &wl_surface::WlSurface,
        _serial: u32,
        _raw: &[u32],
        _keysyms: &[Keysym],
    ) {
    }

    fn leave(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _surface: &wl_surface::WlSurface,
        _serial: u32,
    ) {
    }

    fn press_key(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _serial: u32,
        event: KeyEvent,
    ) {
        match self.mode {
            Mode::Password => self.handle_password_key(event),
            Mode::Picker => self.handle_picker_key(event),
        }
    }

    fn release_key(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _serial: u32,
        _event: KeyEvent,
    ) {
    }

    fn update_modifiers(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _serial: u32,
        modifiers: Modifiers,
        _layout: u32,
    ) {
        self.modifiers = modifiers;
    }
}

impl PointerHandler for AppState {
    fn pointer_frame(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _pointer: &wl_pointer::WlPointer,
        _events: &[PointerEvent],
    ) {
    }
}

impl ShmHandler for AppState {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

impl ProvidesRegistryState for AppState {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }

    registry_handlers!(OutputState, SeatState);
}

/// Fill a rectangle with a solid color (free function to avoid borrow issues)
fn fill_rect(
    canvas: &mut [u8],
    stride: usize,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    r: u8,
    g: u8,
    b: u8,
) {
    for row in y..(y + h) {
        for col in x..(x + w) {
            let idx = (row * stride + col) * 4;
            if idx + 3 < canvas.len() {
                canvas[idx] = b; // B
                canvas[idx + 1] = g; // G
                canvas[idx + 2] = r; // R
                canvas[idx + 3] = 255; // A
            }
        }
    }
}

/// Draw text vertically centered in a row
/// `y` is the top of the row, `row_height` is the height to center within
fn draw_text_centered(
    font: &Font,
    canvas: &mut [u8],
    stride: usize,
    canvas_height: usize,
    text: &str,
    x: usize,
    y: usize,
    row_height: usize,
    size: f32,
    color: (u8, u8, u8),
) {
    let mut cursor_x = x as i32;
    let (r, g, b) = color;

    // Get font metrics for vertical centering
    let line_metrics = font.horizontal_line_metrics(size);
    let text_height = if let Some(m) = line_metrics {
        (m.ascent - m.descent) as i32
    } else {
        size as i32
    };

    // Calculate baseline y for vertical centering
    let ascent = line_metrics.map(|m| m.ascent as i32).unwrap_or(size as i32);
    let baseline_y = y as i32 + (row_height as i32 - text_height) / 2 + ascent;

    for ch in text.chars() {
        let (metrics, bitmap) = font.rasterize(ch, size);

        // Position glyph relative to baseline
        let glyph_x = cursor_x + metrics.xmin;
        let glyph_y = baseline_y - metrics.height as i32 - metrics.ymin;

        // Copy glyph bitmap to canvas
        for row in 0..metrics.height {
            for col in 0..metrics.width {
                let px = glyph_x + col as i32;
                let py = glyph_y + row as i32;

                if px >= 0 && (px as usize) < stride && py >= 0 && (py as usize) < canvas_height {
                    let alpha = bitmap[row * metrics.width + col];
                    if alpha > 0 {
                        let idx = (py as usize * stride + px as usize) * 4;
                        if idx + 3 < canvas.len() {
                            // Alpha blend the glyph onto the background
                            let bg_b = canvas[idx] as u16;
                            let bg_g = canvas[idx + 1] as u16;
                            let bg_r = canvas[idx + 2] as u16;
                            let a = alpha as u16;
                            let inv_a = 255 - a;

                            canvas[idx] = ((b as u16 * a + bg_b * inv_a) / 255) as u8;
                            canvas[idx + 1] = ((g as u16 * a + bg_g * inv_a) / 255) as u8;
                            canvas[idx + 2] = ((r as u16 * a + bg_r * inv_a) / 255) as u8;
                        }
                    }
                }
            }
        }

        cursor_x += metrics.advance_width as i32;
    }
}

impl AppState {
    /// Calculate appropriate window size for current mode
    fn size_for_mode(&self) -> (u32, u32) {
        match self.mode {
            Mode::Password => {
                let content_width = self.password_window.width;
                let content_height = self.password_window.height;
                let max_percent = self.password_window.max_percent;

                let max_width = self.screen_width * max_percent / 100;
                let max_height = self.screen_height * max_percent / 100;

                let width = content_width.min(max_width).max(300);
                let height = content_height.min(max_height).max(150);

                (width, height)
            }
            Mode::Picker => {
                let width = self.screen_width * self.picker_window.width_percent / 100;
                let height = self.screen_height * self.picker_window.height_percent / 100;
                (width.max(400), height.max(200))
            }
        }
    }

    /// Resize window for current mode
    fn resize_for_mode(&mut self) {
        let (width, height) = self.size_for_mode();

        if width != self.width || height != self.height {
            self.width = width;
            self.height = height;

            if let Some(ref layer) = self.layer_surface {
                layer.set_size(width, height);
                layer.commit();
            }

            // Need to recreate the buffer pool for new size
            self.pool = None;
            self.needs_redraw = true;
        }
    }

    /// Trigger a redraw of the UI if needed
    pub fn request_redraw(&mut self, qh: &QueueHandle<Self>) {
        // Don't draw if we're waiting for the previous frame to be displayed
        if !self.needs_redraw || self.frame_pending {
            return;
        }
        self.needs_redraw = false;

        // Clone the layer surface to avoid borrow issues
        let layer = match self.layer_surface.as_ref() {
            Some(l) => l.clone(),
            None => return,
        };
        self.draw(qh, &layer);
    }

    /// Handle key press in password mode
    fn handle_password_key(&mut self, event: KeyEvent) {
        match event.keysym {
            Keysym::Escape => {
                if let Some(ref mut cb) = self.on_escape {
                    cb();
                }
                self.running = false;
            }
            Keysym::Return | Keysym::KP_Enter => {
                self.try_unlock();
            }
            Keysym::BackSpace => {
                if !self.password.is_empty() {
                    self.password.pop();
                    self.password_error = None;
                    self.last_keypress = Some(Instant::now());
                    self.needs_redraw = true;
                }
            }
            _ => {
                // Handle text input for password
                if let Some(c) = event.utf8.as_ref().and_then(|s| s.chars().next()) {
                    // Accept printable characters
                    if !c.is_control() {
                        self.password.push(c);
                        self.password_error = None;
                        self.last_keypress = Some(Instant::now());
                        self.needs_redraw = true;
                    }
                }
            }
        }
    }

    /// Handle key press in picker mode
    fn handle_picker_key(&mut self, event: KeyEvent) {
        match event.keysym {
            Keysym::Escape => {
                if let Some(ref mut cb) = self.on_escape {
                    cb();
                }
                self.running = false;
            }
            Keysym::Return | Keysym::KP_Enter => {
                if let Some(ref mut cb) = self.on_select {
                    // Shift+Enter copies username instead of password
                    let copy_username = self.modifiers.shift;
                    cb(self.selected_index, copy_username);
                }
                self.running = false;
            }
            Keysym::Up => {
                if self.selected_index > 0 {
                    self.selected_index -= 1;
                    self.needs_redraw = true;
                }
            }
            Keysym::Down => {
                if self.selected_index + 1 < self.entries.len() {
                    self.selected_index += 1;
                    self.needs_redraw = true;
                }
            }
            Keysym::BackSpace => {
                if !self.query.is_empty() {
                    self.query.pop();
                    self.selected_index = 0;
                    self.needs_redraw = true;
                    if let Some(ref mut cb) = self.on_query_change {
                        cb(&self.query);
                    }
                }
            }
            _ => {
                // Handle text input
                if let Some(c) = event.utf8.as_ref().and_then(|s| s.chars().next()) {
                    if c.is_ascii_graphic() || c == ' ' {
                        self.query.push(c);
                        self.selected_index = 0;
                        self.needs_redraw = true;
                        if let Some(ref mut cb) = self.on_query_change {
                            cb(&self.query);
                        }
                    }
                }
            }
        }
    }

    /// Try to unlock the database with the current password
    fn try_unlock(&mut self) {
        match open_database(&self.db_path, &self.password) {
            Ok(entries) => {
                // Success - call callback and switch to picker mode
                if let Some(ref mut cb) = self.on_unlock {
                    cb(entries);
                }
                self.password.clear();
                self.password_error = None;
                self.mode = Mode::Picker;
                self.resize_for_mode();
                self.needs_redraw = true;
            }
            Err(crate::database::DatabaseError::InvalidPassword) => {
                self.password_error = Some("Invalid password".to_string());
                self.password.clear();
                self.needs_redraw = true;
            }
            Err(e) => {
                self.password_error = Some(format!("Error: {}", e));
                self.needs_redraw = true;
            }
        }
    }

    fn draw(&mut self, qh: &QueueHandle<Self>, layer: &LayerSurface) {
        // Recreate pool if needed (e.g., after resize)
        if self.pool.is_none() {
            self.pool = Some(
                SlotPool::new(self.width as usize * self.height as usize * 4, &self.shm)
                    .expect("Failed to create pool"),
            );
        }

        let pool = self.pool.as_mut().unwrap();
        let stride = self.width as i32 * 4;
        let (buffer, canvas) = pool
            .create_buffer(
                self.width as i32,
                self.height as i32,
                stride,
                wl_shm::Format::Argb8888,
            )
            .expect("Failed to create buffer");

        let width = self.width as usize;
        let height = self.height as usize;

        // Fill with background color
        let bg = self.colors.background;
        for pixel in canvas.chunks_exact_mut(4) {
            pixel[0] = bg.2; // B
            pixel[1] = bg.1; // G
            pixel[2] = bg.0; // R
            pixel[3] = 255; // A
        }

        let flash_duration = Duration::from_millis(self.flash_duration_ms);

        match self.mode {
            Mode::Password => {
                // Check if we're within the flash window
                let flash_active = self.last_keypress
                    .map(|t| t.elapsed() < flash_duration)
                    .unwrap_or(false);

                draw_password_mode(
                    &self.font,
                    canvas,
                    width,
                    height,
                    &self.colors,
                    self.password.is_empty(),
                    self.password_error.as_deref(),
                    flash_active,
                    self.font_size,
                    self.hints_font_size,
                );

                // Keep redrawing while flash is active
                if flash_active {
                    self.needs_redraw = true;
                }
            }
            Mode::Picker => draw_picker_mode(
                &self.font,
                canvas,
                width,
                height,
                &self.colors,
                &self.query,
                &self.entries,
                self.selected_index,
                self.font_size,
                self.hints_font_size,
                self.max_entries,
            ),
        }

        layer.wl_surface().attach(Some(buffer.wl_buffer()), 0, 0);
        layer
            .wl_surface()
            .damage_buffer(0, 0, self.width as i32, self.height as i32);

        // Request frame callback if we need to keep animating
        if self.needs_redraw {
            layer.wl_surface().frame(qh, layer.wl_surface().clone());
            self.frame_pending = true;
        }

        layer.wl_surface().commit();
    }
}

fn draw_password_mode(
    font: &Font,
    canvas: &mut [u8],
    width: usize,
    height: usize,
    colors: &ColorSchemeRgb,
    password_is_empty: bool,
    password_error: Option<&str>,
    flash: bool,
    font_size: f32,
    hints_font_size: f32,
) {
    const PADDING: usize = 16;
    const INPUT_HEIGHT: usize = 40;
    const LABEL_HEIGHT: usize = 32;

    // Center the password prompt vertically
    let total_height = LABEL_HEIGHT + PADDING + INPUT_HEIGHT;
    let start_y = (height - total_height) / 2;

    // Draw "Master Password:" label
    draw_text_centered(
        font,
        canvas,
        width,
        height,
        "Master Password:",
        PADDING + 8,
        start_y,
        LABEL_HEIGHT,
        font_size,
        colors.foreground,
    );

    // Draw input box background
    let input_y = start_y + LABEL_HEIGHT + PADDING / 2;
    let bg_light = colors.background_light;
    fill_rect(
        canvas,
        width,
        PADDING,
        input_y,
        width - 2 * PADDING,
        INPUT_HEIGHT,
        bg_light.0,
        bg_light.1,
        bg_light.2,
    );

    // Draw placeholder when empty
    if password_is_empty {
        draw_text_centered(
            font,
            canvas,
            width,
            height,
            "Enter password...",
            PADDING + 8,
            input_y,
            INPUT_HEIGHT,
            font_size,
            colors.foreground_subtle,
        );
    }

    // Draw flash indicator separately (small dot on the right side of input box)
    if flash && !password_is_empty {
        let dot_size = 8usize;
        let dot_x = width - PADDING - 16 - dot_size;
        let dot_y = input_y + (INPUT_HEIGHT - dot_size) / 2;
        fill_rect(
            canvas,
            width,
            dot_x,
            dot_y,
            dot_size,
            dot_size,
            colors.foreground_bright.0,
            colors.foreground_bright.1,
            colors.foreground_bright.2,
        );
    }

    // Draw error message if present
    if let Some(error) = password_error {
        let error_y = input_y + INPUT_HEIGHT + PADDING;
        draw_text_centered(
            font,
            canvas,
            width,
            height,
            error,
            PADDING + 8,
            error_y,
            LABEL_HEIGHT,
            font_size,
            colors.error,
        );
    }

    // Draw keyboard hints at bottom
    const HINTS_HEIGHT: usize = 20;
    let hints = "Enter: unlock | Esc: cancel";
    let hints_y = height - PADDING - HINTS_HEIGHT;
    draw_text_centered(
        font,
        canvas,
        width,
        height,
        hints,
        PADDING,
        hints_y,
        HINTS_HEIGHT,
        hints_font_size,
        colors.foreground_subtle,
    );
}

fn draw_picker_mode(
    font: &Font,
    canvas: &mut [u8],
    width: usize,
    height: usize,
    colors: &ColorSchemeRgb,
    query: &str,
    entries: &[(String, String)],
    selected_index: usize,
    font_size: f32,
    hints_font_size: f32,
    max_entries: usize,
) {
    // Constants for layout
    const PADDING: usize = 16;
    const INPUT_HEIGHT: usize = 40;
    const ENTRY_HEIGHT: usize = 32;

    // Draw input box background
    let bg_light = colors.background_light;
    fill_rect(
        canvas,
        width,
        PADDING,
        PADDING,
        width - 2 * PADDING,
        INPUT_HEIGHT,
        bg_light.0,
        bg_light.1,
        bg_light.2,
    );

    // Draw query text in input box (vertically centered)
    let query_display = if query.is_empty() {
        "Type to search..."
    } else {
        query
    };
    let text_color = if query.is_empty() {
        colors.foreground_subtle
    } else {
        colors.foreground_bright
    };
    draw_text_centered(
        font,
        canvas,
        width,
        height,
        query_display,
        PADDING + 8,
        PADDING,
        INPUT_HEIGHT,
        font_size,
        text_color,
    );

    // Draw entries list
    let entries_start_y = PADDING + INPUT_HEIGHT + PADDING;
    let max_visible = ((height - entries_start_y - 40) / ENTRY_HEIGHT).min(max_entries);

    if entries.is_empty() && !query.is_empty() {
        // Show "No matches" when search returns no results
        draw_text_centered(
            font,
            canvas,
            width,
            height,
            "No matches",
            PADDING + 8,
            entries_start_y,
            ENTRY_HEIGHT,
            font_size,
            colors.foreground_subtle,
        );
    } else {
        for (i, (name, login)) in entries.iter().take(max_visible).enumerate() {
            let y = entries_start_y + i * ENTRY_HEIGHT;

            // Highlight selected entry
            if i == selected_index {
                let sel = colors.selection;
                fill_rect(
                    canvas,
                    width,
                    PADDING,
                    y,
                    width - 2 * PADDING,
                    ENTRY_HEIGHT,
                    sel.0,
                    sel.1,
                    sel.2,
                );
            }

            // Format entry as "name - login" (vertically centered)
            let entry_text = format!("{} - {}", name, login);
            let color = if i == selected_index {
                colors.foreground_bright
            } else {
                colors.foreground
            };
            draw_text_centered(
                font,
                canvas,
                width,
                height,
                &entry_text,
                PADDING + 8,
                y,
                ENTRY_HEIGHT,
                font_size,
                color,
            );
        }
    }

    // Draw keyboard hints at bottom
    const HINTS_HEIGHT: usize = 20;
    let hints = "Enter: select | Esc: cancel | Up/Down: navigate";
    let hints_y = height - PADDING - HINTS_HEIGHT;
    draw_text_centered(
        font,
        canvas,
        width,
        height,
        hints,
        PADDING,
        hints_y,
        HINTS_HEIGHT,
        hints_font_size,
        colors.foreground_subtle,
    );
}

delegate_compositor!(AppState);
delegate_output!(AppState);
delegate_shm!(AppState);
delegate_seat!(AppState);
delegate_keyboard!(AppState);
delegate_pointer!(AppState);
delegate_layer!(AppState);
delegate_registry!(AppState);
