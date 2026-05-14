// Regression tests for known edge cases.

use kla_parser_lib::output::to_bracketed;
use kla_parser_lib::types::ParseType;
use kla_parser_lib::{parse_word, Dictionary};

fn dict() -> Dictionary {
    Dictionary::new()
}

#[test]
fn qapla_is_whole_word() {
    let d = dict();
    let result = parse_word("Qapla'", &d);
    let top = &result.hypotheses[0];
    assert_eq!(
        top.parse_type,
        ParseType::WholeWord,
        "Qapla' should be WholeWord, not decomposed as Qap + -la'"
    );
    assert_eq!(top.components[0].entry_name, "Qapla'");
}

#[test]
fn lolah_has_both_hypotheses() {
    let d = dict();
    let result = parse_word("lo'laH", &d);
    let has_whole = result
        .hypotheses
        .iter()
        .any(|h| h.parse_type == ParseType::WholeWord);
    let has_verb = result.hypotheses.iter().any(|h| {
        h.parse_type == ParseType::Verb
            && h.components.iter().any(|c| c.entry_name == "lo'")
            && h.components.iter().any(|c| c.entry_name == "-laH")
    });
    assert!(has_whole, "lo'laH should have a WholeWord hypothesis");
    assert!(
        has_verb,
        "lo'laH should have a verb decomposition lo' + -laH"
    );
}

#[test]
fn puqpuwij_noun_decomposition() {
    let d = dict();
    let result = parse_word("puqpu'wIj", &d);
    let noun = result
        .hypotheses
        .iter()
        .find(|h| h.parse_type == ParseType::Noun)
        .expect("should have noun hypothesis");
    assert_eq!(noun.components[0].entry_name, "puq");
    assert_eq!(noun.components[1].entry_name, "-pu'");
    assert_eq!(noun.components[2].entry_name, "-wIj");
}

#[test]
fn yajbequ_verb_with_rovers() {
    let d = dict();
    let result = parse_word("yajbe'qu'", &d);
    let verb = result
        .hypotheses
        .iter()
        .find(|h| {
            h.parse_type == ParseType::Verb
                && h.components.len() == 3
                && h.components[0].entry_name == "yaj"
        })
        .expect("should have verb with two rovers");
    assert_eq!(verb.components[1].entry_name, "-be'");
    assert_eq!(verb.components[2].entry_name, "-qu'");
}

#[test]
fn batlh_bihegh_jaj_bracketed() {
    let d = dict();
    let parse = kla_parser_lib::parse_sentence("batlh bIHeghjaj.", &d);
    let bracketed = kla_parser_lib::output::sentence_to_bracketed(&parse);
    assert_eq!(bracketed, "{batlh:adv}, {bI-:v}, {Hegh:v}, {-jaj:v}");
}

#[test]
fn ghach_without_suffix_warns() {
    let d = dict();
    // {lobghach} = {lob} + {-ghach}, but no other verb suffix before {-ghach}.
    let result = parse_word("lobghach", &d);
    let verb = result
        .hypotheses
        .iter()
        .find(|h| {
            h.parse_type == ParseType::Verb
                && h.components.iter().any(|c| c.entry_name == "-ghach")
        });
    if let Some(v) = verb {
        assert!(
            v.warnings
                .iter()
                .any(|w| w.contains("-ghach")),
            "-ghach without preceding suffix should warn"
        );
    }
}

#[test]
fn boqwi_has_homophone() {
    let d = dict();
    let result = parse_word("boQwI'", &d);
    // {boQwI'} could be WholeWord or {boQ} + {-wI'}. Check that we have both.
    let has_decomposition = result.hypotheses.iter().any(|h| {
        h.components.len() > 1
            && h.components.iter().any(|c| c.entry_name == "boQ")
            && h.components.iter().any(|c| c.entry_name == "-wI'")
    });
    assert!(
        has_decomposition,
        "boQwI' should have a decomposition into boQ + -wI'"
    );
}

