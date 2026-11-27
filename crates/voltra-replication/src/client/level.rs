//! What a client keeps of its own copy of the level: the entities it set
//! aside without the server ever sending them, let go of with their level.

use voltra_scene::SceneId;

use super::ReplicationClient;

impl ReplicationClient {
    /// Lets go of the level entities set aside at the first complete
    /// snapshot that the server never sent: call it when the level they
    /// came from goes (a scene change, a region unloaded), or they would be
    /// kept for the session. What the server culled is kept — it knows.
    pub fn forget_unsent(&mut self) {
        self.stash.forget_unnumbered();
    }

    /// [`forget_unsent`](Self::forget_unsent), of the entities `ids` names
    /// alone: when one region of a streamed world unloads, the rest of the
    /// level — and its swept entities — stays.
    pub fn forget_unsent_of(&mut self, ids: &[SceneId]) {
        self.stash.forget_unnumbered_of(ids);
    }
}
