// Sentence-level parsing: split into words and parse each independently.

use crate::confidence;
use crate::dictionary::Dictionary;
use crate::morphology;
use crate::types::*;

/// Parse a Klingon sentence into morphological components.
/// Tries multi-word dictionary lookups before falling back to single-word parsing.
pub fn parse_sentence(input: &str, dict: &Dictionary) -> SentenceParse {
    let words = split_sentence(input);
    let mut word_parses = Vec::new();
    let mut i = 0;

    while i < words.len() {
        // Try matching 2- and 3-word compound entries.
        let mut matched = false;
        for window in (2..=3).rev() {
            if i + window > words.len() {
                continue;
            }

            // Exact compound match.
            let compound = words[i..i + window].join(" ");
            if dict.contains(&compound) {
                let mut wp = morphology::parse_word(&compound, dict);
                // Also add "split" hypotheses from parsing each word individually,
                // so both the compound and decomposed forms are available.
                add_split_hypotheses(&mut wp, &words[i..i + window], dict);
                word_parses.push(wp);
                i += window;
                matched = true;
                break;
            }

            // Compound+suffix: parse the last word, check if preceding words +
            // stem form a compound entry.
            if let Some(mut parse) = try_compound_suffix(&words[i..i + window], dict) {
                add_split_hypotheses(&mut parse, &words[i..i + window], dict);
                word_parses.push(parse);
                i += window;
                matched = true;
                break;
            }
        }
        if !matched {
            // Hyphenated tokens (e.g., number sequences like {pagh-pagh-DoD-cha'})
            // are split and each part parsed individually. Promote the number
            // ("num") reading for each part since hyphens join digits.
            let w = &words[i];
            if w.contains('-') && !dict.contains(w) {
                for part in w.split('-').filter(|p| !p.is_empty()) {
                    let mut wp = morphology::parse_word(part, dict);
                    prefer_num_reading(&mut wp);
                    word_parses.push(wp);
                }
            } else {
                word_parses.push(morphology::parse_word(w, dict));
            }
            i += 1;
        }
    }

    // Post-process: detect the comparative pattern (A Q law' B Q puS).
    rewrite_comparative(&mut word_parses);

    // Post-process: demote adjectival hypotheses where context makes them
    // unlikely (sentence-initial, after {latlh}, etc.).
    demote_contextual_adjectival(&mut word_parses);

    // Post-process: prefer verb readings for bare words following a noun.
    prefer_verb_after_noun(&mut word_parses);

    // Post-process: prefer noun for {wej} after a number ({wa'maH wej} = "thirteen").
    prefer_noun_after_number(&mut word_parses);

    // Post-process: {je} at end of sentence or before a verb → conjunction "and".
    prefer_conj_je(&mut word_parses);

    // Post-process: {pagh} between two verbs → conjunction "or".
    prefer_conj_pagh(&mut word_parses);

    SentenceParse {
        input: input.to_string(),
        words: word_parses,
    }
}

/// Detect the Klingon comparative/superlative pattern ({A Q law' B Q puS}) and
/// replace the {law'} and {puS} word parses with `{... law':sen}` and
/// `{... puS:sen}`.
fn rewrite_comparative(parses: &mut Vec<WordParse>) {
    // Find indices where the top hypothesis stem is {law'} or {puS}.
    let mut law_indices = Vec::new();
    let mut pus_indices = Vec::new();
    for (i, wp) in parses.iter().enumerate() {
        if wp.word == "law'" {
            law_indices.push(i);
        } else if wp.word == "puS" {
            pus_indices.push(i);
        }
    }

    // Pair each {law'} with the next {puS} that follows it.
    // Require at least one word between them (the real pattern is {A Q law' B Q puS}).
    for &li in &law_indices {
        if let Some(&pi) = pus_indices.iter().find(|&&pi| pi > li + 1) {
            parses[li] = comparative_word_parse("... law'", "sen");
            parses[pi] = comparative_word_parse("... puS", "sen");

            // The word before {law'} and before {puS} is the quality verb Q, and
            // it must be the same word. Prefer the verb hypothesis for both.
            if li > 0 && pi > 0 {
                let q_before_law = li - 1;
                let q_before_pus = pi - 1;
                if parses[q_before_law].word == parses[q_before_pus].word {
                    prefer_verb(&mut parses[q_before_law]);
                    prefer_verb(&mut parses[q_before_pus]);
                }
            }
        }
    }
}