#[test]
fn simple_whole_word() {
    let d = dict();
    let result = parse_word("tlhIngan", &d);
    let top = &result.hypotheses[0];
    assert_eq!(top.parse_type, ParseType::WholeWord);
    assert_eq!(top.components[0].entry_name, "tlhIngan");
}

#[test]
fn bracketed_output_format() {
    let d = dict();
    let result = parse_word("jIyaj", &d);
    let verb = result
        .hypotheses
        .iter()
        .find(|h| h.parse_type == ParseType::Verb)
        .expect("should have verb hypothesis");
    let bracketed = to_bracketed(verb);
    assert_eq!(bracketed, "{jI-:v}, {yaj:v}");
}

// ---------------------------------------------------------------------------
// Adjectival verb hypotheses
// ---------------------------------------------------------------------------

#[test]
fn posdaq_adjectival_verb() {
    let d = dict();
    let result = parse_word("poSDaq", &d);
    // Should have an Adjectival hypothesis: {poS} (verb) + {-Daq} (noun).
    let adj = result
        .hypotheses
        .iter()
        .find(|h| h.parse_type == ParseType::Adjectival)
        .expect("poSDaq should have an adjectival hypothesis");
    assert_eq!(adj.components[0].entry_name, "poS");
    assert_eq!(adj.components[1].entry_name, "-Daq");
    assert_eq!(adj.components[1].pos, "n");
    // Adjectival should rank above the noun parse.
    let top = &result.hypotheses[0];
    assert_eq!(
        top.parse_type,
        ParseType::Adjectival,
        "adjectival should outrank noun for poSDaq"
    );
}

#[test]
fn tinqudaq_adjectival_with_qu() {
    let d = dict();
    let result = parse_word("tInqu'Daq", &d);
    // Should have an Adjectival hypothesis: {tIn} (verb) + {-qu'} (rover) + {-Daq} (noun).
    let adj = result
        .hypotheses
        .iter()
        .find(|h| {
            h.parse_type == ParseType::Adjectival
                && h.components.len() == 3
                && h.components[0].entry_name == "tIn"
        })
        .expect("tInqu'Daq should have adjectival hypothesis with -qu'");
    assert_eq!(adj.components[1].entry_name, "-qu'");
    assert_eq!(adj.components[1].pos, "v");
    assert_eq!(adj.components[2].entry_name, "-Daq");
    assert_eq!(adj.components[2].pos, "n");
}

#[test]
fn ngadhadaq_adjectival_with_ha() {
    let d = dict();
    let result = parse_word("ngaDHa'Daq", &d);
    // Should have an Adjectival hypothesis: {ngaD} (verb) + {-Ha'} (suffix) + {-Daq} (noun).
    let adj = result
        .hypotheses
        .iter()
        .find(|h| {
            h.parse_type == ParseType::Adjectival
                && h.components.iter().any(|c| c.entry_name == "ngaD")
                && h.components.iter().any(|c| c.entry_name == "-Ha'")
                && h.components.iter().any(|c| c.entry_name == "-Daq")
        })
        .expect("ngaDHa'Daq should have adjectival hypothesis with -Ha'");
    assert_eq!(adj.components[0].entry_name, "ngaD");
    assert_eq!(adj.components[1].entry_name, "-Ha'");
    assert_eq!(adj.components[2].entry_name, "-Daq");
}

#[test]
fn nivdaq_adjectival_verb_only_stem() {
    let d = dict();
    let result = parse_word("nIvDaq", &d);
    // {nIv} is verb-only, so adjectival should rank first (no competing noun POS_MATCH).
    let adj = result
        .hypotheses
        .iter()
        .find(|h| h.parse_type == ParseType::Adjectival)
        .expect("nIvDaq should have an adjectival hypothesis");
    assert_eq!(adj.components[0].entry_name, "nIv");
    assert_eq!(adj.components[1].entry_name, "-Daq");
    let top = &result.hypotheses[0];
    assert_eq!(
        top.parse_type,
        ParseType::Adjectival,
        "adjectival should be top hypothesis for verb-only stem nIv"
    );
}

