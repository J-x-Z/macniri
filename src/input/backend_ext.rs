#[cfg(target_os = "linux")]
use ::input as libinput;
#[cfg(target_os = "macos")]
use crate::input_shim as libinput;
use smithay::backend::input;
// use smithay::backend::winit::WinitVirtualDevice;
use smithay::output::Output;

use crate::niri::State;
use crate::protocols::virtual_pointer::VirtualPointer;

pub trait NiriInputBackend: input::InputBackend<Device = Self::NiriDevice> {
    type NiriDevice: NiriInputDevice;
}
impl<T: input::InputBackend> NiriInputBackend for T
where
    Self::Device: NiriInputDevice,
{
    type NiriDevice = Self::Device;
}

pub trait NiriInputDevice: input::Device {
    // FIXME: this should maybe be per-event, not per-device,
    // but it's not clear that this matters in practice?
    // it might be more obvious once we implement it for libinput
    fn output(&self, state: &State) -> Option<Output>;
}

impl NiriInputDevice for libinput::Device {
    fn output(&self, _state: &State) -> Option<Output> {
        // FIXME: Allow specifying the output per-device?
        None
    }
}

// impl NiriInputDevice for WinitVirtualDevice {
//     fn output(&self, _state: &State) -> Option<Output> {
//         None
//     }
// }

impl NiriInputDevice for VirtualPointer {
    fn output(&self, _: &State) -> Option<Output> {
        self.output().cloned()
    }
}
