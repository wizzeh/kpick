use egui::{Context, RawInput, Rect, Vec2};
use tiny_skia::{Color, Pixmap};

pub struct EguiRenderer {
    ctx: Context,
    pixels_per_point: f32,
}

impl EguiRenderer {
    pub fn new() -> Self {
        Self {
            ctx: Context::default(),
            pixels_per_point: 1.0,
        }
    }

    pub fn render(
        &mut self,
        width: u32,
        height: u32,
        mut run_ui: impl FnMut(&Context),
    ) -> Pixmap {
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(
                Default::default(),
                Vec2::new(width as f32, height as f32) / self.pixels_per_point,
            )),
            ..Default::default()
        };

        let full_output = self.ctx.run(input, |ctx| {
            run_ui(ctx);
        });

        let clipped_primitives = self.ctx.tessellate(full_output.shapes, self.pixels_per_point);

        // Create pixmap and render
        let mut pixmap = Pixmap::new(width, height).unwrap();
        pixmap.fill(Color::from_rgba8(40, 40, 40, 255));

        // Use egui_skia or manual rendering here
        // For MVP, we'll do simple rectangle drawing
        for clipped in &clipped_primitives {
            // Basic rendering - full implementation would use epaint
            let _ = clipped;
        }

        pixmap
    }

    pub fn context(&self) -> &Context {
        &self.ctx
    }
}
