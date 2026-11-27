//! What the search boxes over the hierarchy and the asset browser type:
//! words a name must hold, and `t:` terms naming a type.
//!
//! Unity's syntax, which its Hierarchy and Project windows share: plain
//! words match the name, case aside, and `t:Sprite2D` matches by type — a
//! component in the hierarchy, a kind of file in the assets. Godot filters
//! by name only; Unity's `t:` is the one gesture that finds "every camera"
//! in a scene of a thousand entities, so it is the one copied. What a type
//! term means is each list's own business: see [`hierarchy`] and
//! [`assets`].

pub(crate) mod assets;
pub(crate) mod hierarchy;

/// A search, read from what was typed.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct Query {
    /// Words a name must hold, every one of them, lowercased.
    words: Vec<String>,
    /// What followed each `t:`, lowercased.
    types: Vec<String>,
}

impl Query {
    /// Reads `text`: words split on whitespace, `t:` (or `T:`) in front of
    /// a type. A bare `t:` names nothing and is dropped, so a box mid-typing
    /// does not empty the list.
    pub(crate) fn parse(text: &str) -> Self {
        let mut query = Self::default();
        for word in text.split_whitespace() {
            let lower = word.to_lowercase();
            match lower.strip_prefix("t:") {
                Some("") => {}
                Some(kind) => query.types.push(kind.to_owned()),
                None => query.words.push(lower),
            }
        }
        query
    }

    /// Whether it asks for nothing, and every row is shown.
    pub(crate) fn is_empty(&self) -> bool {
        self.words.is_empty() && self.types.is_empty()
    }

    /// Whether `name` holds every word, case aside. True with no words.
    pub(crate) fn name_matches(&self, name: &str) -> bool {
        let name = name.to_lowercase();
        self.words.iter().all(|word| name.contains(word.as_str()))
    }

    /// The `t:` terms, lowercased.
    pub(crate) fn types(&self) -> &[String] {
        &self.types
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_and_types_are_told_apart_and_case_does_not_matter() {
        let query = Query::parse("  Coin t:Sprite T:collider gold ");
        assert_eq!(query.types(), ["sprite", "collider"]);
        assert!(query.name_matches("Gold coin 3"));
        assert!(!query.name_matches("Coin"), "every word, not any");
        assert!(!query.is_empty());
    }

    #[test]
    fn nothing_typed_and_a_bare_t_ask_for_nothing() {
        assert!(Query::parse("").is_empty());
        assert!(Query::parse("  t: ").is_empty());
        assert!(Query::parse("").name_matches("anything"));
    }
}
