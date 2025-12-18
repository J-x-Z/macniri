use std::num::NonZeroU32;
use glutin::config::{ConfigTemplateBuilder, GetGlConfig};
use glutin::context::{ContextAttributesBuilder, PossiblyCurrentContext};
use glutin::display::GetGlDisplay;
use glutin::prelude::*;
use glutin::surface::{Surface, SwapInterval, WindowSurface};
use glutin_winit::{DisplayBuilder, GlWindow};
use raw_window_handle::HasWindowHandle;
use winit::window::Window;

pub struct GlRenderer {
    pub window: Window,
    pub gl_context: PossiblyCurrentContext,
    pub gl_surface: Surface<WindowSurface>,
    pub width: u32,
    pub height: u32,
}

impl GlRenderer {
    pub fn new(event_loop: &winit::event_loop::EventLoop<()>, title: &str, width: u32, height: u32) -> Result<Self, String> {
        let template = ConfigTemplateBuilder::new()
            .with_alpha_size(8)
            .with_transparency(false);
            
        let window_attributes = Window::default_attributes()
            .with_title(title)
            .with_transparent(false)
            .with_visible(true) // Explicitly force visibility
            .with_inner_size(winit::dpi::LogicalSize::new(width as f64, height as f64));
            
        let display_builder = DisplayBuilder::new().with_window_attributes(Some(window_attributes));
        
        let (window, gl_config) = display_builder
            .build(event_loop, template, |configs| {
                configs
                    .reduce(|accum, config| {
                        if config.num_samples() > accum.num_samples() {
                            config
                        } else {
                            accum
                        }
                    })
                    .unwrap()
            })
            .map_err(|e| format!("Failed to build display: {:?}", e))?;
            
        let window = window.ok_or("No window created")?;
        let raw_window_handle = window.window_handle().map_err(|e| format!("Window handle error: {}", e))?.as_raw();
        let gl_display = gl_config.display();
        
        let context_attributes = ContextAttributesBuilder::new().build(Some(raw_window_handle));
        let not_current_context = unsafe {
            gl_display
                .create_context(&gl_config, &context_attributes)
                .map_err(|e| format!("Failed to create context: {:?}", e))?
        };
        
        let attrs = window.build_surface_attributes(Default::default()).expect("Failed to build surface attributes");
        let gl_surface = unsafe {
            gl_display
                .create_window_surface(&gl_config, &attrs)
            .unwrap()
        };
        
        let gl_context = not_current_context
            .make_current(&gl_surface)
            .map_err(|e| format!("Failed to make current: {:?}", e))?;

        // CORE PROFILE HACK: Generate and Bind a Dummy VAO.
        // Without this, glDrawArrays fails silently on macOS Core Profile (3.2+).
        unsafe {
             use smithay::backend::renderer::gles::ffi;
             let gl = ffi::Gles2::load_with(|s| {
                gl_display.get_proc_address(&std::ffi::CString::new(s).unwrap()) as *const _
             });
             let mut vao = 0;
             gl.GenVertexArrays(1, &mut vao);
             gl.BindVertexArray(vao);
             println!("DEBUG: Core Profile VAO Hack active. VAO: {}", vao);
        }

        if let Err(e) = gl_surface.set_swap_interval(&gl_context, SwapInterval::DontWait) {
            log::warn!("Error setting vsync: {:?}", e);
        }

        window.set_visible(true);
        // window.request_redraw(); // Optional, but usually Niri handles this.

        window.set_visible(true);
        window.set_cursor_visible(true); // User wants OS cursor visible
        println!("DEBUG: Forced cursor visibility to TRUE");
        window.focus_window();
        window.set_maximized(true);
        
        let size = window.inner_size();
        let pos = window.outer_position().unwrap_or(winit::dpi::PhysicalPosition::new(0, 0));
        println!("DEBUG: Window created at {:?} with size {:?}", pos, size);

        Ok(Self {
            window,
            gl_context,
            gl_surface,
            width: size.width,
            height: size.height,
        })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.width = width;
            self.height = height;
            self.gl_surface.resize(
                &self.gl_context,
                NonZeroU32::new(width).unwrap(),
                NonZeroU32::new(height).unwrap(),
            );
            // Viewport is handled by Smithay renderer during render pass
        }
    }

    pub fn make_current(&self) -> Result<(), String> {
        if !self.gl_context.is_current() {
            self.gl_context.make_current(&self.gl_surface)
                .map_err(|e| format!("Failed to make context current: {:?}", e))?;
        }
        Ok(())
    }

    pub fn swap_buffers(&self) -> Result<(), String> {
        // self.make_current()?; // Ensure current before swap?
        
        // Flush before swap to ensure commands aren't buffered
        // gl::Flush(); // We don't have direct access to gl here easily without importing. 
        // Smithay does the rendering, and we patched Smithay to Flush/Clear.
        
        self.gl_surface
            .swap_buffers(&self.gl_context)
            .map_err(|e| format!("Failed to swap buffers: {:?}", e))
    }
}