/// Demote adjectival hypotheses where sentence context makes them unlikely.
/// Sentence-initial words cannot be adjectival (no preceding noun to modify).
/// Words following {latlh} are more likely nouns (noun-noun "other X" pattern).
fn demote_contextual_adjectival(parses: &mut [WordParse]) {
    for i in 0..parses.len() {
        let top_is_adjectival = parses[i]
            .hypotheses
            .first()
            .map_or(false, |h| h.parse_type == ParseType::Adjectival);
        if !top_is_adjectival {
            continue;
        }

        // Rule 1: sentence-initial words cannot be adjectival.
        // Rule 2: words following {latlh} are more likely nouns.
        let should_demote = i == 0 || parses[i - 1].word == "latlh";
        if should_demote {
            if let Some(pos) = parses[i]
                .hypotheses
                .iter()
                .position(|h| h.parse_type != ParseType::Adjectival)
            {
                let preferred = parses[i].hypotheses.remove(pos);
                parses[i].hypotheses.insert(0, preferred);
            }
        }
    }
}

/// Klingon pronouns (closed class). After a noun these are copulas
/// ("X jIH" = "I am X"), not verbs.
const PRONOUNS: &[&str] = &["jIH", "SoH", "ghaH", "'oH", "maH", "tlhIH", "chaH", "bIH"];

/// Type 4 (possessive) noun suffixes. A noun phrase ending with one of these
/// is a complete possessive NP; a following bare word is more likely another
/// noun in a genitive pattern than a verb.
const POSSESSIVE_SUFFIXES: &[&str] = &[
    "-wIj", "-lIj", "-Daj", "-chaj", "-maj", "-raj",
    "-wI'", "-lI'", "-ma'", "-ra'",
];

/// Prefer verb readings for ambiguous bare words that follow a noun phrase.
/// In Klingon OVS word order, a bare word after a noun is more likely a verb
/// (the noun is its object) than another noun.
fn prefer_verb_after_noun(parses: &mut [WordParse]) {
    for i in 1..parses.len() {
        // Check if the preceding word's top hypothesis is a noun.
        let prev_top = match parses[i - 1].hypotheses.first() {
            Some(h) => h,
            None => continue,
        };
        let prev_first_pos = prev_top
            .components
            .first()
            .map(|c| c.pos.as_str())
            .unwrap_or("");
        if !prev_first_pos.starts_with("n") {
            continue;
        }

        // Skip numbers — after a number, the next word is far more likely a
        // noun being counted than a verb (e.g., {wa' ram} = "one night").
        if prev_first_pos.contains("num") {
            continue;
        }

        // Skip if the preceding noun ends with a possessive suffix — the NP
        // is complete and the next word is likely a noun (genitive pattern,
        // e.g., {SoSlI' Quch} = "your mother's forehead").
        let prev_ends_possessive = prev_top
            .components
            .last()
            .map_or(false, |c| POSSESSIVE_SUFFIXES.contains(&c.entry_name.as_str()));
        if prev_ends_possessive {
            continue;
        }

        // Skip pronouns — after a noun they function as copulas, not verbs
        // (e.g., {tlhIngan jIH} = "I am a Klingon").
        if PRONOUNS.contains(&parses[i].word.as_str()) {
            continue;
        }

        // Only intervene if the word has both noun and verb WholeWord readings,
        // and the noun reading is not a number (numbers should not become verbs).
        let has_noun_ww = parses[i].hypotheses.iter().any(|h| {
            h.parse_type == ParseType::WholeWord
                && h.components.first().map_or(false, |c| c.pos.starts_with("n"))
        });
        let noun_is_number = parses[i].hypotheses.iter().any(|h| {
            h.parse_type == ParseType::WholeWord
                && h.components.first().map_or(false, |c| c.pos.contains("num"))
        });
        let has_verb_ww = parses[i].hypotheses.iter().any(|h| {
            h.parse_type == ParseType::WholeWord
                && h.components.first().map_or(false, |c| c.pos.starts_with("v"))
        });

        if has_noun_ww && has_verb_ww && !noun_is_number {
            prefer_verb(&mut parses[i]);
        }
    }
}

/// Promote the highest-scoring verb hypothesis to the top.
fn prefer_verb(wp: &mut WordParse) {
    if let Some(pos) = wp
        .hypotheses
        .iter()
        .position(|h| h.parse_type == ParseType::Verb || (h.parse_type == ParseType::WholeWord
            && h.components.first().map_or(false, |c| c.pos.starts_with("v"))))
    {
        if pos > 0 {
            let verb = wp.hypotheses.remove(pos);
            wp.hypotheses.insert(0, verb);
        }
    }
}

