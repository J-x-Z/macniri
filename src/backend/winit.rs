use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::ffi::CString;

use niri_config::{Config, OutputName};
use smithay::backend::allocator::dmabuf::Dmabuf;
use smithay::backend::renderer::damage::OutputDamageTracker;
use smithay::backend::renderer::{
    gles::GlesRenderer,
    Renderer,
    Bind,
};
use smithay::output::{Mode, Output, PhysicalProperties, Subpixel};

use calloop::{LoopHandle, RegistrationToken, EventSource, Interest, PostAction};
use calloop::ping::{Ping, PingSource, make_ping};
use calloop::channel::{Channel, Sender, channel};

use smithay::reexports::wayland_protocols::wp::presentation_time::server::wp_presentation_feedback;
use smithay::wayland::presentation::Refresh;

use winit::event::{Event, WindowEvent};
use winit::event_loop::{EventLoop, ControlFlow};
use winit::platform::pump_events::EventLoopExtPumpEvents;
use winit::platform::scancode::PhysicalKeyExtScancode; // Needed for scancode

use calloop::{Readiness, Token, TokenFactory};
use winit::window::Window;
use glutin::prelude::GlDisplay; 
use glutin::display::GetGlDisplay;
use glutin::context::GlContext;

use super::{IpcOutputMap, OutputId, RenderResult};
use crate::backend::Backend;
use crate::niri::{Niri, RedrawState, State};
use crate::render_helpers::debug::draw_damage;
use crate::render_helpers::{resources, shaders, RenderTarget};
use crate::utils::{get_monotonic_time, logical_output};
use crate::backend::cocoa_renderer::GlRenderer as CocoaWindowHandle;

// Serializable version of winit events that can be sent across threads
#[derive(Debug, Clone)]
pub enum WinitEventMsg {
    Resized(u32, u32),
    CloseRequested,
    RedrawRequested,
    Focused(bool),
    KeyboardInput { scancode: u32, pressed: bool },
    CursorMoved { x: f64, y: f64 },
    MouseButton { button: u32, pressed: bool },
    MouseWheel { delta_x: f64, delta_y: f64 },
    ScaleFactorChanged(f64),
    Occluded(bool),
    // Add more as needed
}

pub struct WinitEventSource {
    event_loop: EventLoop<()>,
    ping: PingSource,
}

impl WinitEventSource {
    pub fn new(event_loop: EventLoop<()>) -> (Self, Ping) {
        let (ping_sender, ping) = make_ping().unwrap();
        (Self { 
            event_loop,
            ping,
        }, ping_sender)
    }
}

impl EventSource for WinitEventSource {
    type Event = winit::event::Event<()>;
    type Metadata = ();
    type Ret = (); 
    type Error = winit::error::EventLoopError;

    fn process_events<F>(
        &mut self,
        readiness: Readiness,
        token: Token,
        mut callback: F,
    ) -> Result<PostAction, Self::Error>
    where
        F: FnMut(Self::Event, &mut Self::Metadata) -> Self::Ret,
    {
        // Process ping to clear the readiness
        let _ = self.ping.process_events(readiness, token, |_, _| {});

        let timeout = Some(Duration::ZERO);
        #[allow(deprecated)]
        self.event_loop.pump_events(timeout, |event, target| {
            
            callback(event, &mut ());
            target.set_control_flow(ControlFlow::Wait);
        });
        Ok(PostAction::Continue)
    }

    fn register(
        &mut self,
        poll: &mut calloop::Poll,
        token_factory: &mut TokenFactory,
    ) -> calloop::Result<()> {
        let _ = calloop::EventSource::register(&mut self.ping, poll, token_factory);
        Ok(())
    }

    fn reregister(
        &mut self,
        poll: &mut calloop::Poll,
        token_factory: &mut TokenFactory,
    ) -> calloop::Result<()> {
        let _ = calloop::EventSource::reregister(&mut self.ping, poll, token_factory);
        Ok(())
    }

    fn unregister(
        &mut self,
        poll: &mut calloop::Poll,
    ) -> calloop::Result<()> {
        self.ping.unregister(poll)
    }
}


