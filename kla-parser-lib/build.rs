// Parses Klingon dictionary XML files and embeds them as JSON at compile time.

use quick_xml::events::Event;
use quick_xml::Reader;
use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Serialize)]
struct EmbeddedData {
    entries: Vec<DictEntryRaw>,
    sentences: Vec<SentenceRaw>,
}

#[derive(Serialize)]
struct DictEntryRaw {
    name: String,
    pos: String,
    canonical_pos: String,
    homophone: u8,
    being: bool,
    letter: bool,
    slang: bool,
    /// True if this is a stative verb ("be X").
    stative: bool,
    components: String,
}

#[derive(Serialize)]
struct SentenceRaw {
    entry_name: String,
    components: String,
}

fn main() {
    let data_dir = Path::new("../data");
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let out_path = Path::new(&out_dir).join("dictionary.json");

    let mut entries = Vec::new();
    let mut sentences = Vec::new();

    // Process each mem-*.xml file (skip header and footer).
    let mut xml_files: Vec<_> = fs::read_dir(data_dir)
        .expect("cannot read data/ directory")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            let name = p.file_name().unwrap().to_str().unwrap();
            name.starts_with("mem-")
                && name.ends_with(".xml")
                && name != "mem-00-header.xml"
                && name != "mem-29-footer.xml"
        })
        .collect();
    xml_files.sort();

    for path in &xml_files {
        println!("cargo:rerun-if-changed={}", path.display());
        let content = fs::read_to_string(path).expect("cannot read XML file");
        // XML files are fragments; wrap in a root element.
        let wrapped = format!("<root>{content}</root>");
        process_xml(&wrapped, &mut entries, &mut sentences);
    }

    let data = EmbeddedData { entries, sentences };
    let json = serde_json::to_string(&data).expect("JSON serialization failed");
    fs::write(&out_path, json).expect("cannot write dictionary.json");

    println!("cargo:rerun-if-changed=build.rs");
}

fn process_xml(xml: &str, entries: &mut Vec<DictEntryRaw>, sentences: &mut Vec<SentenceRaw>) {
    let mut reader = Reader::from_str(xml);

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) if e.name().as_ref() == b"table" => {
                let columns = read_table_columns(&mut reader);
                let entry_name = columns.get("entry_name").cloned().unwrap_or_default();
                let pos_raw = columns.get("part_of_speech").cloned().unwrap_or_default();
                let components = columns.get("components").cloned().unwrap_or_default();
                let definition = columns.get("definition").cloned().unwrap_or_default();

                if entry_name.is_empty() {
                    continue;
                }

                if pos_raw.starts_with("sen:") {
                    // Sentence entry. Skip templates — they have placeholders
                    // that cannot be parsed without being filled in.
                    if !components.is_empty() && !pos_raw.contains("tmpl") && !pos_raw.contains("archaic") {
                        sentences.push(SentenceRaw {
                            entry_name: entry_name.clone(),
                            components: components.clone(),
                        });
                    }
                } else if !is_affix(&pos_raw) {
                    // Dictionary stem entry.
                    let (base_pos, canonical_pos, homophone) = parse_pos(&pos_raw);
                    let being = pos_raw.contains("being");
                    let letter = definition.starts_with("the consonant ")
                        || definition.starts_with("the vowel ");
                    let slang = pos_raw.contains("slang");
                    let stative = pos_raw
                        .split(|c: char| c == ',' || c == ':')
                        .any(|t| t == "is");
                    entries.push(DictEntryRaw {
                        name: entry_name,
                        pos: base_pos,
                        canonical_pos,
                        homophone,
                        being,
                        letter,
                        slang,
                        stative,
                        components,
                    });
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => panic!("XML parse error: {e}"),
            _ => {}
        }
    }
}

/// Read all <column> children of the current <table> element.
fn read_table_columns(reader: &mut Reader<&[u8]>) -> HashMap<String, String> {
    let mut columns = HashMap::new();
    let mut depth = 1u32;
    let mut current_col_name: Option<String> = None;
    let mut current_text = String::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) => {
                depth += 1;
                if e.name().as_ref() == b"column" {
                    current_col_name = e
                        .attributes()
                        .filter_map(|a| a.ok())
                        .find(|a| a.key.as_ref() == b"name")
                        .map(|a| String::from_utf8_lossy(&a.value).to_string());
                    current_text.clear();
                }
            }
            Ok(Event::Text(ref e)) => {
                if current_col_name.is_some() {
                    current_text
                        .push_str(&e.unescape().unwrap_or_default());
                }
            }
            Ok(Event::End(ref e)) => {
                if e.name().as_ref() == b"column" {
                    if let Some(name) = current_col_name.take() {
                        columns.insert(name, current_text.clone());
                    }
                    current_text.clear();
                }
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => panic!("XML parse error in table: {e}"),
            _ => {}
        }
    }

    columns
}

/// Returns true for suffix and prefix entries (which are not dictionary stems).
fn is_affix(pos: &str) -> bool {
    pos.contains("suff") || pos.contains("pref")
}

