use std::collections::HashMap;

use crate::types::{DictEntry, EmbeddedData, SentenceEntry};

const EMBEDDED_JSON: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/dictionary.json"));

/// In-memory dictionary loaded once from compile-time embedded data.
pub struct Dictionary {
    entries: HashMap<String, Vec<DictEntry>>,
    sentences: Vec<SentenceEntry>,
    /// Corpus-derived frequency counts: (stem, homophone) → occurrence count.
    /// Built at init time by parsing annotated sentence components.
    homophone_freq: HashMap<String, HashMap<u8, u32>>,
}

impl Dictionary {
    /// Load the dictionary from embedded JSON data.
    pub fn new() -> Self {
        let data: EmbeddedData =
            serde_json::from_slice(EMBEDDED_JSON).expect("embedded dictionary JSON is invalid");

        let mut entries: HashMap<String, Vec<DictEntry>> = HashMap::new();
        for entry in data.entries {
            entries
                .entry(entry.name.clone())
                .or_default()
                .push(entry);
        }

        let homophone_freq = build_homophone_freq(&data.sentences);

        Self {
            entries,
            sentences: data.sentences,
            homophone_freq,
        }
    }

    /// Look up all entries for a given stem name.
    pub fn lookup(&self, name: &str) -> Option<&[DictEntry]> {
        self.entries.get(name).map(|v| v.as_slice())
    }

    /// Look up entries matching a specific POS.
    pub fn lookup_by_pos(&self, name: &str, pos: &str) -> Vec<&DictEntry> {
        self.entries
            .get(name)
            .map(|v| v.iter().filter(|e| e.pos == pos).collect())
            .unwrap_or_default()
    }

    /// Check whether any entry with this name exists.
    pub fn contains(&self, name: &str) -> bool {
        self.entries.contains_key(name)
    }

    /// Return all sentence entries with canonical components.
    pub fn sentences(&self) -> &[SentenceEntry] {
        &self.sentences
    }

    /// Look up how often a specific homophone appears in the annotated corpus.
    pub fn homophone_frequency(&self, stem: &str, homophone: u8) -> u32 {
        self.homophone_freq
            .get(stem)
            .and_then(|m| m.get(&homophone))
            .copied()
            .unwrap_or(0)
    }
}

/// Build a frequency table of homophone occurrences from annotated sentence
/// components. Parses each `{word:pos:N}` token and counts how often each
/// (word, homophone) pair appears across all sentences.
fn build_homophone_freq(sentences: &[SentenceEntry]) -> HashMap<String, HashMap<u8, u32>> {
    let mut freq: HashMap<String, HashMap<u8, u32>> = HashMap::new();
    for sent in sentences {
        for token in sent.components.split(", ") {
            if let Some((word, homo)) = extract_homophone(token.trim()) {
                *freq.entry(word).or_default().entry(homo).or_insert(0) += 1;
            }
        }
    }
    freq
}

/// Extract (word, homophone) from a bracketed component like `{ghoS:v:2}`.
/// Returns None for affixes, sentence references, or components without a
/// homophone number.
fn extract_homophone(token: &str) -> Option<(String, u8)> {
    let inner = token.strip_prefix('{')?.strip_suffix('}')?;
    // Skip affixes and sentence references.
    if inner.starts_with('-') || inner.starts_with("...") {
        return None;
    }
    let colon = inner.find(':')?;
    let word = &inner[..colon];
    // Skip affixes (prefix notation with trailing dash).
    if word.ends_with('-') {
        return None;
    }
    let pos_part = &inner[colon + 1..];
    // Skip sentence references.
    if pos_part.starts_with("sen") {
        return None;
    }
    // The homophone is the last colon-separated segment if it starts with a
    // digit. Handles formats like "v:2", "n:1,num", "n:1h".
    let last_colon = pos_part.rsplit(':').next()?;
    let first_seg = last_colon.split(',').next()?;
    let digits: String = first_seg.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return None;
    }
    let homo = digits.parse::<u8>().ok()?;
    if homo == 0 {
        return None;
    }
    Some((word.to_string(), homo))
}

impl Default for Dictionary {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dictionary_loads() {
        let dict = Dictionary::new();
        // The dictionary should have thousands of entries.
        assert!(dict.entries.len() > 1000, "dictionary too small");
    }

    #[test]
    fn test_lookup_qapla() {
        let dict = Dictionary::new();
        let results = dict.lookup("Qapla'");
        assert!(results.is_some(), "Qapla' should be in dictionary");
    }

    #[test]
    fn test_lookup_nonexistent() {
        let dict = Dictionary::new();
        assert!(dict.lookup("zzzznotaword").is_none());
    }

    #[test]
    fn test_contains() {
        let dict = Dictionary::new();
        assert!(dict.contains("tlhIngan"));
        assert!(!dict.contains("zzzznotaword"));
    }

    #[test]
    fn test_sentences_loaded() {
        let dict = Dictionary::new();
        assert!(
            dict.sentences().len() > 100,
            "expected at least 100 sentence entries"
        );
    }

    #[test]
    fn test_homophone_frequency() {
        let dict = Dictionary::new();
        // {SaH:v:2} ("care about") appears in multiple corpus sentences;
        // {SaH:v:1} ("be sober") does not. The frequency table should
        // reflect this.
        let freq2 = dict.homophone_frequency("SaH", 2);
        let freq1 = dict.homophone_frequency("SaH", 1);
        assert!(
            freq2 > freq1,
            "SaH:v:2 should appear more often than SaH:v:1 (got {freq2} vs {freq1})"
        );
    }
}