pub struct Winit {
    config: Rc<RefCell<Config>>,
    output: Output,
    cocoa_window: CocoaWindowHandle,
    gles_renderer: GlesRenderer,
    damage_tracker: OutputDamageTracker,
    ipc_outputs: Arc<Mutex<IpcOutputMap>>,
    ping_sender: calloop::ping::Ping,
    last_modifiers: winit::keyboard::ModifiersState,
    // Debounce: Track last event time per scancode to filter buffered event bursts
    last_key_time: std::cell::RefCell<HashMap<u32, std::time::Instant>>,
}

impl Winit {
    pub fn new(
        config: Rc<RefCell<Config>>,
        event_loop: LoopHandle<State>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let _span = tracy_client::span!("Winit::new");

        use winit::platform::macos::{EventLoopBuilderExtMacOS, ActivationPolicy};

        let winit_loop = winit::event_loop::EventLoopBuilder::new()
            .with_activation_policy(ActivationPolicy::Regular)
            .build()?;

        // Force activation to ensure we get focus
        #[cfg(target_os = "macos")]
        unsafe {
            use objc::{msg_send, sel, sel_impl};
            use objc::runtime::{Class, Object};
            let ns_app: *mut Object = msg_send![Class::get("NSApplication").unwrap(), sharedApplication];
            let _: () = msg_send![ns_app, activateIgnoringOtherApps:true];
        }



        let cocoa_window = CocoaWindowHandle::new(&winit_loop, "niri (macOS)", 1600, 1000)
            .map_err(|e| format!("Failed to initialize Cocoa backend: {}", e))?;

        // Initialize GlesRenderer using the custom macOS constructor
        let display = cocoa_window.gl_context.display();
        let renderer = unsafe {
             GlesRenderer::new_with_loader(|s| {
                 let symbol = CString::new(s).unwrap();
                 display.get_proc_address(symbol.as_c_str()).cast()
             })?
        };

        println!("DEBUG: Initialized GlesRenderer on macOS!");

        let output = Output::new(
            "winit".to_string(),
            PhysicalProperties {
                size: (0, 0).into(),
                subpixel: Subpixel::Unknown,
                make: "Smithay".into(),
                model: "Winit".into(),
                serial_number: "Unknown".into(),
            },
        );

        // Access inner window size from cocoa_window
        let window_size = cocoa_window.width; // u32
        let height = cocoa_window.height;     // u32

        let mode = Mode {
            size: (window_size as i32, height as i32).into(),
            refresh: 60_000,
        };
        output.change_current_state(Some(mode), None, None, None);
        output.set_preferred(mode);

        output.user_data().insert_if_missing(|| OutputName {
            connector: "winit".to_string(),
            make: Some("Smithay".to_string()),
            model: Some("Winit".to_string()),
            serial: None,
        });

        let physical_properties = output.physical_properties();
        let ipc_outputs = Arc::new(Mutex::new(HashMap::from([(
            OutputId::next(),
            niri_ipc::Output {
                name: output.name(),
                make: physical_properties.make,
                model: physical_properties.model,
                serial: None,
                physical_size: None,
                modes: vec![niri_ipc::Mode {
                    width: window_size as u16,
                    height: height as u16,
                    refresh_rate: 60_000,
                    is_preferred: true,
                }],
                current_mode: Some(0),
                is_custom_mode: true,
                vrr_supported: false,
                vrr_enabled: false,
                logical: Some(logical_output(&output)),
            },
        )])));

        let damage_tracker = OutputDamageTracker::from_output(&output);

        use calloop::timer::{Timer, TimeoutAction};

        let (winit_source, ping_sender) = WinitEventSource::new(winit_loop);

        event_loop
            .insert_source(winit_source, move |event, _, state| {
                match &event {
                    Event::WindowEvent { event: w_event, .. } => {
                       match w_event {
                            WindowEvent::Resized(size) => {
                                if let Backend::Winit(winit) = &mut state.backend {
                                    winit.cocoa_window.resize(size.width, size.height);
                                }
                            }
                            WindowEvent::ScaleFactorChanged { .. } => {
                                // No-op or trigger resize
                            }
                            _ => {}
                       }
                    }
                    _ => {}
                };

               match event {
                   Event::WindowEvent { event, .. } => match event {
                       WindowEvent::Resized(size) => {
                           tracing::info!("Niri received WinitEvent::Resized: {:?}", size);
                           let winit = state.backend.winit();
                           winit.output.change_current_state(
                               Some(Mode {
                                   size: (size.width as i32, size.height as i32).into(),
                                   refresh: 60_000,
                               }),
                               None,
                               None,
                               None,
                            );
                            
                           winit.CocoaResize(size.width, size.height);

                           {
                               let mut ipc_outputs = winit.ipc_outputs.lock().unwrap();
                               let output = ipc_outputs.values_mut().next().unwrap();
                               let mode = &mut output.modes[0];
                               mode.width = size.width as u16;
                               mode.height = size.height as u16;
                                if let Some(logical) = output.logical.as_mut() {
                                   logical.width = size.width;
                                   logical.height = size.height;
                               }
                               state.niri.ipc_outputs_changed = true;
                           }

                           state.niri.output_resized(&winit.output);
                       }
                       WindowEvent::CloseRequested => state.niri.stop_signal.stop(),
                       WindowEvent::RedrawRequested => {
                           state.niri.queue_redraw(&state.backend.winit().output);
                       }
                        WindowEvent::ModifiersChanged(modifiers_event) => {
                            tracing::info!("Modifiers Changed: {:?}", modifiers_event);
                            
                            // Synthesize key events for modifiers (Winit 0.30/macOS swallows them)
                            let new_state = modifiers_event.state();
                            let old_state = state.backend.winit().last_modifiers;
                            
                            // If states differ, check each flag
                            if new_state != old_state {
                                use winit::keyboard::{ModifiersState, KeyCode, PhysicalKey};
                                use winit::event::ElementState;
                                
                                let mut check_mod = |mask: ModifiersState, code: KeyCode| {
                                    let was_on = old_state.contains(mask);
                                    let is_on = new_state.contains(mask);
                                    
                                    if was_on != is_on {
                                        let state_enum = if is_on { ElementState::Pressed } else { ElementState::Released };
                                        
                                        // Manual Scancode Map (Evdev + 8)
                                        // ShiftLeft(42)  -> 50
                                        // CtrlLeft(29)   -> 37
                                        // AltLeft(56)    -> 64
                                        // SuperLeft(125) -> 133
                                        
                                        let evdev = match code {
                                             KeyCode::ShiftLeft => 42,
                                             KeyCode::ControlLeft => 29, 
                                             KeyCode::AltLeft => 56, 
                                             KeyCode::SuperLeft => 125, 
                                             _ => 0
                                        };
                                        // WinitKeyboardInputEvent adds 8 automatically in implementation of KeyboardKeyEvent.
                                        // So we pass raw Evdev code here.
                                        let scancode = evdev;
                                        
                                        println!("Synthesizing Modifier Event: {:?} -> {} ({:?})", code, scancode, state_enum);
                                        
                                        use smithay::backend::input::InputEvent;
                                        use crate::backend::winit_input::{WinitInput, WinitKeyboardInputEvent};
                                        let time = get_monotonic_time().as_micros() as u64;
                                        
                                        let event = InputEvent::<WinitInput>::Keyboard {
                                             event: WinitKeyboardInputEvent {
                                                 time,
                                                 key: scancode,
                                                 count: 1, 
                                                 state: state_enum,
                                             },
                                         };
                                         state.process_input_event(event);
                                    }
                                };
                                
                                // Map Generic Flags to Left Keys (Good enough for most bindings)
                                check_mod(ModifiersState::SHIFT, KeyCode::ShiftLeft);
                                check_mod(ModifiersState::CONTROL, KeyCode::ControlLeft);
                                check_mod(ModifiersState::ALT, KeyCode::AltLeft);
                                check_mod(ModifiersState::SUPER, KeyCode::SuperLeft);
                                
                                state.backend.winit().last_modifiers = new_state;
                            }
                        }
                        // INPUT HANDLING MAPPING
                        WindowEvent::KeyboardInput { event, is_synthetic, .. } => {

                             // Filter out synthetic and repeat events - Smithay handles repeats internally.
                             // Not filtering these causes event queue flooding when keys are held.
                             if event.repeat {
                                 // Only skip repeats, not synthetic (synthetic are modifier state updates)
                                 return;
                             }

                             // DEBOUNCE: Skip buffered event bursts caused by CFRunLoop sleep
                             // When the loop sleeps for 16ms, events accumulate and arrive as bursts.
                             // This causes Press-Release-Press-Release sequences within 1ms.
                             // Debounce: Skip Press if same key was pressed < 50ms ago.
                             let scancode_for_debounce = match event.physical_key {
                                 PhysicalKey::Code(code) => code as u32,
                                 PhysicalKey::Unidentified(_) => 0,
                             };
                             if event.state == winit::event::ElementState::Pressed {
                                 let now = std::time::Instant::now();
                                 let mut last_times = state.backend.winit().last_key_time.borrow_mut();
                                 if let Some(last) = last_times.get(&scancode_for_debounce) {
                                     if now.duration_since(*last) < std::time::Duration::from_millis(50) {
                                         // tracing::info!("Debounce: Skipping rapid Press for scancode {}", scancode_for_debounce);
                                         return;
                                     }
                                 }
                                 last_times.insert(scancode_for_debounce, now);
                             }

                             use smithay::backend::input::InputEvent;
                             use crate::backend::winit_input::{WinitInput, WinitKeyboardInputEvent};
                             use winit::keyboard::{KeyCode, PhysicalKey};

                             let time = get_monotonic_time().as_micros() as u64;
                             
                            // Filter out real modifier key events to avoid stuck keys.
                            // Winit/macOS often swallows the Release events for modifiers, so we rely 
                            // entirely on the Synthetic events from `ModifiersChanged` to ensure valid Press/Release pairs.
                            match event.physical_key {
                                PhysicalKey::Code(KeyCode::ShiftLeft) |
                                PhysicalKey::Code(KeyCode::ShiftRight) |
                                PhysicalKey::Code(KeyCode::ControlLeft) |
                                PhysicalKey::Code(KeyCode::ControlRight) |
                                PhysicalKey::Code(KeyCode::AltLeft) |
                                PhysicalKey::Code(KeyCode::AltRight) |
                                PhysicalKey::Code(KeyCode::SuperLeft) |
                                PhysicalKey::Code(KeyCode::SuperRight) => {
                                    tracing::info!("Suppressing Real Modifier Event (using synthetic instead): {:?}", event.physical_key);
                                    return;
                                }
                                _ => {}
                            }

                             // Robust Mapping based on Cocoa-Way (KeyCode -> Evdev + 8)
                             // This bypasses macOS specific scancodes and uses Winit's unified PhysicalKey
                             let evdev_code = match event.physical_key {
                                 PhysicalKey::Code(code) => match code {
                                     KeyCode::Escape => Some(1),
                                     KeyCode::Digit1 => Some(2), KeyCode::Digit2 => Some(3), KeyCode::Digit3 => Some(4),
                                     KeyCode::Digit4 => Some(5), KeyCode::Digit5 => Some(6), KeyCode::Digit6 => Some(7),
                                     KeyCode::Digit7 => Some(8), KeyCode::Digit8 => Some(9), KeyCode::Digit9 => Some(10),
                                     KeyCode::Digit0 => Some(11), KeyCode::Minus => Some(12), KeyCode::Equal => Some(13),
                                     KeyCode::Backspace => Some(14), KeyCode::Tab => Some(15),
                                     KeyCode::KeyQ => Some(16), KeyCode::KeyW => Some(17), KeyCode::KeyE => Some(18),
                                     KeyCode::KeyR => Some(19), KeyCode::KeyT => Some(20), KeyCode::KeyY => Some(21),
                                     KeyCode::KeyU => Some(22), KeyCode::KeyI => Some(23), KeyCode::KeyO => Some(24),
                                     KeyCode::KeyP => Some(25), KeyCode::BracketLeft => Some(26), KeyCode::BracketRight => Some(27),
                                     KeyCode::Enter => Some(28), KeyCode::ControlLeft => Some(29),
                                     KeyCode::KeyA => Some(30), KeyCode::KeyS => Some(31), KeyCode::KeyD => Some(32),
                                     KeyCode::KeyF => Some(33), KeyCode::KeyG => Some(34), KeyCode::KeyH => Some(35),
                                     KeyCode::KeyJ => Some(36), KeyCode::KeyK => Some(37), KeyCode::KeyL => Some(38),
                                     KeyCode::Semicolon => Some(39), KeyCode::Quote => Some(40), KeyCode::Backquote => Some(41),
                                     KeyCode::ShiftLeft => Some(42), KeyCode::Backslash => Some(43),
                                     KeyCode::KeyZ => Some(44), KeyCode::KeyX => Some(45), KeyCode::KeyC => Some(46),
                                     KeyCode::KeyV => Some(47), KeyCode::KeyB => Some(48), KeyCode::KeyN => Some(49),
                                     KeyCode::KeyM => Some(50), KeyCode::Comma => Some(51), KeyCode::Period => Some(52),
                                     KeyCode::Slash => Some(53), KeyCode::ShiftRight => Some(54),
                                     KeyCode::AltLeft => Some(56), KeyCode::Space => Some(57), KeyCode::CapsLock => Some(58),
                                     KeyCode::F1 => Some(59), KeyCode::F2 => Some(60), KeyCode::F3 => Some(61), KeyCode::F4 => Some(62),
                                     KeyCode::F5 => Some(63), KeyCode::F6 => Some(64), KeyCode::F7 => Some(65), KeyCode::F8 => Some(66),
                                     KeyCode::F9 => Some(67), KeyCode::F10 => Some(68),
                                     KeyCode::AltRight => Some(100),
                                     KeyCode::ArrowUp => Some(103), KeyCode::ArrowLeft => Some(105),
                                     KeyCode::ArrowRight => Some(106), KeyCode::ArrowDown => Some(108),
                                     KeyCode::SuperLeft => Some(125), KeyCode::SuperRight => Some(126),
                                     _ => None,
                                 },
                                 _ => None,
                             };

                             let scancode = if let Some(code) = evdev_code {
                                 code // Raw Evdev (winit_input adds +8)
                             } else {
                                 // Fallback for unmapped keys (try raw + 8 heuristic? No, winit_input adds 8)
                                 // So we send raw macOS scancode?
                                 // If winit_input adds 8, and we send raw mac scancode, result is mac+8.
                                 // This is consistent with what we tried before, but maybe incorrect for unmapped keys.
                                 // However, for mapped keys, we MUST NOT add 8.
                                 event.physical_key.to_scancode().unwrap_or(0)
                             };
                             
                             println!("Key Input Debug [PRINTLN]: key={:?}, evdev={:?}, final_scancode={} (raw), state={:?}", 
                               event.physical_key, evdev_code, scancode, event.state);

                             tracing::info!("Key Input Debug: key={:?}, evdev={:?}, final_scancode={}, state={:?}", 
                                event.physical_key, evdev_code, scancode, event.state);
                             
                             let event = InputEvent::<WinitInput>::Keyboard {
                                 event: WinitKeyboardInputEvent {
                                     time,
                                     key: scancode,
                                     count: 1, 
                                     state: event.state,
                                 },
                             };
                             state.process_input_event(event);
                        }
                        WindowEvent::Focused(focused) => {
                            tracing::info!("Window Focus Changed: {}", focused);
                            // If we gain focus, ensure we are active
                            if focused {
                                // optional: force activation again?
                            }
                        }
                       WindowEvent::CursorMoved { position, .. } => {
                            use smithay::backend::input::InputEvent;
                            use crate::backend::winit_input::{WinitInput, WinitMouseMovedEvent, RelativePosition};
                            
                            let winit = state.backend.winit();
                            let size = winit.window().inner_size();
                            let x = position.x / size.width as f64;
                            let y = position.y / size.height as f64;
                            
                            let event = InputEvent::<WinitInput>::PointerMotionAbsolute {
                                event: WinitMouseMovedEvent {
                                    time: get_monotonic_time().as_micros() as u64,
                                    position: RelativePosition::new(x, y),
                                    global_position: position,
                                }
                            };
                            state.process_input_event(event);
                       }
                       WindowEvent::MouseInput { state: element_state, button, .. } => {
                            use smithay::backend::input::InputEvent;
                            use crate::backend::winit_input::{WinitInput, WinitMouseInputEvent};
                            
                            let event = InputEvent::<WinitInput>::PointerButton {
                                event: WinitMouseInputEvent {
                                    time: get_monotonic_time().as_micros() as u64,
                                    button,
                                    state: element_state,
                                    is_x11: false,
                                }
                            };
                            state.process_input_event(event);
                       }
                       WindowEvent::MouseWheel { delta, .. } => {
                            use smithay::backend::input::InputEvent;
                            use crate::backend::winit_input::{WinitInput, WinitMouseWheelEvent};
                            
                            let event = InputEvent::<WinitInput>::PointerAxis {
                                event: WinitMouseWheelEvent {
                                    time: get_monotonic_time().as_micros() as u64,
                                    delta,
                                }
                            };
                            state.process_input_event(event);
                       }
                       _ => (),
                   },
                   _ => (),
               }
            })
            .unwrap();

        Ok(Self {
            config,
            output,
            cocoa_window,
            gles_renderer: renderer,
            damage_tracker,
            ipc_outputs,
            ping_sender,
            last_modifiers: winit::keyboard::ModifiersState::empty(),
            last_key_time: std::cell::RefCell::new(HashMap::new()),
        })
    }