// Tags that are internal metadata — stripped from canonical output.
const INTERNAL_TAGS: &[&str] = &[
    "alt", "klcp1", "t_c", "i_c", "is", "t", "i", "ambi", "anim", "being", "body", "deiv",
    "deri", "deriv", "epithet", "extcan", "fic", "food", "idiom", "nodict", "noanki", "person",
    "place", "plural", "reg", "terran", "weap", "inhpl", "inhps", "slang", "archaic", "hyp",
    "nolink",
];

/// Extract base POS, canonical POS (for output), and homophone index.
///
/// The base POS is just the first token (n, v, adv, etc.) used for morphological
/// matching. The canonical POS preserves the original structure but drops internal
/// tags, producing the format used in canonical components.
fn parse_pos(pos: &str) -> (String, String, u8) {
    // Split on colon to get the colon-separated parts.
    let colon_parts: Vec<&str> = pos.split(':').collect();
    let base = colon_parts.first().copied().unwrap_or(pos)
        .split(',').next().unwrap_or(pos);

    let mut homophone: u8 = 0;

    // Build canonical POS by filtering each colon-part's comma-sub-parts.
    let mut canonical_colon_parts = Vec::new();
    for (i, colon_part) in colon_parts.iter().enumerate() {
        let comma_parts: Vec<&str> = colon_part.split(',').collect();
        let mut kept = Vec::new();
        for cp in &comma_parts {
            let trimmed = cp.trim();
            if trimmed.is_empty() {
                continue;
            }
            // Extract homophone from numeric parts.
            if let Some(first_char) = trimmed.chars().next() {
                if first_char.is_ascii_digit() {
                    if let Ok(n) = trimmed
                        .trim_end_matches(|c: char| !c.is_ascii_digit())
                        .parse::<u8>()
                    {
                        homophone = n;
                    }
                    // Always keep numeric parts.
                    kept.push(trimmed);
                    continue;
                }
            }
            // Keep the base POS (first part) and non-internal tags.
            if i == 0 && kept.is_empty() {
                // Always keep the very first token (n, v, adv, etc.).
                kept.push(trimmed);
            } else if !INTERNAL_TAGS.contains(&trimmed) {
                kept.push(trimmed);
            }
        }
        if !kept.is_empty() {
            canonical_colon_parts.push(kept.join(","));
        }
    }

    // When a homophone number is present, "pro" and "name" are redundant — drop
    // them. Keep "num" so sentence-level rules can identify numbers.
    if homophone > 0 {
        for part in &mut canonical_colon_parts {
            let filtered: Vec<&str> = part
                .split(',')
                .filter(|s| *s != "pro" && *s != "name")
                .collect();
            *part = filtered.join(",");
        }
        canonical_colon_parts.retain(|p| !p.is_empty());
    }

    let canonical = canonical_colon_parts.join(":");
    (base.to_string(), canonical, homophone)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_pos_simple() {
        assert_eq!(parse_pos("n"), ("n".into(), "n".into(), 0));
        assert_eq!(parse_pos("v"), ("v".into(), "v".into(), 0));
        assert_eq!(parse_pos("adv"), ("adv".into(), "adv".into(), 0));
    }

    #[test]
    fn test_parse_pos_with_internal_tags() {
        // Internal tags are stripped from canonical.
        assert_eq!(parse_pos("n:being"), ("n".into(), "n".into(), 0));
        assert_eq!(parse_pos("v:t_c,klcp1"), ("v".into(), "v".into(), 0));
        assert_eq!(parse_pos("n:food"), ("n".into(), "n".into(), 0));
        assert_eq!(parse_pos("n:inhpl"), ("n".into(), "n".into(), 0));
    }

    #[test]
    fn test_parse_pos_with_semantic_tags() {
        // Tags like pro, name, num are preserved when no homophone.
        assert_eq!(parse_pos("n:pro"), ("n".into(), "n:pro".into(), 0));
        assert_eq!(
            parse_pos("n:name,klcp1"),
            ("n".into(), "n:name".into(), 0)
        );
        assert_eq!(parse_pos("n:num"), ("n".into(), "n:num".into(), 0));
        // pro is dropped when homophone is present.
        assert_eq!(
            parse_pos("n:1,pro,klcp1"),
            ("n".into(), "n:1".into(), 1)
        );
    }

    #[test]
    fn test_parse_pos_homophone() {
        assert_eq!(parse_pos("n:1"), ("n".into(), "n:1".into(), 1));
        assert_eq!(parse_pos("v:is,1"), ("v".into(), "v:1".into(), 1));
    }

    #[test]
    fn test_parse_pos_homophone_with_tag() {
        // Rare case: homophone + semantic tag preserved.
        assert_eq!(
            parse_pos("n:2,name"),
            ("n".into(), "n:2,name".into(), 2)
        );
    }

    #[test]
    fn test_is_affix() {
        assert!(is_affix("n:suff,klcp1"));
        assert!(is_affix("v:pref,klcp1"));
        assert!(!is_affix("n:alt"));
        assert!(!is_affix("n:being"));
        assert!(!is_affix("v:t_c"));
    }
}
