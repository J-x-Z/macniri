use std::path::PathBuf;

use winit::{
    dpi::PhysicalPosition,
    event::{ElementState, MouseButton as WinitMouseButton, MouseScrollDelta},
};

use smithay::backend::input::{
    self, AbsolutePositionEvent, Axis, AxisRelativeDirection, AxisSource, ButtonState, Device,
    DeviceCapability, Event, InputBackend, KeyState, KeyboardKeyEvent, Keycode, PointerAxisEvent,
    PointerButtonEvent, PointerMotionAbsoluteEvent, TouchCancelEvent, TouchDownEvent, TouchEvent,
    TouchMotionEvent, TouchSlot, TouchUpEvent, UnusedEvent,
};

/// Marker used to define the `InputBackend` types for the winit backend.
#[derive(Debug)]
pub struct WinitInput;

/// Virtual input device used by the backend to associate input events
#[derive(PartialEq, Eq, Hash, Debug)]
pub struct WinitVirtualDevice;

impl Device for WinitVirtualDevice {
    fn id(&self) -> String {
        String::from("winit")
    }

    fn name(&self) -> String {
        String::from("winit virtual input")
    }

    fn has_capability(&self, capability: DeviceCapability) -> bool {
        matches!(
            capability,
            DeviceCapability::Keyboard | DeviceCapability::Pointer | DeviceCapability::Touch
        )
    }

    fn usb_id(&self) -> Option<(u32, u32)> {
        None
    }

    fn syspath(&self) -> Option<PathBuf> {
        None
    }
}

/// Winit-Backend internal event wrapping `winit`'s types into a [`KeyboardKeyEvent`]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WinitKeyboardInputEvent {
    pub time: u64,
    pub key: u32,
    pub count: u32,
    pub state: ElementState,
}

impl Event<WinitInput> for WinitKeyboardInputEvent {
    fn time(&self) -> u64 {
        self.time
    }

    fn device(&self) -> WinitVirtualDevice {
        WinitVirtualDevice
    }
}

impl KeyboardKeyEvent<WinitInput> for WinitKeyboardInputEvent {
    fn key_code(&self) -> Keycode {
        (self.key + 8).into()
    }

    fn state(&self) -> KeyState {
        match self.state {
            ElementState::Pressed => KeyState::Pressed,
            ElementState::Released => KeyState::Released,
        }
    }

    fn count(&self) -> u32 {
        self.count
    }
}

/// Winit-Backend internal event wrapping `winit`'s types into a [`PointerMotionAbsoluteEvent`]
#[derive(Debug, Clone)]
pub struct WinitMouseMovedEvent {
    pub time: u64,
    pub position: RelativePosition,
    pub global_position: PhysicalPosition<f64>,
}

impl Event<WinitInput> for WinitMouseMovedEvent {
    fn time(&self) -> u64 {
        self.time
    }

    fn device(&self) -> WinitVirtualDevice {
        WinitVirtualDevice
    }
}

impl PointerMotionAbsoluteEvent<WinitInput> for WinitMouseMovedEvent {}
impl AbsolutePositionEvent<WinitInput> for WinitMouseMovedEvent {
    fn x(&self) -> f64 {
        self.global_position.x
    }

    fn y(&self) -> f64 {
        self.global_position.y
    }

    fn x_transformed(&self, width: i32) -> f64 {
        f64::max(self.position.x * width as f64, 0.0)
    }

    fn y_transformed(&self, height: i32) -> f64 {
        f64::max(self.position.y * height as f64, 0.0)
    }
}

/// Winit-Backend internal event wrapping `winit`'s types into a [`PointerAxisEvent`]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WinitMouseWheelEvent {
    pub time: u64,
    pub delta: MouseScrollDelta,
}

impl Event<WinitInput> for WinitMouseWheelEvent {
    fn time(&self) -> u64 {
        self.time
    }

    fn device(&self) -> WinitVirtualDevice {
        WinitVirtualDevice
    }
}

