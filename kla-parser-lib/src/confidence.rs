// Confidence scoring and ranking for morphological hypotheses.

use std::cmp::Ordering;

use crate::dictionary::Dictionary;
use crate::types::*;

// Scoring weights.
const STEM_IN_DICT: f64 = 50.0;
const POS_MATCH: f64 = 20.0;
const WHOLE_WORD_BONUS: f64 = 30.0;
const AFFIX_PENALTY: f64 = -2.0;
const GHACH_NO_SUFFIX: f64 = -30.0;
const OY_INVALID: f64 = -30.0;
const STEM_NOT_IN_DICT: f64 = -10.0;
const POSSESSIVE_MISMATCH: f64 = -15.0;
const EMPTY_STEM: f64 = -100.0;
// Question words (ques) are almost always used in that role when standalone.
const QUES_BONUS: f64 = 5.0;
// Letter names (consonants/vowels) are rarely used in normal sentences.
const LETTER_PENALTY: f64 = -10.0;
// Pronoun-as-copula readings are common and should outrank verb homophones.
const PRONOUN_COPULA_BONUS: f64 = 5.0;
// Slang whole-word entries are less likely than their decomposed counterparts.
const SLANG_PENALTY: f64 = -5.0;
// Archaic or hypothetical entries are vanishingly rare in real usage; penalise
// heavily so they never compete with common readings (e.g. {'ej:conj} should
// outrank {'ej:n:hyp} unambiguously).
const RARE_PENALTY: f64 = -50.0;
// Whole-word entries that are transparently noun + plural suffix (e.g., {nuHmey})
// are convenience listings; prefer decomposition.
const TRANSPARENT_PLURAL_PENALTY: f64 = -5.0;
// Stative verb with a prefix that requires a direct object - grammatically impossible.
const STATIVE_TRANSITIVE_PREFIX: f64 = -30.0;
// Stative verb with an imperative prefix - grammatically valid but rare.
const STATIVE_IMPERATIVE_PREFIX: f64 = -3.0;
// Stative verb with a Type 2 (predisposition/volition) suffix — semantically
// incoherent: "ready to be green" etc. The suffix presupposes an action, so
// prefer a non-stative homophone of the stem when one exists.
const STATIVE_VOLITIONAL_SUFFIX: f64 = -30.0;
// Type 2 verb suffixes (TKD §4.2.2: predisposition / volition / readiness).
const VOLITIONAL_SUFFIXES: &[&str] = &["-nIS", "-qang", "-rup", "-beH", "-vIp"];
// Corpus frequency bonus per annotated occurrence of a homophone. Small enough
// not to override grammar-based scoring, large enough to break ties.
const CORPUS_FREQ_PER_OCCURRENCE: f64 = 0.1;
// Same idea, but per occurrence of the base POS for a stem. Lets the parser
// prefer the POS readings that actually appear in annotated sentences (e.g.,
// {'ej:conj} over the archaic {'ej:n:hyp}).
const POS_FREQ_PER_OCCURRENCE: f64 = 0.1;
// Bonus for whole-word entries whose name is a multi-word compound. The
// dictionary has explicitly registered the compound as a unit, so prefer it
// over a split into corpus-frequent constituents (e.g., {may' ta:n} over
// {may':n}, {ta:n}). The split form's primary stem can pick up at most
// `POS_FREQ_PER_OCCURRENCE * 10 = 1.0` from the POS-frequency bonus; this
// constant is set to twice that cap so the compound wins even when the
// constituents happen to be corpus-frequent.
const COMPOUND_WHOLE_WORD_BONUS: f64 = 2.0;
// Counts at or below this threshold contribute no POS bonus. Suppresses weak
// signals - e.g. `wej:n=2` vs `wej:adv=1` should not bias toward `n`, but
// `'ej:conj=9` vs `'ej:n=0` still produces a clear preference.
const POS_FREQ_BASELINE: u32 = 2;
// Type-2 noun suffixes (plurals) that mark a whole-word entry as transparent.
const NOUN_PLURAL_SUFFIXES: &[&str] = &["-mey", "-pu'", "-Du'"];
// Prefixes that never take a direct object (intransitive only).
const NO_OBJECT_PREFIXES: &[&str] = &["jI", "bI", "ma", "Su"];
// Imperative prefixes (ambiguous: may or may not have an object).
const IMPERATIVE_PREFIXES: &[&str] = &["yI", "pe"];

