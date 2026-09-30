use std::collections::{BTreeMap, HashSet};
use std::error::Error;
use std::path::Path;
use std::{env, fs};

use clap::{Parser, Subcommand};
use serde::Deserialize;

const CLDR: &str = "https://raw.githubusercontent.com/unicode-org/cldr/release-48";

#[derive(Subcommand)]
enum Command {
    /// Update bundled emoji search annotations from Unicode CLDR
    Emoji,
}

#[derive(Parser)]
#[command(about = "Development tasks for Relvi")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Deserialize)]
struct Annotation {
    #[serde(rename = "@cp")]
    glyph: String,
    #[serde(rename = "@type")]
    kind: Option<String>,
    #[serde(rename = "$text")]
    value: String,
}

#[derive(Deserialize)]
struct Annotations {
    #[serde(rename = "annotation")]
    entries: Vec<Annotation>,
}

#[derive(Deserialize)]
struct Document {
    annotations: Annotations,
}

fn generate_emoji() -> Result<(), Box<dyn Error>> {
    let glyphs: HashSet<_> = emojis::iter()
        .map(|emoji| emoji.as_str().replace('\u{fe0f}', ""))
        .collect();
    let mut data: BTreeMap<String, (String, Vec<String>)> = BTreeMap::new();

    for language in ["en", "zh"] {
        for collection in ["annotations", "annotationsDerived"] {
            let url = format!("{CLDR}/common/{collection}/{language}.xml");
            println!("Reading {url}");
            let xml = ureq::get(&url).call()?.body_mut().read_to_string()?;
            let document: Document = quick_xml::de::from_str(&xml)?;

            for annotation in document.annotations.entries {
                if !glyphs.contains(&annotation.glyph) {
                    continue;
                }
                let (name, keywords) = data.entry(annotation.glyph).or_default();
                if annotation.kind.as_deref() == Some("tts") {
                    if language == "zh" {
                        *name = annotation.value;
                    }
                } else {
                    for word in annotation.value.split(" | ") {
                        if !keywords.iter().any(|keyword| keyword == word) {
                            keywords.push(word.to_owned());
                        }
                    }
                }
            }
        }
    }

    let data: BTreeMap<_, _> = data
        .into_iter()
        .map(|(glyph, (name, keywords))| (glyph, [name, keywords.join("\n")]))
        .collect();
    let license = ureq::get(&format!("{CLDR}/LICENSE"))
        .call()?
        .body_mut()
        .read_to_string()?;
    let mut json = serde_json::to_string_pretty(&data)?;
    json.push('\n');

    fs::write("resources/emoji/annotations.json", json)?;
    fs::write("resources/emoji/LICENSE", license)?;
    println!("Saved annotations for {} emoji", data.len());

    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    env::set_current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap())?;
    match Cli::parse().command {
        Command::Emoji => generate_emoji(),
    }
}
