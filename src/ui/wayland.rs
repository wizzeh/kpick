use fontdue::{Font, FontSettings};
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
    Connection, QueueHandle,
};

/// Attempts to load a system font, trying common paths
fn load_system_font() -> Font {
    // Common font paths on Linux/NixOS
    let font_paths = [
        "/usr/share/fonts/DejaVuSans.ttf",
        "/usr/share/fonts/LiberationSans-Regular.ttf",
        "/run/current-system/sw/share/X11/fonts/DejaVuSans.ttf",
        "/run/current-system/sw/share/X11/fonts/LiberationSans-Regular.ttf",
    ];

    for path in &font_paths {
        if let Ok(data) = std::fs::read(path) {
            if let Ok(font) = Font::from_bytes(data, FontSettings::default()) {
                return font;
            }
        }
    }

    // Fallback: try to find any TTF font
    if let Ok(entries) = std::fs::read_dir("/usr/share/fonts") {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map_or(false, |ext| ext == "ttf") {
                if let Ok(data) = std::fs::read(&path) {
                    if let Ok(font) = Font::from_bytes(data, FontSettings::default()) {
                        return font;
                    }
                }
            }
        }
    }

    panic!("No system font found. Please install DejaVu Sans or Liberation Sans.");
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

    layer_shell: LayerShell,
    layer_surface: Option<LayerSurface>,
    keyboard: Option<wl_keyboard::WlKeyboard>,
    pointer: Option<wl_pointer::WlPointer>,

    // Font for text rendering
    font: Font,

    // Input state
    pub query: String,
    pub selected_index: usize,

    // Entries to display: (name, login) pairs
    pub entries: Vec<(String, String)>,

    // Callbacks for when user makes a selection
    pub on_select: Option<Box<dyn FnMut(usize)>>,
    pub on_escape: Option<Box<dyn FnMut()>>,
    pub on_query_change: Option<Box<dyn FnMut(&str)>>,
}

impl AppState {
    pub fn new(conn: &Connection, qh: &QueueHandle<Self>) -> Self {
        let (globals, _event_queue) = registry_queue_init::<Self>(conn).unwrap();
        let registry_state = RegistryState::new(&globals);

        let shm = Shm::bind(&globals, qh).expect("wl_shm not available");
        let compositor = CompositorState::bind(&globals, qh).expect("wl_compositor not available");
        let layer_shell = LayerShell::bind(&globals, qh).expect("layer shell not available");
        let seat_state = SeatState::new(&globals, qh);
        let output_state = OutputState::new(&globals, qh);

        // Load our font for text rendering
        let font = load_system_font();

        // Create our surface
        let surface = compositor.create_surface(qh);

        // Create layer surface
        let layer_surface = layer_shell.create_layer_surface(
            qh,
            surface,
            Layer::Overlay,
            Some("kpick"),
            None,
        );

        // Configure layer surface
        layer_surface.set_anchor(Anchor::TOP);
        layer_surface.set_size(600, 400);
        layer_surface.set_keyboard_interactivity(KeyboardInteractivity::Exclusive);
        layer_surface.set_margin(100, 0, 0, 0); // 100px from top
        layer_surface.commit();

        Self {
            running: true,
            width: 600,
            height: 400,
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
            query: String::new(),
            selected_index: 0,
            entries: Vec::new(),
            on_select: None,
            on_escape: None,
            on_query_change: None,
        }
    }

    /// Update the list of entries to display
    pub fn set_entries(&mut self, entries: Vec<(String, String)>) {
        self.entries = entries;
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
    }

