use std::io::{self, BufRead};

use clap::Parser;
use kla_parser_lib::{parse_sentence, parse_word, Dictionary, WordParse};

#[derive(Parser)]
#[command(name = "kla-parser", about = "Klingon morphological parser")]
struct Cli {
    /// Klingon text to parse. If omitted, reads from stdin.
    input: Option<String>,

    /// Output in bracketed {word:pos} format.
    #[arg(long)]
    bracketed: bool,

    /// Show all hypotheses per word (filters out negative confidence).
    #[arg(long)]
    all: bool,

    /// Show all hypotheses unfiltered, including invalid parses.
    #[arg(long)]
    raw: bool,

    /// Pretty-print JSON output.
    #[arg(long)]
    pretty: bool,

    /// Show top N hypotheses per word.
    #[arg(short = 'n', long = "top", default_value = "1")]
    top: usize,
}

fn main() {
    let cli = Cli::parse();
    let dict = Dictionary::new();

    match cli.input {
        Some(ref text) => process_line(text, &dict, &cli),
        None => {
            let stdin = io::stdin();
            for line in stdin.lock().lines() {
                let line = line.expect("failed to read stdin");
                if !line.trim().is_empty() {
                    process_line(&line, &dict, &cli);
                }
            }
        }
    }
}

/// Strip trailing punctuation from a single word, matching the same set of
/// characters that `split_sentence` in the library strips.
fn strip_trailing_punct(word: &str) -> &str {
    word.trim_end_matches(['.', '!', '?', ',', ';'])
}

fn process_line(line: &str, dict: &Dictionary, cli: &Cli) {
    // Determine whether this is a sentence (multiple words) or a single word.
    let is_sentence = line.split_whitespace().count() > 1;

    if cli.bracketed {
        if is_sentence {
            let parse = parse_sentence(line, dict);
            println!(
                "{}",
                kla_parser_lib::output::sentence_to_bracketed(&parse)
            );
        } else {
            let word = strip_trailing_punct(line.trim());
            let wp = parse_word(word, dict);
            if let Some(h) = wp.hypotheses.first() {
                println!("{}", kla_parser_lib::output::to_bracketed(h));
            }
        }
    } else if is_sentence {
        let parse = parse_sentence(line, dict);
        // Trim hypotheses per the --top / --all flags.
        let trimmed = trim_sentence_parse(parse, cli);
        let json = if cli.pretty {
            serde_json::to_string_pretty(&trimmed)
        } else {
            serde_json::to_string(&trimmed)
        };
        println!("{}", json.expect("JSON serialization failed"));
    } else {
        let word = strip_trailing_punct(line.trim());
        let wp = parse_word(word, dict);
        let trimmed = trim_word_parse(wp, cli);
        let json = if cli.pretty {
            serde_json::to_string_pretty(&trimmed)
        } else {
            serde_json::to_string(&trimmed)
        };
        println!("{}", json.expect("JSON serialization failed"));
    }
}

fn trim_word_parse(mut wp: WordParse, cli: &Cli) -> WordParse {
    if cli.raw {
        // Keep everything.
    } else if cli.all {
        wp.hypotheses.retain(|h| h.confidence >= 0.0);
    } else {
        wp.hypotheses.truncate(cli.top);
    }
    wp
}

fn trim_sentence_parse(
    mut sp: kla_parser_lib::SentenceParse,
    cli: &Cli,
) -> kla_parser_lib::SentenceParse {
    for wp in &mut sp.words {
        if cli.raw {
            // Keep everything.
        } else if cli.all {
            wp.hypotheses.retain(|h| h.confidence >= 0.0);
        } else {
            wp.hypotheses.truncate(cli.top);
        }
    }
    sp
}
