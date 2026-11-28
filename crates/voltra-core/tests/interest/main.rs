//! Interest management through `tick.net`: a headless server and headless
//! clients, each sent only what is near the entities it owns or the points
//! the game watches for it. Entering spawns, leaving despawns, the edge does
//! not flap, a hierarchy goes and comes whole, and what comes back carries
//! what changed while it was away.

mod entering;
mod joining;
mod returning;
mod rig;
