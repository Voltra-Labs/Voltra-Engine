//! One physical control an action can be bound to.

use winit::event::MouseButton;
use winit::keyboard::KeyCode;

/// A key or a mouse button, as an input map file names it: `Key(Space)`,
/// `Mouse(Left)`.
///
/// Keys are *physical* — `KeyW` is the key where W is on a US layout, which
/// is A on an AZERTY one. That is what Unity, Godot and Unreal bind by
/// default too: WASD stays a cross under the left hand on any layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Binding {
    Key(KeyCode),
    Mouse(MouseButton),
}