impl PointerAxisEvent<WinitInput> for WinitMouseWheelEvent {
    fn source(&self) -> AxisSource {
        match self.delta {
            MouseScrollDelta::LineDelta(_, _) => AxisSource::Wheel,
            MouseScrollDelta::PixelDelta(_) => AxisSource::Continuous,
        }
    }

    fn amount(&self, axis: Axis) -> Option<f64> {
        match (axis, self.delta) {
            (Axis::Horizontal, MouseScrollDelta::PixelDelta(delta)) => Some(-delta.x),
            (Axis::Vertical, MouseScrollDelta::PixelDelta(delta)) => Some(-delta.y),
            (_, MouseScrollDelta::LineDelta(_, _)) => None,
        }
    }

    fn amount_v120(&self, axis: Axis) -> Option<f64> {
        match (axis, self.delta) {
            (Axis::Horizontal, MouseScrollDelta::LineDelta(x, _)) => Some(-x as f64 * 120.),
            (Axis::Vertical, MouseScrollDelta::LineDelta(_, y)) => Some(-y as f64 * 120.),
            (_, MouseScrollDelta::PixelDelta(_)) => None,
        }
    }

    fn relative_direction(&self, _axis: Axis) -> AxisRelativeDirection {
        AxisRelativeDirection::Identical
    }
}

/// Winit-Backend internal event wrapping `winit`'s types into a [`PointerButtonEvent`]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WinitMouseInputEvent {
    pub time: u64,
    pub button: WinitMouseButton,
    pub state: ElementState,
    pub is_x11: bool,
}

impl Event<WinitInput> for WinitMouseInputEvent {
    fn time(&self) -> u64 {
        self.time
    }

    fn device(&self) -> WinitVirtualDevice {
        WinitVirtualDevice
    }
}

impl PointerButtonEvent<WinitInput> for WinitMouseInputEvent {
    fn button_code(&self) -> u32 {
        match self.button {
            WinitMouseButton::Left => 0x110,
            WinitMouseButton::Right => 0x111,
            WinitMouseButton::Middle => 0x112,
            WinitMouseButton::Forward => 0x115,
            WinitMouseButton::Back => 0x116,
            WinitMouseButton::Other(b) => b as u32,
        }
    }

    fn state(&self) -> ButtonState {
        match self.state {
            ElementState::Pressed => ButtonState::Pressed,
            ElementState::Released => ButtonState::Released,
        }
    }
}


/// Position relative to the source window
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RelativePosition {
    pub x: f64,
    pub y: f64,
}

impl RelativePosition {
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

impl InputBackend for WinitInput {
    type Device = WinitVirtualDevice;
    type KeyboardKeyEvent = WinitKeyboardInputEvent;
    type PointerAxisEvent = WinitMouseWheelEvent;
    type PointerButtonEvent = WinitMouseInputEvent;
    type PointerMotionEvent = UnusedEvent;
    type PointerMotionAbsoluteEvent = WinitMouseMovedEvent;

    type GestureSwipeBeginEvent = UnusedEvent;
    type GestureSwipeUpdateEvent = UnusedEvent;
    type GestureSwipeEndEvent = UnusedEvent;
    type GesturePinchBeginEvent = UnusedEvent;
    type GesturePinchUpdateEvent = UnusedEvent;
    type GesturePinchEndEvent = UnusedEvent;
    type GestureHoldBeginEvent = UnusedEvent;
    type GestureHoldEndEvent = UnusedEvent;

    type TouchDownEvent = UnusedEvent;
    type TouchUpEvent = UnusedEvent;
    type TouchMotionEvent = UnusedEvent;
    type TouchCancelEvent = UnusedEvent;
    type TouchFrameEvent = UnusedEvent;
    type TabletToolAxisEvent = UnusedEvent;
    type TabletToolProximityEvent = UnusedEvent;
    type TabletToolTipEvent = UnusedEvent;
    type TabletToolButtonEvent = UnusedEvent;

    type SwitchToggleEvent = UnusedEvent;
    type SpecialEvent = UnusedEvent;
}
