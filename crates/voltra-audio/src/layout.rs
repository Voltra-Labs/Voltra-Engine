//! A project's buses as a file describes them: names, volumes, effects.

use crate::effect::EffectDesc;

/// One bus in a project's bus layout.
///
/// What Godot's `default_bus_layout.tres` holds per bus and Unity's
/// `AudioMixer` per group: a name, a volume and the effects in order. The
/// engine reads a list of these from a project file and hands it to
/// [`Audio::apply_layout`](crate::Audio::apply_layout).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct BusSettings {
    /// The bus, by the name sources and ticks use.
    pub name: String,
    /// Its volume, linear.
    #[serde(default = "unity")]
    pub volume: f32,
    /// Its effects, first to last.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub effects: Vec<EffectDesc>,
}

fn unity() -> f32 {
    1.0
}

/// Whether `layout` is one the mixer can run, and why not: every bus named
/// once, every volume a number, every effect one that means something, and
/// no bus ducked by itself.
pub fn check_layout(layout: &[BusSettings]) -> Result<(), String> {
    let mut seen: Vec<String> = Vec::new();
    for bus in layout {
        let name = bus.name.trim().to_ascii_lowercase();
        if name.is_empty() {
            return Err("a bus with an empty name".to_owned());
        }
        if seen.contains(&name) {
            return Err(format!("the bus {name:?} is listed twice"));
        }
        if !bus.volume.is_finite() || bus.volume < 0.0 {
            return Err(format!("the bus {name:?} has volume {}", bus.volume));
        }
        for effect in &bus.effects {
            effect
                .check()
                .map_err(|e| format!("the bus {name:?}: {e}"))?;
            if let EffectDesc::Duck { by, .. } = effect {
                if by.trim().eq_ignore_ascii_case(&name) {
                    return Err(format!("the bus {name:?} is ducked by itself"));
                }
            }
        }
        seen.push(name);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bus(name: &str, effects: Vec<EffectDesc>) -> BusSettings {
        BusSettings {
            name: name.to_owned(),
            volume: 1.0,
            effects,
        }
    }

    fn duck(by: &str) -> EffectDesc {
        EffectDesc::Duck {
            by: by.to_owned(),
            depth: 0.5,
            threshold: 0.01,
            attack: 0.05,
            release: 0.2,
        }
    }

    #[test]
    fn a_plain_layout_checks() {
        let layout = [
            bus("music", vec![duck("dialogue")]),
            bus("dialogue", vec![]),
        ];
        assert!(check_layout(&layout).is_ok());
    }

    #[test]
    fn a_bus_twice_or_ducking_itself_is_refused() {
        assert!(check_layout(&[bus("sfx", vec![]), bus("SFX", vec![])])
            .expect_err("twice")
            .contains("twice"));
        assert!(check_layout(&[bus("music", vec![duck("Music")])])
            .expect_err("itself")
            .contains("itself"));
        assert!(check_layout(&[bus(" ", vec![])]).is_err());
    }
}