    pub fn pump(&self) {
        self.ping_sender.ping();
    }



    pub fn init(&mut self, niri: &mut Niri) {
        let renderer = &mut self.gles_renderer;
        resources::init(renderer);
        shaders::init(renderer);
        niri.update_shaders();
        niri.add_output(self.output.clone(), None, false);
    }

    pub fn seat_name(&self) -> String {
        "winit".to_owned()
    }

    pub fn with_primary_renderer<T>(
        &mut self,
        f: impl FnOnce(&mut GlesRenderer) -> T,
    ) -> Option<T> {
        Some(f(&mut self.gles_renderer))
    }

    pub fn render(&mut self, niri: &mut Niri, output: &Output) -> RenderResult {
        let _span = tracy_client::span!("Winit::render");
        
        // Bind renderer to the window size (framebuffer 0)
        let mut bind_size = (self.cocoa_window.width as i32, self.cocoa_window.height as i32);
        let mut target = self.gles_renderer.bind(&mut bind_size).expect("Failed to bind renderer");

        let mut elements = niri.render::<GlesRenderer>(
            &mut self.gles_renderer,
            output,
            true,
            RenderTarget::Output,
        );

        if niri.debug_draw_damage {
            let output_state = niri.output_state.get_mut(output).unwrap();
            draw_damage(&mut output_state.debug_damage_tracker, &mut elements);
        }

        let res = self.damage_tracker.render_output(
            &mut self.gles_renderer,
            &mut target,
            0,
            &elements,
            [0.1, 0.1, 0.1, 1.0], 
        );

        let render_result = match res {
             Ok(r) => r,
             Err(err) => {
                 tracing::warn!("Rendering failed: {:?}", err);
                 return RenderResult::Submitted;
             }
        };

        if let Err(e) = self.cocoa_window.make_current() {
             tracing::error!("Make current failed: {}", e);
        }

        if let Err(e) = self.cocoa_window.swap_buffers() {
             tracing::error!("Swap buffers failed: {}", e);
        }
        
         let mut presentation_feedbacks = niri.take_presentation_feedbacks(output, &render_result.states);
         presentation_feedbacks.presented::<_, smithay::utils::Monotonic>(
             get_monotonic_time(),
             Refresh::Unknown,
             0,
             wp_presentation_feedback::Kind::empty(),
         );

         // Crucial: Request the next frame to keep the event loop spinning at VSync.
         // Without this, the loop sleeps until external input, causing lag.
         self.cocoa_window.window.request_redraw();
         
         // FORCE CURSOR VISIBILITY (User Request)
         // Overrides any Smithay/Niri logic that hides it.
         self.cocoa_window.window.set_cursor_visible(true);
         self.cocoa_window.window.set_cursor_icon(winit::window::CursorIcon::Default);

        RenderResult::Submitted
    }
    
    pub fn toggle_debug_tint(&mut self) {}

    #[cfg(target_os = "linux")]
    pub fn import_dmabuf(&mut self, _dmabuf: &Dmabuf) -> bool { false }

    #[cfg(not(target_os = "linux"))]
    pub fn import_dmabuf(&mut self, _dmabuf: &Dmabuf) -> bool { false }

    pub fn ipc_outputs(&self) -> Arc<Mutex<IpcOutputMap>> {
        self.ipc_outputs.clone()
    }
    
    pub fn CocoaResize(&mut self, w: u32, h: u32) {
         self.cocoa_window.resize(w, h);
    }
    
    pub fn window(&self) -> &Window {
         &self.cocoa_window.window
    }
}

use crate::input::backend_ext::NiriInputDevice;
use crate::backend::winit_input::WinitVirtualDevice;

impl NiriInputDevice for WinitVirtualDevice {
    fn output(&self, _state: &State) -> Option<Output> {
        None
    }
}