// ---------------------------------------------------------------------------
// Transparent plural and slang deprioritisation
// ---------------------------------------------------------------------------

#[test]
fn nuhmey_decomposition_preferred() {
    let d = dict();
    let result = parse_word("nuHmey", &d);
    // {nuHmey} is a convenience whole-word entry for "arsenal"; the decomposition
    // {nuH} + {-mey} should rank first because {-mey} is a transparent plural.
    let top = &result.hypotheses[0];
    assert_eq!(
        top.parse_type,
        ParseType::Noun,
        "nuHmey should prefer noun decomposition over whole word"
    );
    assert_eq!(top.components[0].entry_name, "nuH");
    assert_eq!(top.components[1].entry_name, "-mey");
}

#[test]
fn ghewmey_slang_deprioritised() {
    let d = dict();
    let result = parse_word("ghewmey", &d);
    // {ghewmey} is a slang whole-word entry; decomposition {ghew} + {-mey} should win.
    let top = &result.hypotheses[0];
    assert_eq!(
        top.parse_type,
        ParseType::Noun,
        "ghewmey should prefer noun decomposition over slang whole word"
    );
    assert_eq!(top.components[0].entry_name, "ghew");
    assert_eq!(top.components[1].entry_name, "-mey");
}

#[test]
fn suvwi_whole_word_preserved() {
    let d = dict();
    let result = parse_word("SuvwI'", &d);
    // {SuvwI'} (warrior) is a genuine whole-word entry and should NOT be
    // deprioritised - its last sub_component is {-wI'}, not a plural suffix.
    let top = &result.hypotheses[0];
    assert_eq!(
        top.parse_type,
        ParseType::WholeWord,
        "SuvwI' should remain a whole-word hypothesis"
    );
    assert_eq!(top.components[0].entry_name, "SuvwI'");
}

// ---------------------------------------------------------------------------
// Noun-verb disambiguation (OVS word order)
// ---------------------------------------------------------------------------

#[test]
fn qagh_hoh_verb_after_noun() {
    let d = dict();
    let parse = kla_parser_lib::parse_sentence("qagh HoH", &d);
    let bracketed = kla_parser_lib::output::sentence_to_bracketed(&parse);
    assert_eq!(bracketed, "{qagh:n}, {HoH:v}");
}

#[test]
fn pronoun_after_noun_stays_noun() {
    // {tlhIngan jIH} = "I am a Klingon" - {jIH} is a pronoun-as-copula, not a verb.
    let d = dict();
    let parse = kla_parser_lib::parse_sentence("tlhIngan jIH", &d);
    let bracketed = kla_parser_lib::output::sentence_to_bracketed(&parse);
    assert_eq!(bracketed, "{tlhIngan:n}, {jIH:n:1}");
}

#[test]
fn noun_after_number_stays_noun() {
    // {wa' ram} = "one night" - {ram} is "night" (noun), not "be trivial" (verb).
    let d = dict();
    let parse = kla_parser_lib::parse_sentence("wa' ram", &d);
    let bracketed = kla_parser_lib::output::sentence_to_bracketed(&parse);
    assert_eq!(bracketed, "{wa':n:num}, {ram:n}");
}

#[test]
fn noun_after_possessive_stays_noun() {
    // {Hab SoSlI' Quch} = "Your mother has a smooth forehead" - {Quch} is
    // "forehead" (noun), not "be happy" (verb), after the possessive NP {SoSlI'}.
    let d = dict();
    let parse = kla_parser_lib::parse_sentence("Hab SoSlI' Quch", &d);
    let bracketed = kla_parser_lib::output::sentence_to_bracketed(&parse);
    assert_eq!(bracketed, "{Hab:v}, {SoS:n}, {-lI':n}, {Quch:n}");
}

// ---------------------------------------------------------------------------
// Sentence-level disambiguation ({je}, {pagh}, {wej}, number hyphenation)
// ---------------------------------------------------------------------------