/// Compute the confidence score for a hypothesis.
pub fn score(h: &Hypothesis, dict: &Dictionary) -> f64 {
    let mut s = 0.0;

    // Find the primary stem component.
    let stem = h.components.iter().find(|c| c.role == MorphemeRole::Stem);
    let stem_name = stem.map(|c| c.entry_name.as_str()).unwrap_or("");

    if stem_name.is_empty() {
        return EMPTY_STEM;
    }

    // Stem in dictionary.
    if dict.contains(stem_name) {
        s += STEM_IN_DICT;

        // POS match: check if the stem has an entry matching the parse type.
        let expected_pos = match h.parse_type {
            ParseType::Verb | ParseType::Adjectival => "v",
            ParseType::Noun | ParseType::Pronoun => "n",
            _ => "",
        };
        if !expected_pos.is_empty() {
            let has_match = dict
                .lookup(stem_name)
                .map(|es| es.iter().any(|e| e.pos == expected_pos))
                .unwrap_or(false);
            if has_match {
                s += POS_MATCH;
            }
        }

        // Pronoun-as-copula bonus.
        if h.parse_type == ParseType::Pronoun {
            s += PRONOUN_COPULA_BONUS;
        }

        // Heavy penalty for archaic/hypothetical entries. Match by stem base
        // POS and homophone so e.g. {'ej:n:hyp} is penalised but {'ej:conj}
        // (a separate entry on the same stem) is not.
        if let Some(stem) = stem {
            let base_pos = stem
                .pos
                .split(':')
                .next()
                .unwrap_or(&stem.pos)
                .split(',')
                .next()
                .unwrap_or(&stem.pos);
            let homo = stem.homophone.unwrap_or(0);
            let is_rare = dict.lookup(stem_name).map_or(false, |es| {
                es.iter()
                    .any(|e| e.rare && e.pos == base_pos && e.homophone == homo)
            });
            if is_rare {
                s += RARE_PENALTY;
            }
        }
    } else {
        s += STEM_NOT_IN_DICT;
    }

    // Penalize additional stems not found in the dictionary. Split hypotheses
    // (from multi-word compound parsing) can have multiple stems; unknown ones
    // like {poHmey} (not a real entry) should drag the score down.
    let extra_stems = h
        .components
        .iter()
        .filter(|c| c.role == MorphemeRole::Stem)
        .skip(1);
    for extra in extra_stems {
        let name = extra.entry_name.as_str();
        if !name.is_empty() && !dict.contains(name) {
            s += STEM_NOT_IN_DICT;
        }
    }

    // Whole-word bonus (uses the first stem for type-specific checks).
    let first_stem = stem;
    if h.parse_type == ParseType::WholeWord {
        // Check for slang and transparent plural entries.
        let homo = first_stem.map(|s| s.homophone.unwrap_or(0)).unwrap_or(0);
        let is_slang = dict.lookup(stem_name).map_or(false, |es| {
            es.iter().any(|e| e.slang && e.homophone == homo)
        });
        let is_transparent_plural = first_stem.map_or(false, |s| {
            s.sub_components.last().map_or(false, |last| {
                NOUN_PLURAL_SUFFIXES.contains(&last.entry_name.as_str())
            })
        });

        if is_slang {
            s += SLANG_PENALTY;
        } else if is_transparent_plural {
            s += TRANSPARENT_PLURAL_PENALTY;
        } else {
            s += WHOLE_WORD_BONUS;
        }

        // Multi-word compound entries get an extra bump so the explicit
        // registration in the dictionary outranks a split into individually
        // corpus-frequent constituents.
        if stem_name.contains(' ') {
            s += COMPOUND_WHOLE_WORD_BONUS;
        }

        // Question words are almost always the intended reading when standalone.
        if let Some(stem) = first_stem {
            if stem.pos.starts_with("ques") {
                s += QUES_BONUS;
            }
        }

        // Letter names (consonants/vowels) are rarely used in sentences.
        if let Some(stem) = first_stem {
            let stem_name = stem.entry_name.as_str();
            let homo = stem.homophone.unwrap_or(0);
            let is_letter = dict.lookup(stem_name).map_or(false, |es| {
                es.iter().any(|e| e.letter && e.homophone == homo)
            });
            if is_letter {
                s += LETTER_PENALTY;
            }
        }
    }

    // Affix penalty.
    let affix_count = h
        .components
        .iter()
        .filter(|c| matches!(c.role, MorphemeRole::Prefix | MorphemeRole::Suffix | MorphemeRole::Rover))
        .count();
    s += affix_count as f64 * AFFIX_PENALTY;

    // Prefix-transitivity check: stative verbs cannot take a direct object.
    // Volitional-suffix check: Type 2 suffixes (need/ready/willing/afraid)
    // presuppose an action and don't combine with stative readings.
    if h.parse_type == ParseType::Verb {
        let prefix = h
            .components
            .iter()
            .find(|c| c.role == MorphemeRole::Prefix);
        let homo = stem.map(|s| s.homophone.unwrap_or(0)).unwrap_or(0);
        let is_stative = dict.lookup(stem_name).map_or(false, |es| {
            es.iter().any(|e| e.homophone == homo && e.stative)
        });
        if is_stative {
            if let Some(pfx) = prefix {
                let pfx_text = pfx.text.as_str();
                if NO_OBJECT_PREFIXES.contains(&pfx_text) {
                    // Intransitive prefix + stative: compatible, no penalty.
                } else if IMPERATIVE_PREFIXES.contains(&pfx_text) {
                    s += STATIVE_IMPERATIVE_PREFIX;
                } else {
                    // All other non-null prefixes always indicate a direct object.
                    s += STATIVE_TRANSITIVE_PREFIX;
                }
            }
            let has_volitional = h.components.iter().any(|c| {
                c.role == MorphemeRole::Suffix
                    && VOLITIONAL_SUFFIXES.contains(&c.entry_name.as_str())
            });
            if has_volitional {
                s += STATIVE_VOLITIONAL_SUFFIX;
            }
        }
    }

    // Corpus frequency bonus: prefer homophones that appear more often in
    // annotated sentences. Only applies when a specific homophone is selected.
    if let Some(stem) = stem {
        if let Some(homo) = stem.homophone {
            let freq = dict.homophone_frequency(&stem.entry_name, homo);
            s += freq.min(10) as f64 * CORPUS_FREQ_PER_OCCURRENCE;
        }
        // POS frequency bonus: prefer base POS readings that appear more often
        // in annotated sentences. Breaks ties between e.g. {'ej:conj} (common)
        // and {'ej:n} (archaic).
        let base_pos = stem
            .pos
            .split(':')
            .next()
            .unwrap_or(&stem.pos)
            .split(',')
            .next()
            .unwrap_or(&stem.pos);
        let pos_freq = dict.pos_frequency(&stem.entry_name, base_pos);
        s += pos_freq.saturating_sub(POS_FREQ_BASELINE).min(10) as f64
            * POS_FREQ_PER_OCCURRENCE;
    }

    // Warning-based penalties.
    for w in &h.warnings {
        if w.contains("-ghach") {
            s += GHACH_NO_SUFFIX;
        }
        if w.contains("-'oy") {
            s += OY_INVALID;
        }
        if w.contains("possessive but") {
            s += POSSESSIVE_MISMATCH;
        }
    }

    s
}

