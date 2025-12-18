#![allow(dead_code, unused_variables)]

// Re-export specific items from Smithay's input backend to match "input" crate structure where possible
// The "input" crate (libinput) structure is different from "smithay::backend::input".
// But we can re-use some types if we alias them, or define our own.

// Smithay's DeviceCapability is compatible with what we need (enum).
pub use smithay::backend::input::DeviceCapability;

// Re-export other types that might be used
pub use smithay::backend::input::{
    ButtonState, KeyState, Axis, AxisSource,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SendEventsMode {
    DISABLED,
    DISABLED_ON_EXTERNAL_MOUSE,
    ENABLED,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccelProfile {
    Flat,
    Adaptive,
}

impl From<niri_config::input::AccelProfile> for AccelProfile {
    fn from(_: niri_config::input::AccelProfile) -> Self {
        Self::Adaptive
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollMethod {
    NoScroll,
    TwoFinger,
    Edge,
    OnButtonDown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollButtonLockState {
    Enabled,
    Disabled,
}

impl From<niri_config::input::ScrollMethod> for ScrollMethod {
    fn from(m: niri_config::input::ScrollMethod) -> Self {
        match m {
            niri_config::input::ScrollMethod::NoScroll => Self::NoScroll,
            niri_config::input::ScrollMethod::TwoFinger => Self::TwoFinger,
            niri_config::input::ScrollMethod::Edge => Self::Edge,
            niri_config::input::ScrollMethod::OnButtonDown => Self::OnButtonDown,
        }
    }
}

use smithay::input::keyboard::LedState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClickMethod {
    NoClick,
    ButtonAreas,
    Clickfinger,
}

impl From<niri_config::input::ClickMethod> for ClickMethod {
    fn from(m: niri_config::input::ClickMethod) -> Self {
        match m {
            // niri_config::input::ClickMethod::None => ClickMethod::NoClick,
            niri_config::input::ClickMethod::ButtonAreas => ClickMethod::ButtonAreas,
            niri_config::input::ClickMethod::Clickfinger => ClickMethod::Clickfinger,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TapButtonMap {
    Lrm_,
    Lmr_,
}

impl From<niri_config::input::TapButtonMap> for TapButtonMap {
    fn from(m: niri_config::input::TapButtonMap) -> Self {
        match m {
            // niri_config::input::TapButtonMap::Lrm => TapButtonMap::Lrm_,
            // niri_config::input::TapButtonMap::Lmr => TapButtonMap::Lmr_,
            _ => TapButtonMap::Lrm_, // Keep wildcard here if Lrm/Lmr missing
        }
    }
}

// Mock Device struct
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Device;

// Implement methods used by niri configuration
impl Device {
    pub fn config_tap_finger_count(&self) -> u32 { 0 }
    pub fn config_send_events_set_mode(&mut self, _mode: SendEventsMode) -> Result<(), ()> { Ok(()) }
    pub fn config_tap_set_enabled(&mut self, _enable: bool) -> Result<(), ()> { Ok(()) }
    pub fn config_dwt_set_enabled(&mut self, _enable: bool) -> Result<(), ()> { Ok(()) }
    pub fn config_dwtp_set_enabled(&mut self, _enable: bool) -> Result<(), ()> { Ok(()) }
    pub fn config_tap_set_drag_lock_enabled(&mut self, _enable: bool) -> Result<(), ()> { Ok(()) }
    pub fn config_scroll_set_natural_scroll_enabled(&mut self, _enable: bool) -> Result<(), ()> { Ok(()) }
    pub fn config_accel_set_speed(&mut self, _speed: f64) -> Result<(), ()> { Ok(()) }
    pub fn config_left_handed_set(&mut self, _left: bool) -> Result<(), ()> { Ok(()) }
    pub fn config_middle_emulation_set_enabled(&mut self, _enable: bool) -> Result<(), ()> { Ok(()) }
    pub fn config_tap_set_drag_enabled(&mut self, _enable: bool) -> Result<(), ()> { Ok(()) }
    pub fn config_tap_default_drag_enabled(&self) -> bool { true }
    pub fn config_accel_set_profile(&mut self, _profile: AccelProfile) -> Result<(), ()> { Ok(()) }
    pub fn config_accel_default_profile(&self) -> Option<AccelProfile> { None }
    pub fn config_scroll_set_method(&mut self, _method: ScrollMethod) -> Result<(), ()> { Ok(()) }
    pub fn config_scroll_default_method(&self) -> Option<ScrollMethod> { Some(ScrollMethod::TwoFinger) }
    pub fn config_scroll_set_button(&mut self, _button: u32) -> Result<(), ()> { Ok(()) }
    pub fn config_scroll_set_button_lock(&mut self, _state: ScrollButtonLockState) -> Result<(), ()> { Ok(()) }
    pub fn config_calibration_set_matrix(&mut self, _matrix: [f32; 6]) -> Result<(), ()> { Ok(()) }
    pub fn config_calibration_default_matrix(&self) -> Option<[f32; 6]> { Some([1.0, 0.0, 0.0, 0.0, 1.0, 0.0]) }
    
    // New methods from last check
    pub fn config_scroll_natural_scroll_enabled(&self) -> bool { false }
    pub fn config_tap_set_button_map(&mut self, _map: TapButtonMap) -> Result<(), ()> { Ok(()) }
    pub fn config_tap_default_button_map(&self) -> Option<TapButtonMap> { Some(TapButtonMap::Lrm_) }
    pub fn config_click_set_method(&mut self, _method: ClickMethod) -> Result<(), ()> { Ok(()) }
    pub fn config_click_default_method(&self) -> Option<ClickMethod> { Some(ClickMethod::ButtonAreas) }
    
    // Unsafe udev_device shim
    pub unsafe fn udev_device(&self) -> Option<()> { None }

    pub fn led_update(&mut self, _led_state: LedState) {}
    
    // Capability check shim
    pub fn has_capability(&self, _cap: DeviceCapability) -> bool { false }
    
    // Smithay Device trait implementation methods stubs
    pub fn id(&self) -> String { "macos-stub".into() }
    pub fn name(&self) -> String { "macOS Stub Device".into() }
    pub fn usb_id(&self) -> Option<(u32, u32)> { None }
    pub fn syspath(&self) -> Option<std::path::PathBuf> { None }
}

// Ideally we implement smithay::backend::input::Device for our Device struct
// so it can be passed where impl Device is expected.
impl smithay::backend::input::Device for Device {
    fn id(&self) -> String { self.id() }
    fn name(&self) -> String { self.name() }
    fn has_capability(&self, cap: DeviceCapability) -> bool { self.has_capability(cap) }
    fn usb_id(&self) -> Option<(u32, u32)> { self.usb_id() }
    fn syspath(&self) -> Option<std::path::PathBuf> { self.syspath() }
}

pub mod event {
    pub mod gesture {
        pub trait GestureEventCoordinates {
            fn dx(&self) -> f64 { 0.0 }
            fn dy(&self) -> f64 { 0.0 }
            fn dx_unaccelerated(&self) -> f64 { 0.0 }
            fn dy_unaccelerated(&self) -> f64 { 0.0 }
        }
        impl<T> GestureEventCoordinates for T {} 
        
        // Mock event structs required for downcasting
        pub struct GestureSwipeBeginEvent;
        pub struct GestureSwipeUpdateEvent;
        pub struct GestureSwipeEndEvent;
        pub struct GesturePinchBeginEvent;
        pub struct GesturePinchUpdateEvent;
        pub struct GesturePinchEndEvent;
        pub struct GestureHoldBeginEvent;
        pub struct GestureHoldEndEvent;
    }
}
