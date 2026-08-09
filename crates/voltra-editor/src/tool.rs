//! Which transform the next drag in the viewport performs.
//!
//! Unity's model rather than Blender's: the tool is persistent state, and the
//! gizmo on screen tells you what a drag will do before you start it. Blender
//! binds the transform to a gesture instead — `G`/`R`/`S` begin a modal
//! transform the mouse drives until a click confirms — which is faster once
//! learned and invisible until someone tells you the letter. It also needs
//! modal input capture that `Input` does not have: swallowing every key while
//! active, surviving a lost window focus, unwinding on `Esc`. That can be added
//! over a working gizmo; the reverse is harder. Godot 2D made the same call and
//! has a proposal open to add Blender's on top.

/// The active transform tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tool {
    /// Click to select, drag a handle to move. The only tool in stage 11a.
    ///
    /// One variant rather than a `bool`, so `Rotate` and `Scale` arrive as
    /// variants rather than as a rename of everything that reads this.
    #[default]
    Translate,
}