/// Promote the WholeWord hypothesis with "num" in its POS to position 0.
/// Used for parts of hyphenated number sequences.
fn prefer_num_reading(wp: &mut WordParse) {
    if let Some(pos) = wp.hypotheses.iter().position(|h| {
        h.parse_type == ParseType::WholeWord
            && h.components
                .first()
                .map_or(false, |c| c.pos.contains("num"))
    }) {
        if pos > 0 {
            let num = wp.hypotheses.remove(pos);
            wp.hypotheses.insert(0, num);
        }
    }
}

/// After a number, prefer noun over adverb for ambiguous words.
/// E.g., {wa'maH wej} = "thirteen" — {wej} is noun "three", not adverb "not yet".
fn prefer_noun_after_number(parses: &mut [WordParse]) {
    for i in 1..parses.len() {
        let prev_is_number = parses[i - 1]
            .hypotheses
            .first()
            .map_or(false, |h| {
                h.components
                    .first()
                    .map_or(false, |c| c.pos.contains("num"))
            });
        if !prev_is_number {
            continue;
        }

        // If the current word has a noun WholeWord reading, promote it.
        if let Some(pos) = parses[i].hypotheses.iter().position(|h| {
            h.parse_type == ParseType::WholeWord
                && h.components
                    .first()
                    .map_or(false, |c| c.pos.starts_with("n"))
        }) {
            if pos > 0 {
                let noun = parses[i].hypotheses.remove(pos);
                parses[i].hypotheses.insert(0, noun);
            }
        }
    }
}

/// Promote the conjunction reading of {je} when it is sentence-final or
/// followed by a verb. In these positions {je} means "and" (listing), not
/// "also" (adverb).
fn prefer_conj_je(parses: &mut [WordParse]) {
    for i in 0..parses.len() {
        if parses[i].word != "je" {
            continue;
        }
        let is_last = i == parses.len() - 1;
        let next_is_verb = !is_last
            && parses[i + 1]
                .hypotheses
                .first()
                .map_or(false, |h| {
                    h.components
                        .first()
                        .map_or(false, |c| c.pos.starts_with("v"))
                });
        if is_last || next_is_verb {
            promote_pos(&mut parses[i], "conj");
        }
    }
}

/// Promote the conjunction reading of {pagh} when it appears between two
/// verb-initial words. The pattern "V pagh V" means "V or V".
fn prefer_conj_pagh(parses: &mut [WordParse]) {
    for i in 1..parses.len() {
        if parses[i].word != "pagh" {
            continue;
        }
        if i + 1 >= parses.len() {
            continue;
        }
        let prev_is_verb = parses[i - 1]
            .hypotheses
            .first()
            .map_or(false, |h| {
                h.components
                    .first()
                    .map_or(false, |c| c.pos.starts_with("v"))
            });
        let next_is_verb = parses[i + 1]
            .hypotheses
            .first()
            .map_or(false, |h| {
                h.components
                    .first()
                    .map_or(false, |c| c.pos.starts_with("v"))
            });
        if prev_is_verb && next_is_verb {
            promote_pos(&mut parses[i], "conj");
        }
    }
}

/// Promote the first WholeWord hypothesis whose POS matches `target` exactly.
fn promote_pos(wp: &mut WordParse, target: &str) {
    if let Some(pos) = wp.hypotheses.iter().position(|h| {
        h.parse_type == ParseType::WholeWord
            && h.components
                .first()
                .map_or(false, |c| c.pos == target)
    }) {
        if pos > 0 {
            let preferred = wp.hypotheses.remove(pos);
            wp.hypotheses.insert(0, preferred);
        }
    }
}

fn comparative_word_parse(entry_name: &str, pos: &str) -> WordParse {
    WordParse {
        word: entry_name.to_string(),
        hypotheses: vec![Hypothesis {
            components: vec![Component {
                text: entry_name.to_string(),
                entry_name: entry_name.to_string(),
                pos: pos.to_string(),
                homophone: None,
                role: MorphemeRole::Stem,
                sub_components: vec![],
            }],
            confidence: 100.0,
            parse_type: ParseType::WholeWord,
            warnings: vec![],
        }],
    }
}

