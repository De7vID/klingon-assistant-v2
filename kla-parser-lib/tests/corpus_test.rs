// Corpus tests: parse known sentences and compare against expected bracketed components.
//
// Training/test split: hash(entry_name) % 5 != 0 is training, == 0 is holdout.
// Holdout tests are #[ignore]d and run with `cargo test -- --ignored`.

use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use kla_parser_lib::output::{sentence_to_bracketed, to_bracketed};
use kla_parser_lib::{parse_sentence, Dictionary};

fn dict() -> Dictionary {
    Dictionary::new()
}

fn hash_name(name: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    name.hash(&mut hasher);
    hasher.finish()
}

fn is_training(name: &str) -> bool {
    hash_name(name) % 5 != 0
}

/// Strip redundant POS sub-tags that the database uses inconsistently.
/// These tags are correct when present but their absence is not an error:
///   :pro  — distinguishes pronouns, but redundant with homophone number
///   :num  — distinguishes numbers, same
///   :suff — redundant because suffixes always begin with -
///   :pref — redundant because prefixes always end with -
fn normalize_tags(canonical: &str) -> String {
    const STRIP_TAGS: &[&str] = &["pro", "num", "name", "suff", "pref"];

    // Process each {...} component individually.
    let mut result = String::new();
    let mut rest = canonical;
    while let Some(open) = rest.find('{') {
        result.push_str(&rest[..open + 1]);
        rest = &rest[open + 1..];
        if let Some(close) = rest.find('}') {
            let inner = &rest[..close];
            // Inner format: "word:pos_part:pos_part:..." where each pos_part
            // is comma-separated tokens. Split on ":" then filter tokens.
            let colon_parts: Vec<&str> = inner.splitn(2, ':').collect();
            let word = colon_parts[0];
            result.push_str(word);
            if colon_parts.len() > 1 {
                // Preserve colon-separated structure: "pos:qualifier:...".
                // Within each colon-part, filter comma-separated tags.
                let pos_str = colon_parts[1];
                let mut colon_groups: Vec<String> = Vec::new();
                for group in pos_str.split(':') {
                    let filtered: Vec<&str> = group
                        .split(',')
                        .filter(|t| !t.is_empty())
                        .filter(|t| !STRIP_TAGS.contains(t))
                        // Strip "h" (hidden) flag from homophone numbers.
                        .map(|t| t.trim_end_matches('h'))
                        .collect();
                    if !filtered.is_empty() {
                        colon_groups.push(filtered.join(","));
                    }
                }
                if !colon_groups.is_empty() {
                    result.push(':');
                    result.push_str(&colon_groups.join(":"));
                }
            }
            result.push('}');
            rest = &rest[close + 1..];
        } else {
            break;
        }
    }
    result.push_str(rest);
    result
}

