use anyhow::bail;
use smithay::reexports::calloop::LoopHandle;

use crate::niri::State;

use smithay::reexports::calloop::RegistrationToken;
use smithay::utils::{Size, Physical};
use std::time::Duration;

use crate::niri::CastTarget;

#[derive(Debug)]
pub struct DummyStream;

impl DummyStream {
    pub fn disconnect(&self) -> anyhow::Result<()> {
        Ok(())
    }
}

pub struct Cast {
    pub session_id: usize,
    pub stream_id: usize,
    pub target: CastTarget,
    pub dynamic_target: bool,
    pub last_frame_time: Duration,
    pub stream: DummyStream,
}

impl Cast {
    pub fn is_active(&self) -> bool {
        false
    }
    pub fn ensure_size(&self, _size: Size<i32, Physical>) -> anyhow::Result<CastSizeChange> {
        Ok(CastSizeChange::Ready)
    }
    pub fn set_refresh(&mut self, _refresh: u32) -> anyhow::Result<()> {
        Ok(())
    }
    pub fn check_time_and_schedule(&mut self, _output: &smithay::output::Output, _time: Duration) -> bool {
        false
    }
    pub fn dequeue_buffer_and_clear(&mut self, _renderer: &mut smithay::backend::renderer::gles::GlesRenderer) -> bool {
        false
    }
    // Simplified signature to avoid strict type matching hell, assuming it's used in method calls
    // But niri.rs calls it with arguments.
    // If I use generic arguments where possible?
    // GlesRenderer is specific. I need to import it.
    pub fn dequeue_buffer_and_render<R>(
        &mut self,
        _renderer: &mut R,
        _elements: &[impl smithay::backend::renderer::element::RenderElement<R>],
        _size: Size<i32, Physical>,
        _scale: smithay::utils::Scale<f64>,
    ) -> bool 
    where R: smithay::backend::renderer::Renderer
    {
        false
    }
}

pub struct PipeWire {
    pub token: RegistrationToken,
}

impl PipeWire {
    pub fn new(_event_loop: &LoopHandle<'static, State>, _to_niri: calloop::channel::Sender<PwToNiri>) -> anyhow::Result<Self> {
        bail!("PipeWire support is disabled (see \"xdp-gnome-screencast\" feature)");
    }
}

#[derive(Debug)]
pub enum PwToNiri {
    StopCast { session_id: usize },
    Redraw { stream_id: usize },
    FatalError,
}

#[derive(Debug, PartialEq, Eq)]
pub enum CastSizeChange {
    Ready,
    Pending,
}
