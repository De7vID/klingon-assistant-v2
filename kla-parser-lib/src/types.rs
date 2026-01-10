use serde::{Deserialize, Serialize};

/// Role of a morpheme within a word.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MorphemeRole {
    Prefix,
    Stem,
    Suffix,
    Rover,
}

/// Type of parse (what grammatical category was assumed).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ParseType {
    WholeWord,
    Verb,
    Adjectival,
    Pronoun,
    Noun,
    Unknown,
}

/// A single morpheme in a decomposition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Component {
    /// The surface text of this morpheme (e.g., {pu'}, {bI}, {Hegh}).
    pub text: String,
    /// The entry name with dash notation (e.g., {-pu'}, {bI-}, {Hegh}).
    pub entry_name: String,
    /// Part of speech (e.g., "n", "v", "adv").
    pub pos: String,
    /// Homophone index if applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub homophone: Option<u8>,
    /// Role in the word.
    pub role: MorphemeRole,
    /// Internal decomposition of a whole-word entry (e.g., {nuHmey} → {nuH} + {-mey}).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sub_components: Vec<Component>,
}

/// One hypothesis for how a word decomposes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hypothesis {
    pub components: Vec<Component>,
    pub confidence: f64,
    pub parse_type: ParseType,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

/// All hypotheses for a single word.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WordParse {
    pub word: String,
    pub hypotheses: Vec<Hypothesis>,
}

/// All word parses for a sentence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentenceParse {
    pub input: String,
    pub words: Vec<WordParse>,
}

/// Raw dictionary entry loaded from embedded JSON.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DictEntry {
    pub name: String,
    /// Base POS (e.g., "n", "v", "adv") for morphological matching.
    pub pos: String,
    /// Canonical POS for output (e.g., "n:pro", "n:name", "v:1").
    pub canonical_pos: String,
    pub homophone: u8,
    /// True if tagged as a being (capable of language) in the dictionary.
    #[serde(default)]
    pub being: bool,
    /// True if this entry is a letter name (consonant or vowel).
    #[serde(default)]
    pub letter: bool,
    /// True if tagged as slang in the dictionary.
    #[serde(default)]
    pub slang: bool,
    /// True if this is a stative verb ("be X").
    #[serde(default)]
    pub stative: bool,
    /// Components string from the dictionary (e.g., "{nuH:n:1}, {-mey:n}").
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub components: String,
}

/// Raw embedded data from build.rs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddedData {
    pub entries: Vec<DictEntry>,
    pub sentences: Vec<SentenceEntry>,
}

/// A sentence with its expected canonical components.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentenceEntry {
    pub entry_name: String,
    pub components: String,
}