#[test]
fn je_sentence_final_is_conj() {
    // {beqDaj je} at end of sentence = "and his/her crew" - conjunction, not adverb.
    let d = dict();
    let parse = kla_parser_lib::parse_sentence("quv lughaj beqDaj je", &d);
    let bracketed = kla_parser_lib::output::sentence_to_bracketed(&parse);
    assert!(
        bracketed.ends_with("{je:conj}"),
        "sentence-final {{je}} should be conjunction, got: {bracketed}"
    );
}

#[test]
fn pagh_between_verbs_is_conj() {
    // {taH pagh taHbe'} = "to be or not to be".
    let d = dict();
    let parse = kla_parser_lib::parse_sentence("taH pagh taHbe'", &d);
    let bracketed = kla_parser_lib::output::sentence_to_bracketed(&parse);
    assert_eq!(bracketed, "{taH:v:1}, {pagh:conj}, {taH:v:1}, {-be':v}");
}

#[test]
fn wej_after_number_is_noun() {
    // {wa'maH wej} = "thirteen".
    let d = dict();
    let parse = kla_parser_lib::parse_sentence("wa'maH wej", &d);
    let bracketed = kla_parser_lib::output::sentence_to_bracketed(&parse);
    assert_eq!(bracketed, "{wa'maH:n:num}, {wej:n:num}");
}

#[test]
fn wej_before_verb_stays_adverb() {
    // {wej Sop} = "S/he does not yet eat" - {wej:adv} ("not yet") rather
    // than {wej:n:num} ("three"). Guards the POS_FREQ_BASELINE in
    // confidence scoring: without the baseline, a marginal corpus POS
    // count for {wej:n} (~2) would beat {wej:adv} (~1) and flip the
    // ranking.
    let d = dict();
    let parse = kla_parser_lib::parse_sentence("wej Sop", &d);
    let bracketed = kla_parser_lib::output::sentence_to_bracketed(&parse);
    assert_eq!(bracketed, "{wej:adv}, {Sop:v}");
}

#[test]
fn hyphenated_number_prefers_num() {
    // Digits in a hyphenated number sequence should use the "num" reading.
    let d = dict();
    let parse = kla_parser_lib::parse_sentence("pagh-pagh-DoD-cha' yInab", &d);
    let bracketed = kla_parser_lib::output::sentence_to_bracketed(&parse);
    assert!(
        bracketed.contains("{pagh:n:2,num}"),
        "hyphenated {{pagh}} should be n:2,num (digit zero), got: {bracketed}"
    );
}

// ---------------------------------------------------------------------------
// Prefix-transitivity disambiguation
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Multi-word dictionary entries inside larger sentences
// ---------------------------------------------------------------------------

#[test]
fn sentence_entry_with_punctuation_matches_inside_input() {
    // {tlhIngan maH!:sen} is a sentence entry whose entry_name carries the
    // trailing {!}. When it appears as a leading clause inside a larger
    // input ({tlhIngan maH! taHjaj!}), the compound matcher should retry
    // the lookup with the stripped punctuation re-appended, and the result
    // should not treat the entire input as one compound.
    let d = dict();
    let parse = kla_parser_lib::parse_sentence("tlhIngan maH! taHjaj!", &d);
    let bracketed = kla_parser_lib::output::sentence_to_bracketed(&parse);
    assert_eq!(bracketed, "{tlhIngan maH!:sen}, {taH:v:1}, {-jaj:v}");
}

#[test]
fn wipustah_transitive_homophone() {
    let d = dict();
    let result = parse_word("wIpuStaH", &d);
    // {wI-} requires a direct object, so stative {puS:v:1} "be few" is invalid.
    // {puS:v:2} "sight (with gunsight)" should win.
    let top = &result.hypotheses[0];
    assert!(
        top.components
            .iter()
            .any(|c| c.entry_name == "puS" && c.homophone == Some(2)),
        "wIpuStaH should prefer transitive puS:v:2 over stative puS:v:1"
    );
}
