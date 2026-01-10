// Output formatting for parse results.

use crate::types::*;

/// Convert a hypothesis to bracketed {word:pos} component format.
///
/// Format:
///   Prefixes: `{bI-:v}` (trailing dash)
///   Stems:    `{Hegh:v}` (no dash)
///   Suffixes: `{-jaj:v}` (leading dash)
///   Homophones: `{boQ:n:1}` (index appended)
pub fn to_bracketed(h: &Hypothesis) -> String {
    h.components
        .iter()
        .map(|c| {
            let name = &c.entry_name;
            let pos = &c.pos;
            format!("{{{name}:{pos}}}")
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// Format a full sentence parse in bracketed format (top hypothesis per word).
pub fn sentence_to_bracketed(parse: &SentenceParse) -> String {
    parse
        .words
        .iter()
        .map(|wp| {
            wp.hypotheses
                .first()
                .map(|h| to_bracketed(h))
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bracketed_simple() {
        let h = Hypothesis {
            components: vec![
                Component {
                    text: "bI".to_string(),
                    entry_name: "bI-".to_string(),
                    pos: "v".to_string(),
                    homophone: None,
                    role: MorphemeRole::Prefix,
                    sub_components: vec![],
                },
                Component {
                    text: "Hegh".to_string(),
                    entry_name: "Hegh".to_string(),
                    pos: "v".to_string(),
                    homophone: None,
                    role: MorphemeRole::Stem,
                    sub_components: vec![],
                },
                Component {
                    text: "jaj".to_string(),
                    entry_name: "-jaj".to_string(),
                    pos: "v".to_string(),
                    homophone: None,
                    role: MorphemeRole::Suffix,
                    sub_components: vec![],
                },
            ],
            confidence: 80.0,
            parse_type: ParseType::Verb,
            warnings: vec![],
        };
        assert_eq!(to_bracketed(&h), "{bI-:v}, {Hegh:v}, {-jaj:v}");
    }

    #[test]
    fn test_bracketed_with_homophone() {
        let h = Hypothesis {
            components: vec![Component {
                text: "boQ".to_string(),
                entry_name: "boQ".to_string(),
                pos: "n:1".to_string(),
                homophone: Some(1),
                role: MorphemeRole::Stem,
                sub_components: vec![],
            }],
            confidence: 80.0,
            parse_type: ParseType::WholeWord,
            warnings: vec![],
        };
        assert_eq!(to_bracketed(&h), "{boQ:n:1}");
    }
}