/// Add "split" hypotheses to a compound WordParse by parsing each word in the
/// window individually and merging their top hypotheses into combined entries.
/// This ensures both compound and decomposed forms are available for matching.
fn add_split_hypotheses(wp: &mut WordParse, window: &[String], dict: &Dictionary) {
    let individual: Vec<WordParse> = window
        .iter()
        .map(|w| morphology::parse_word(w, dict))
        .collect();

    // Build merged hypotheses from the cross product of each word's hypotheses.
    // Limit to top 3 per word to keep the count reasonable.
    let cap = 3;
    let mut combos: Vec<Vec<&Hypothesis>> = vec![vec![]];
    for word_parse in &individual {
        let mut new_combos = Vec::new();
        for combo in &combos {
            for hyp in word_parse.hypotheses.iter().take(cap) {
                let mut extended = combo.clone();
                extended.push(hyp);
                new_combos.push(extended);
            }
        }
        combos = new_combos;
    }

    for combo in combos {
        let components: Vec<Component> = combo
            .iter()
            .flat_map(|h| h.components.clone())
            .collect();
        let warnings: Vec<String> = combo
            .iter()
            .flat_map(|h| h.warnings.clone())
            .collect();
        // Derive parse_type from the first underlying hypothesis; fall back to Noun.
        let parse_type = combo
            .first()
            .map(|h| h.parse_type)
            .unwrap_or(ParseType::Noun);
        let mut merged = Hypothesis {
            components,
            confidence: 0.0,
            parse_type,
            warnings,
        };
        merged.confidence = confidence::score(&merged, dict);
        wp.hypotheses.push(merged);
    }

    // Re-sort after adding split hypotheses.
    wp.hypotheses.sort_by(confidence::compare);
}

/// Try to match a window of words as a compound+suffix. Parses the last word,
/// and for each hypothesis with suffixes, checks if the preceding words joined
/// with the stem form a compound dictionary entry.
fn try_compound_suffix(window: &[String], dict: &Dictionary) -> Option<WordParse> {
    if window.len() < 2 {
        return None;
    }

    let last_word = &window[window.len() - 1];
    let prefix_str = window[..window.len() - 1].join(" ");
    let last_parse = morphology::parse_word(last_word, dict);

    for hyp in &last_parse.hypotheses {
        let stem = hyp.components.iter().find(|c| c.role == MorphemeRole::Stem)?;
        let suffixes: Vec<_> = hyp
            .components
            .iter()
            .filter(|c| matches!(c.role, MorphemeRole::Suffix | MorphemeRole::Rover))
            .cloned()
            .collect();
        if suffixes.is_empty() {
            continue;
        }

        let candidate = format!("{prefix_str} {}", stem.text);
        if let Some(entries) = dict.lookup(&candidate) {
            let entry = &entries[0];
            let mut components = vec![Component {
                text: candidate.clone(),
                entry_name: candidate,
                pos: entry.canonical_pos.clone(),
                homophone: if entry.homophone > 0 {
                    Some(entry.homophone)
                } else {
                    None
                },
                role: MorphemeRole::Stem,
                sub_components: vec![],
            }];
            components.extend(suffixes);

            let mut hypothesis = Hypothesis {
                components,
                confidence: 0.0,
                parse_type: hyp.parse_type,
                warnings: hyp.warnings.clone(),
            };
            hypothesis.confidence = confidence::score(&hypothesis, dict);

            return Some(WordParse {
                word: window.join(" "),
                hypotheses: vec![hypothesis],
            });
        }
    }

    None
}

/// Split a sentence into words, handling punctuation correctly.
/// Apostrophe (') is NEVER treated as punctuation — it is a Klingon letter.
fn split_sentence(input: &str) -> Vec<String> {
    input
        .trim()
        .split_whitespace()
        .map(|w| {
            // Strip trailing punctuation (period, comma, exclamation, question,
            // semicolon) but NEVER strip apostrophe — it is a Klingon letter.
            let w = w.trim_end_matches(|c: char| matches!(c, '.' | '!' | '?' | ',' | ';'));
            w.to_string()
        })
        .filter(|w| !w.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_simple() {
        assert_eq!(
            split_sentence("batlh bIHeghjaj."),
            vec!["batlh", "bIHeghjaj"]
        );
    }

    #[test]
    fn test_split_preserves_apostrophe() {
        assert_eq!(split_sentence("Qapla'"), vec!["Qapla'"]);
    }

    #[test]
    fn test_split_strips_comma() {
        assert_eq!(
            split_sentence("wo', Hegh"),
            vec!["wo'", "Hegh"]
        );
    }

    #[test]
    fn test_split_multiple_punctuation() {
        assert_eq!(
            split_sentence("tlhIngan maH!"),
            vec!["tlhIngan", "maH"]
        );
    }
}
