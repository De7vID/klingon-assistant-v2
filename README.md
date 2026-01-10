# kla-parser

A Klingon morphological parser that decomposes words and sentences into
their constituent morphemes (prefixes, stems, and suffixes), returning
ranked multiple hypotheses with confidence scores.

The dictionary (~3,000 stems) is compiled from the
[klingon-assistant-data](https://github.com/De7vID/klingon-assistant-data)
XML database and embedded at build time — no external files are needed
at runtime.

## Building

Requires Rust 1.70+ and the `data/` git submodule:

```bash
git submodule update --init
cargo build --release
```

## Usage

```
kla-parser [OPTIONS] [INPUT]

Arguments:
  [INPUT]  Klingon text to parse. If omitted, reads from stdin.

Options:
      --bracketed  Output in bracketed {word:pos} format
      --all        Show all hypotheses per word (confidence >= 0)
      --raw        Show all hypotheses unfiltered, including invalid parses
      --pretty     Pretty-print JSON output
  -n, --top <N>    Show top N hypotheses per word [default: 1]
```

### Examples

Parse a sentence in bracketed format:

```
$ echo "batlh bIHeghjaj." | kla-parser --bracketed
{batlh:adv}, {bI-:v}, {Hegh:v}, {-jaj:v}
```

Decompose a noun with suffixes:

```
$ echo "puqpu'wI'" | kla-parser --bracketed
{puq:n}, {-pu':n}, {-wI':n}
```

Full JSON output for a single word:

```
$ kla-parser --pretty "Qapla'"
{
  "word": "Qapla'",
  "hypotheses": [
    {
      "components": [
        {
          "text": "Qapla'",
          "entry_name": "Qapla'",
          "pos": "n",
          "role": "stem"
        }
      ],
      "confidence": 80.0,
      "parse_type": "wholeword"
    }
  ]
}
```

Stdin mode processes one sentence per line (NDJSON output).

## How it works

For each word, three families of hypotheses are generated:

1. **Whole-word** — direct dictionary lookup.
2. **Verb** — try each of the 29 verb prefixes, then enumerate all valid
   suffix chains (types 1-9, rovers, and the type-R slots) using an
   iterative work queue.
3. **Noun** — enumerate noun suffix chains (types 1-5) without prefixes.

All valid decompositions are kept, scored by confidence (dictionary
match, POS agreement, parsimony), and returned ranked. Multi-word
compound entries (e.g., `tlhIngan Hol`) and compound-plus-suffix forms
are also checked.

## Project structure

```
Cargo.toml                  Workspace manifest
kla-parser-lib/         Library crate
  build.rs                  Compiles XML dictionary to embedded JSON
  src/
    lib.rs
    types.rs                Core data types
    dictionary.rs           Embedded dictionary lookup
    morphology.rs           Multi-hypothesis suffix-stripping
    confidence.rs           Scoring and ranking
    sentence.rs             Sentence splitting and per-word parsing
    output.rs               JSON and bracketed formatters
kla-parser/             Binary crate (CLI)
  src/main.rs
data/                       Dictionary submodule (klingon-assistant-data)
```

## Testing

```bash
cargo test                       # All tests (unit, regression, corpus training)
cargo test -- --ignored          # Corpus holdout set (20%)
```

The corpus test parses ~270 sentences with known bracketed decompositions
and compares results. To see accuracy percentages, run with output visible:

```bash
cargo test corpus_training -- --nocapture
cargo test corpus_holdout -- --ignored --nocapture
```

This prints top-1 accuracy (best hypothesis matches) and any-hypothesis
accuracy (correct parse exists among all hypotheses). Current training
accuracy: top-1 ~76%, any ~98%.

## Licence

Apache 2.0. See [LICENSE](LICENSE).

The dictionary data submodule has its own licence; see `data/LICENSE`.
