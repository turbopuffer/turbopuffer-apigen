use std::{collections::BTreeMap, error::Error, fmt, fs, process};

use clap::{Parser, ValueEnum};

use serde::Deserialize;

mod codegen;
mod util;

const TYPE_PREFIXES: &[&str] = &["Aggregate", "Expr", "Filter", "RankBy"];

#[derive(Parser)]
struct Args {
    /// The language to generate code for.
    #[arg(value_enum)]
    language: Language,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum Language {
    Python,
    Go,
    Typescript,
    Java,
}

impl fmt::Display for Language {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Language::Python => write!(f, "python"),
            Language::Go => write!(f, "go"),
            Language::Typescript => write!(f, "typescript"),
            Language::Java => write!(f, "java"),
        }
    }
}

fn main() {
    let args = Args::parse();
    if let Err(e) = run(args.language) {
        eprint!("error: {e}");
        let mut e = &*e;
        while let Some(source) = e.source() {
            eprint!(": {source}");
            e = source;
        }
        eprintln!();
        process::exit(1);
    }
}

pub fn run(language: Language) -> Result<(), Box<dyn Error>> {
    log!("generating code for {}", language);

    log!("reading Stainless stats file");
    let stats_content = fs::read_to_string(".stats.yml")?;
    let stainless_stats: StainlessStats = serde_yaml::from_str(&stats_content)?;

    let openapi_yaml = if let Ok(spec_file_path) = std::env::var("SPEC_FILE_PATH") {
        log!("reading OpenAPI spec from local file: {}", spec_file_path);
        fs::read_to_string(&spec_file_path)?
    } else {
        log!(
            "discovered OpenAPI spec url: {}",
            stainless_stats.openapi_spec_url
        );
        log!("downloading OpenAPI spec");
        let resp = reqwest::blocking::get(&stainless_stats.openapi_spec_url)?;
        resp.text()?
    };

    log!("parsing OpenAPI spec");
    let mut openapi: serde_yaml::Value = serde_yaml::from_str(&openapi_yaml)?;
    let openapi_schemas = openapi["components"]["schemas"]
        .as_mapping_mut()
        .ok_or_else(|| "no schemas found in OpenAPI spec")?;

    let mut parsed_schemas = BTreeMap::new();
    for (k, v) in openapi_schemas {
        let k = k.as_str().unwrap();
        if !TYPE_PREFIXES.iter().any(|prefix| k.starts_with(prefix)) {
            continue;
        }
        let schema = serde_yaml::from_value(v.clone())?;
        parsed_schemas.insert(k.to_string(), schema);
    }

    let content = match language {
        Language::Go => codegen::go::render(parsed_schemas)?,
        Language::Java => codegen::java::render(parsed_schemas)?,
        Language::Python => codegen::python::render(parsed_schemas)?,
        Language::Typescript => codegen::typescript::render(parsed_schemas)?,
    };

    print!("{}", content.into_string());

    Ok(())
}

#[derive(Debug, Deserialize)]
struct StainlessStats {
    openapi_spec_url: String,
}