/// Parse a bracketed component string into individual `{...}` tokens.
fn parse_components(bracketed: &str) -> Vec<String> {
    bracketed
        .split(", ")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Expand sentence references in expected components. A component like
/// `{tlhIngan maH!:sen}` is replaced by looking up the sentence entry
/// "tlhIngan maH!" and substituting its own components.
fn expand_sentence_refs(components: &[String], sentence_map: &HashMap<String, String>) -> Vec<String> {
    let mut result = Vec::new();
    for comp in components {
        if let Some(expanded) = try_expand_sen_ref(comp, sentence_map) {
            result.extend(expanded);
        } else {
            result.push(comp.clone());
        }
    }
    result
}

/// If a component is a sentence reference (e.g., `{tlhIngan maH!:sen}`),
/// look up its sub-components and return them. Returns None if not a
/// sentence reference or if the entry is not found.
fn try_expand_sen_ref(comp: &str, sentence_map: &HashMap<String, String>) -> Option<Vec<String>> {
    let inner = comp.strip_prefix('{')?.strip_suffix('}')?;
    let colon = inner.find(':')?;
    let pos_part = &inner[colon + 1..];
    if !(pos_part == "sen" || pos_part.starts_with("sen:") || pos_part.starts_with("sen,")) {
        return None;
    }
    let entry_name = &inner[..colon];
    // Skip comparative pattern markers (... law', ... puS).
    if entry_name.starts_with("...") {
        return None;
    }
    let sub_components = sentence_map.get(entry_name)?;
    let normalized = normalize_tags(sub_components);
    let expanded = parse_components(&normalized);
    Some(expand_sentence_refs(&expanded, sentence_map))
}

/// Compare two component tokens. Homophone 0 in the expected component
/// matches any homophone number (0 means "no distinction needed").
fn component_eq(expected: &str, got: &str) -> bool {
    if expected == got {
        return true;
    }
    // Check for :0} or :0, in expected — strip the homophone from both sides.
    if expected.contains(":0}") || expected.contains(":0,") {
        let e = expected.replace(":0}", "}").replace(":0,", ",");
        let g = strip_trailing_homophone(got);
        return e == g;
    }
    false
}

/// Strip a trailing homophone number from a component token.
/// e.g., "{nuH:n:1}" → "{nuH:n}", "{nuH:n}" → "{nuH:n}".
fn strip_trailing_homophone(comp: &str) -> String {
    // Components look like {word:pos:N} — remove the :N before }.
    if let Some(brace) = comp.rfind('}') {
        let inner = &comp[..brace];
        if let Some(colon) = inner.rfind(':') {
            let after = &inner[colon + 1..];
            if after.chars().all(|c| c.is_ascii_digit()) && !after.is_empty() {
                return format!("{}{}", &comp[..colon], &comp[brace..]);
            }
        }
    }
    comp.to_string()
}

/// Compare two component slices with homophone-0 wildcard matching.
fn components_slice_eq(expected: &[String], got: &[String]) -> bool {
    expected.len() == got.len()
        && expected.iter().zip(got.iter()).all(|(e, g)| component_eq(e, g))
}

/// Check if the expected components can be produced by picking one hypothesis
/// per word from the parse result. Each hypothesis contributes one or more
/// components; the concatenation of all chosen hypotheses must match exactly.
fn any_hypothesis_matches(
    words: &[kla_parser_lib::types::WordParse],
    expected: &[String],
) -> bool {
    match_recursive(words, expected, 0, 0)
}

fn match_recursive(
    words: &[kla_parser_lib::types::WordParse],
    expected: &[String],
    word_idx: usize,
    comp_idx: usize,
) -> bool {
    // All words consumed: pass if all expected components are also consumed.
    if word_idx >= words.len() {
        return comp_idx >= expected.len();
    }
    // No more expected components but words remain: fail.
    if comp_idx >= expected.len() {
        return false;
    }

    for hyp in &words[word_idx].hypotheses {
        let hyp_components = parse_components(&normalize_tags(&to_bracketed(hyp)));
        let n = hyp_components.len();
        if comp_idx + n <= expected.len()
            && components_slice_eq(&expected[comp_idx..comp_idx + n], &hyp_components)
        {
            if match_recursive(words, expected, word_idx + 1, comp_idx + n) {
                return true;
            }
        }

        // Also try expanding sub_components for whole-word hypotheses.
        // A WholeWord with sub_components like [nuH, -mey] should match
        // expected components {nuH:n:1}, {-mey:n}.
        let expanded = expand_sub_components(hyp);
        if expanded.len() != n {
            let en = expanded.len();
            if comp_idx + en <= expected.len()
                && components_slice_eq(&expected[comp_idx..comp_idx + en], &expanded)
            {
                if match_recursive(words, expected, word_idx + 1, comp_idx + en) {
                    return true;
                }
            }
        }
    }
    false
}

/// Expand hypothesis components by replacing any component that has
/// sub_components with its sub_components sequence.
fn expand_sub_components(hyp: &kla_parser_lib::types::Hypothesis) -> Vec<String> {
    let mut result = Vec::new();
    for comp in &hyp.components {
        if !comp.sub_components.is_empty() {
            // Convert sub_components to bracketed format.
            for sub in &comp.sub_components {
                let name = &sub.entry_name;
                let pos = &sub.pos;
                result.push(normalize_tags(&format!("{{{name}:{pos}}}")));
            }
        } else {
            let name = &comp.entry_name;
            let pos = &comp.pos;
            result.push(normalize_tags(&format!("{{{name}:{pos}}}")));
        }
    }
    result
}

/// Build a map from sentence entry_name to components for sentence-ref expansion.
fn build_sentence_map(d: &Dictionary) -> HashMap<String, String> {
    d.sentences()
        .iter()
        .map(|s| (s.entry_name.clone(), s.components.clone()))
        .collect()
}

#[test]
fn corpus_training() {
    let d = dict();
    let sentences = d.sentences();
    let sentence_map = build_sentence_map(&d);
    let mut fail = 0;
    let mut failures = Vec::new();

    let mut top1_pass = 0;
    let mut any_pass = 0;

    for sent in sentences {
        if !is_training(&sent.entry_name) {
            continue;
        }

        let parse = parse_sentence(&sent.entry_name, &d);
        let bracketed = normalize_tags(&sentence_to_bracketed(&parse));
        let expected = normalize_tags(&sent.components);
        let expected_components = expand_sentence_refs(&parse_components(&expected), &sentence_map);

        let bracketed_components = parse_components(&bracketed);
        let top1_ok = components_slice_eq(&expected_components, &bracketed_components);
        let any_ok = top1_ok || any_hypothesis_matches(&parse.words, &expected_components);

        if top1_ok {
            top1_pass += 1;
        }
        if any_ok {
            any_pass += 1;
        } else {
            fail += 1;
            if failures.len() < 200 {
                let expanded_str = expected_components.join(", ");
                failures.push(format!(
                    "  {}\n    expected: {}\n    expanded: {}\n    got:      {}",
                    sent.entry_name, expected, expanded_str, bracketed
                ));
            }
        }
    }

    let total = any_pass + fail;
    let top1_pct = if total > 0 {
        (top1_pass as f64 / total as f64) * 100.0
    } else {
        0.0
    };
    let any_pct = if total > 0 {
        (any_pass as f64 / total as f64) * 100.0
    } else {
        0.0
    };

    eprintln!("\nCorpus training: top-1 {top1_pass}/{total} ({top1_pct:.1}%), any {any_pass}/{total} ({any_pct:.1}%).");
    if !failures.is_empty() {
        eprintln!("First failures:");
        for f in &failures {
            eprintln!("{f}");
        }
    }

    // Ensure the training corpus is non-empty so this test can detect regressions.
    assert!(total > 0, "no training corpus sentences found");
    // We don't assert 100% right away — this lets us see the accuracy rate.
    // Uncomment the assert once accuracy is acceptable:
    // assert!(any_pct >= 80.0, "accuracy {any_pct:.1}% below 80%");
}

#[test]
#[ignore]
fn corpus_holdout() {
    let d = dict();
    let sentences = d.sentences();
    let sentence_map = build_sentence_map(&d);
    let mut fail = 0;
    let mut failures = Vec::new();

    let mut top1_pass = 0;
    let mut any_pass = 0;

    for sent in sentences {
        if is_training(&sent.entry_name) {
            continue;
        }

        let parse = parse_sentence(&sent.entry_name, &d);
        let bracketed = normalize_tags(&sentence_to_bracketed(&parse));
        let expected = normalize_tags(&sent.components);
        let expected_components = expand_sentence_refs(&parse_components(&expected), &sentence_map);

        let bracketed_components = parse_components(&bracketed);
        let top1_ok = components_slice_eq(&expected_components, &bracketed_components);
        let any_ok = top1_ok || any_hypothesis_matches(&parse.words, &expected_components);

        if top1_ok {
            top1_pass += 1;
        }
        if any_ok {
            any_pass += 1;
        } else {
            fail += 1;
            if failures.len() < 200 {
                failures.push(format!(
                    "  {}\n    expected: {}\n    got:      {}",
                    sent.entry_name, expected, bracketed
                ));
            }
        }
    }

    let total = any_pass + fail;
    let top1_pct = if total > 0 {
        (top1_pass as f64 / total as f64) * 100.0
    } else {
        0.0
    };
    let any_pct = if total > 0 {
        (any_pass as f64 / total as f64) * 100.0
    } else {
        0.0
    };

    eprintln!("\nCorpus holdout: top-1 {top1_pass}/{total} ({top1_pct:.1}%), any {any_pass}/{total} ({any_pct:.1}%).");
    if !failures.is_empty() {
        eprintln!("First failures:");
        for f in &failures {
            eprintln!("{f}");
        }
    }
}

/// Find sentences where the corpus annotation decomposes a word that exists as
/// a whole-word dictionary entry. This catches cases where an entry was added
/// to the dictionary after the annotation was written.
#[test]
#[ignore]
fn find_overdecomposed() {
    let d = dict();
    let sentences = d.sentences();
    let mut findings = Vec::new();

    for sent in sentences {
        let components = parse_components(&sent.components);
        if components.is_empty() {
            continue;
        }

        // Extract (entry_name, pos) from each component token.
        let parsed: Vec<(&str, &str)> = components
            .iter()
            .filter_map(|c| {
                let inner = c.strip_prefix('{')?.strip_suffix('}')?;
                let colon = inner.find(':')?;
                Some((&inner[..colon], &inner[colon + 1..]))
            })
            .collect();

        // Try windows of size 2 and 3.
        for window_size in 2..=3 {
            if parsed.len() < window_size {
                continue;
            }
            for start in 0..=parsed.len() - window_size {
                let window = &parsed[start..start + window_size];

                // First element must not be a suffix (must be a stem or prefix).
                if window[0].0.starts_with('-') {
                    continue;
                }
                // Remaining elements must be suffixes.
                if !window[1..].iter().all(|(name, _)| name.starts_with('-')) {
                    continue;
                }

                // Concatenate: stem + suffix texts (strip leading dashes).
                let mut combined = window[0].0.to_string();
                for &(name, _) in &window[1..] {
                    combined.push_str(&name[1..]);
                }

                // Check if the combined form exists as a dictionary entry.
                if d.contains(&combined) {
                    let component_str: Vec<String> = window
                        .iter()
                        .map(|(name, pos)| format!("{{{name}:{pos}}}"))
                        .collect();
                    findings.push(format!(
                        "  {}\n    {} → {combined} exists in dictionary",
                        sent.entry_name,
                        component_str.join(", ")
                    ));
                }
            }
        }
    }

    eprintln!("\nOver-decomposed annotations ({} found):", findings.len());
    for f in &findings {
        eprintln!("{f}");
    }
}

/// Find sentences where the corpus annotation uses a whole-word component that
/// has sub_components in the dictionary. These are candidates where the
/// annotation might need to use the decomposed form instead.
#[test]
#[ignore]
fn find_underdecomposed() {
    let d = dict();
    let sentences = d.sentences();
    let mut findings = Vec::new();

    for sent in sentences {
        let components = parse_components(&sent.components);
        if components.is_empty() {
            continue;
        }

        for comp in &components {
            let inner = match comp.strip_prefix('{').and_then(|s| s.strip_suffix('}')) {
                Some(s) => s,
                None => continue,
            };
            let colon = match inner.find(':') {
                Some(c) => c,
                None => continue,
            };
            let entry_name = &inner[..colon];
            let pos_part = &inner[colon + 1..];

            // Skip suffixes, prefixes, and sentence references.
            if entry_name.starts_with('-') || entry_name.ends_with('-') {
                continue;
            }
            if pos_part.starts_with("sen") || entry_name.starts_with("...") {
                continue;
            }

            // Look up the entry and check if it has a components string.
            if let Some(entries) = d.lookup(entry_name) {
                for e in entries {
                    if e.components.is_empty() {
                        continue;
                    }
                    // Skip entries with "or" (ambiguous) or sentence refs.
                    if e.components.contains(" or ") || e.components.contains(":sen") {
                        continue;
                    }
                    findings.push(format!(
                        "  {}\n    {comp} has sub_components: {}",
                        sent.entry_name, e.components
                    ));
                    break; // One finding per component per sentence.
                }
            }
        }
    }

    eprintln!("\nUnder-decomposed annotations ({} found):", findings.len());
    for f in &findings {
        eprintln!("{f}");
    }
}