    fn surface_enter(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
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

        // Initial draw
        self.draw(qh, layer);
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
        match event.keysym {
            Keysym::Escape => {
                if let Some(ref mut cb) = self.on_escape {
                    cb();
                }
                self.running = false;
            }
            Keysym::Return | Keysym::KP_Enter => {
                if let Some(ref mut cb) = self.on_select {
                    cb(self.selected_index);
                }
                self.running = false;
            }
            Keysym::Up => {
                if self.selected_index > 0 {
                    self.selected_index -= 1;
                }
            }
            Keysym::Down => {
                self.selected_index += 1;
            }
            Keysym::BackSpace => {
                self.query.pop();
                self.selected_index = 0;
                if let Some(ref mut cb) = self.on_query_change {
                    cb(&self.query);
                }
            }
            _ => {
                // Handle text input
                if let Some(c) = event.utf8.as_ref().and_then(|s| s.chars().next()) {
                    if c.is_ascii_graphic() || c == ' ' {
                        self.query.push(c);
                        self.selected_index = 0;
                        if let Some(ref mut cb) = self.on_query_change {
                            cb(&self.query);
                        }
                    }
                }
            }
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
        _modifiers: Modifiers,
        _layout: u32,
    ) {
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

impl AppState {
    fn draw(&mut self, _qh: &QueueHandle<Self>, layer: &LayerSurface) {
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

        // Fill with dark background
        for pixel in canvas.chunks_exact_mut(4) {
            pixel[0] = 40;  // B
            pixel[1] = 40;  // G
            pixel[2] = 40;  // R
            pixel[3] = 255; // A
        }

        // Constants for layout
        const FONT_SIZE: f32 = 18.0;
        const PADDING: usize = 16;
        const INPUT_HEIGHT: usize = 40;
        const ENTRY_HEIGHT: usize = 32;

        // Draw input box background (slightly lighter)
        self.fill_rect(canvas, width, PADDING, PADDING, width - 2 * PADDING, INPUT_HEIGHT, 60, 60, 60);

        // Draw query text in input box
        let query_display = if self.query.is_empty() {
            "Type to search..."
        } else {
            &self.query
        };
        let text_color = if self.query.is_empty() {
            (120, 120, 120) // Gray for placeholder
        } else {
            (255, 255, 255) // White for typed text
        };
        self.draw_text(canvas, width, height, query_display, PADDING + 8, PADDING + 12, FONT_SIZE, text_color);

        // Draw entries list
        let entries_start_y = PADDING + INPUT_HEIGHT + PADDING;
        let max_visible = ((height - entries_start_y - 40) / ENTRY_HEIGHT).min(10);

        for (i, (name, login)) in self.entries.iter().take(max_visible).enumerate() {
            let y = entries_start_y + i * ENTRY_HEIGHT;

            // Highlight selected entry
            if i == self.selected_index {
                self.fill_rect(canvas, width, PADDING, y, width - 2 * PADDING, ENTRY_HEIGHT - 4, 80, 120, 180);
            }

            // Format entry as "name - login"
            let entry_text = format!("{} - {}", name, login);
            let color = if i == self.selected_index {
                (255, 255, 255)
            } else {
                (200, 200, 200)
            };
            self.draw_text(canvas, width, height, &entry_text, PADDING + 8, y + 6, FONT_SIZE, color);
        }

        // Draw keyboard hints at bottom
        let hints = "Enter: select | Esc: cancel | Up/Down: navigate";
        let hints_y = height - PADDING - 16;
        self.draw_text(canvas, width, height, hints, PADDING, hints_y, 14.0, (100, 100, 100));

        layer.wl_surface().attach(Some(buffer.wl_buffer()), 0, 0);
        layer.wl_surface().damage_buffer(0, 0, self.width as i32, self.height as i32);
        layer.wl_surface().commit();
    }

    /// Fill a rectangle with a solid color
    fn fill_rect(&self, canvas: &mut [u8], stride: usize, x: usize, y: usize, w: usize, h: usize, r: u8, g: u8, b: u8) {
        for row in y..(y + h) {
            for col in x..(x + w) {
                let idx = (row * stride + col) * 4;
                if idx + 3 < canvas.len() {
                    canvas[idx] = b;     // B
                    canvas[idx + 1] = g; // G
                    canvas[idx + 2] = r; // R
                    canvas[idx + 3] = 255; // A
                }
            }
        }
    }

    /// Draw text at position using fontdue
    fn draw_text(&self, canvas: &mut [u8], stride: usize, canvas_height: usize, text: &str, x: usize, y: usize, size: f32, color: (u8, u8, u8)) {
        let mut cursor_x = x as i32;
        let (r, g, b) = color;

        for ch in text.chars() {
            let (metrics, bitmap) = self.font.rasterize(ch, size);

            // Calculate glyph position
            let glyph_x = cursor_x + metrics.xmin;
            let glyph_y = y as i32 + (size as i32 - metrics.height as i32 - metrics.ymin);

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
}

delegate_compositor!(AppState);
delegate_output!(AppState);
delegate_shm!(AppState);
delegate_seat!(AppState);
delegate_keyboard!(AppState);
delegate_pointer!(AppState);
delegate_layer!(AppState);
delegate_registry!(AppState);
