// CFRunLoop integration for macOS
// This module uses CFRunLoopTimer to wake up calloop periodically
// since we can't easily get the underlying kqueue fd from calloop.

use std::time::Duration;

use core_foundation::runloop::{
    CFRunLoop, kCFRunLoopDefaultMode, CFRunLoopRunInMode,
    kCFRunLoopRunTimedOut, kCFRunLoopRunHandledSource,
};

use calloop::EventLoop;
use crate::niri::State;

/// Run the event loop using CFRunLoop on macOS
/// This properly integrates calloop with the native macOS run loop by using
/// a polling approach where CFRunLoop handles the Cocoa events and we periodically
/// dispatch calloop.
pub fn run_with_cfrunloop(
    event_loop: &mut EventLoop<'static, State>,
    state: &mut State,
) -> Result<(), Box<dyn std::error::Error>> {
    tracing::info!("CFRunLoop integration initialized");
    
    // Safety: we are on macOS where objc is available.
    use objc::rc::autoreleasepool;
    
    let mut last_frame_time = std::time::Instant::now();
    let target_frame_time = Duration::from_micros(16666); // ~60 FPS (safe mode)

    loop {
        autoreleasepool(|| {
            // 1. Pump Winit events (Input) - Always run this to catch input instantly
            if let crate::backend::Backend::Winit(winit) = &mut state.backend {
                winit.pump();
            }

            // 2. Dispatch Wayland events
            if let Err(e) = event_loop.dispatch(Duration::ZERO, state) {
                tracing::error!("Calloop dispatch error: {:?}", e);
            }

            // 3. Render - Throttle to 60 FPS
            let now = std::time::Instant::now();
            let elapsed = now.duration_since(last_frame_time);

            if elapsed >= target_frame_time {
                // tracing::info!("tick - render");
                // tracing::info!("tick - render");
                state.refresh_and_flush_clients();
                last_frame_time = now;
            } else {
               // tracing::trace!("tick - skip render");
            }

            // 4. Smart Sleep
            // Calculate time until next *Render* frame
            let next_render_due = last_frame_time + target_frame_time;
            let sleep_duration = next_render_due.saturating_duration_since(std::time::Instant::now());

            // Render-throttled sleep (16ms)
            // We use standard blocking mode to prevent high CPU usage/leaks.
            // Latency is capped at ~16ms.
            unsafe {
                 CFRunLoopRunInMode(kCFRunLoopDefaultMode, sleep_duration.as_secs_f64(), false as u8);
            }
        });
    }

    Ok(())
}
