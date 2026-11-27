//! Warnings of values a player found nothing to write to, each once, under
//! the clip they are from.

use super::fade::Outgoing;
use super::player::AnimationPlayer;
use super::pose::{Pose, Sample};

/// Warns, once each, of what `player` found nothing to write to: under the
/// clip faded in when it animates that property, and otherwise under the
/// side faded out.
pub(super) fn report(
    player: &mut AnimationPlayer,
    fading: bool,
    incoming: &Pose<'_>,
    missed: &[&Sample<'_>],
) {
    let faded_in = |s: &Sample| !fading || incoming.get(&s.target, s.property).is_some();
    if let (true, Some(fade)) = (fading, player.fade.as_mut()) {
        if !fade.reported && missed.iter().any(|&s| !faded_in(s)) {
            fade.reported = true;
            let from = match &fade.from {
                Outgoing::Clip(out) => format!("animation {}", out.clip.as_str()),
                Outgoing::Synced { set, .. } => set.describe(),
                Outgoing::Pose(_) => "an interrupted fade".to_owned(),
            };
            log::warn!(
                "{from}, fading into {}: nothing to animate for {}",
                playing(player),
                list(missed.iter().filter(|&&s| !faded_in(s))),
            );
        }
    }
    if !missed.iter().any(|&s| faded_in(s)) {
        return;
    }
    let reported = match (&player.clip, &mut player.synced) {
        (Some(_), _) => player.reported == player.clip,
        (None, Some(set)) => std::mem::replace(&mut set.reported, true),
        (None, None) => true,
    };
    if reported {
        return;
    }
    if player.clip.is_some() {
        player.reported = player.clip.clone();
    }
    log::warn!(
        "{}: nothing to animate for {}",
        playing(player),
        list(missed.iter().filter(|&&s| faded_in(s))),
    );
}

/// What a player plays, by name, for a warning.
fn playing(player: &AnimationPlayer) -> String {
    match (&player.clip, &player.synced) {
        (Some(clip), _) => format!("animation {}", clip.as_str()),
        (None, Some(set)) => set.describe(),
        (None, None) => "nothing".to_owned(),
    }
}

/// Missed values, listed for a warning.
fn list<'s, 'a: 's>(missed: impl Iterator<Item = &'s &'s Sample<'a>>) -> String {
    let said: Vec<String> = missed
        .map(|s| format!("{:?} on {:?}", s.property, s.target))
        .collect();
    said.join(", ")
}