/// Compare two hypotheses for sorting (best first).
/// Ties broken by: fewer components (parsimony) > parse type enum order
/// (WholeWord < Verb < Adjectival < Pronoun < Noun < Unknown) > alphabetical.
pub fn compare(a: &Hypothesis, b: &Hypothesis) -> Ordering {
    // Higher confidence first.
    b.confidence
        .partial_cmp(&a.confidence)
        .unwrap_or(Ordering::Equal)
        .then_with(|| {
            // Fewer components first (parsimony).
            a.components.len().cmp(&b.components.len())
        })
        .then_with(|| {
            // Parse type preference by enum declaration order.
            a.parse_type.cmp(&b.parse_type)
        })
        .then_with(|| {
            // Alphabetical by first component entry name.
            let a_name = a
                .components
                .first()
                .map(|c| c.entry_name.as_str())
                .unwrap_or("");
            let b_name = b
                .components
                .first()
                .map(|c| c.entry_name.as_str())
                .unwrap_or("");
            a_name.cmp(b_name)
        })
        .then_with(|| {
            // Alphabetical by first component POS.
            let a_pos = a
                .components
                .first()
                .map(|c| c.pos.as_str())
                .unwrap_or("");
            let b_pos = b
                .components
                .first()
                .map(|c| c.pos.as_str())
                .unwrap_or("");
            a_pos.cmp(b_pos)
        })
}
