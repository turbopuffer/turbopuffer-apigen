use std::{collections::BTreeMap, error::Error};

use monostate::MustBe;
use serde::Deserialize;

pub mod go;
pub mod java;
pub mod python;
pub mod typescript;

mod shared;

const SCHEMA_REF_PREFIX: &str = "#/components/schemas/";

#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
#[serde(deny_unknown_fields)]
#[serde(rename_all_fields = "camelCase")]
pub enum OpenApiSchema {
    AnyOf {
        #[serde(rename = "description")]
        _description: Option<String>,
        any_of: Vec<OpenApiSchema>,
    },
    Object {
        #[serde(rename = "description")]
        _description: Option<String>,
        #[serde(rename = "type")]
        _type: MustBe!("object"),
        properties: BTreeMap<String, OpenApiSchema>,
        #[serde(default)]
        required: Vec<String>,
        #[serde(rename = "title")]
        title: Option<String>,
    },
    ArrayList {
        description: Option<String>,
        #[serde(rename = "type")]
        _type: MustBe!("array"),
        items: Box<OpenApiSchema>,
        #[serde(rename = "title")]
        title: Option<String>,
    },
    ArrayTuple {
        #[serde(rename = "description")]
        _description: Option<String>,
        #[serde(rename = "type")]
        _type: MustBe!("array"),
        /// XXX: this should be called `items` according to the current version
        /// of the JSON Schema spec, but Stainless chokes on that. So we use
        /// the older spelling of `additionalItems`, which Stainless just
        /// ignores.
        #[serde(default = "default_true")]
        additional_items: bool,
        prefix_items: Vec<OpenApiSchema>,
        /// When used in an `anyOf` schema, the name to use for the variant (if
        /// the target language requires variants to be explicitly named). If
        /// omitted, the name is derived from the `const` value in the tuple,
        /// if there is a single `const` value.
        #[serde(rename = "x-turbopuffer-variant-name")]
        x_turbopuffer_variant_name: Option<String>,
        /// When used in an `anyOf` schema in a language that requires variants
        /// to be explicitly named, whether to drop the variant if its name
        /// conflicts with another variant's name. Useful for variants that
        /// are nonessential (e.g., alternate orderings of another variant)
        /// and therefore can be safely omitted in languages that require
        /// explicit naming of variants.
        #[serde(rename = "x-turbopuffer-variant-drop-on-conflict")]
        #[serde(default)]
        x_turbopuffer_variant_drop_on_conflict: bool,
        #[serde(rename = "title")]
        title: Option<String>,
    },
    String {
        #[serde(rename = "description")]
        _description: Option<String>,
        #[serde(rename = "type")]
        _type: MustBe!("string"),
        #[serde(rename = "title")]
        title: Option<String>,
    },
    Number {
        #[serde(rename = "description")]
        _description: Option<String>,
        #[serde(rename = "type")]
        _type: MustBe!("number"),
        #[serde(rename = "title")]
        title: Option<String>,
        #[serde(rename = "x-turbopuffer-width")]
        x_turbopuffer_width: Option<u32>,
    },
    Boolean {
        #[serde(rename = "description")]
        _description: Option<String>,
        #[serde(rename = "type")]
        _type: MustBe!("boolean"),
        #[serde(rename = "title")]
        title: Option<String>,
    },
    Const {
        #[serde(rename = "description")]
        _description: Option<String>,
        #[serde(rename = "const")]
        sconst: String,
        #[serde(rename = "title")]
        title: Option<String>,
    },
    Ref {
        #[serde(rename = "$ref")]
        sref: String,
        #[serde(rename = "title")]
        title: Option<String>,
    },
    Any {
        #[serde(rename = "description")]
        _description: Option<String>,
        #[serde(rename = "x-stainless-any")]
        _x_stainless_any: Option<MustBe!(true)>,
        #[serde(rename = "title")]
        title: Option<String>,
    },
}

impl OpenApiSchema {
    pub fn title(&self) -> Option<&str> {
        match self {
            OpenApiSchema::AnyOf { .. } => None,
            OpenApiSchema::String { title, .. }
            | OpenApiSchema::Boolean { title, .. }
            | OpenApiSchema::Number { title, .. }
            | OpenApiSchema::Const { title, .. }
            | OpenApiSchema::Ref { title, .. }
            | OpenApiSchema::Any { title, .. }
            | OpenApiSchema::ArrayTuple { title, .. }
            | OpenApiSchema::Object { title, .. }
            | OpenApiSchema::ArrayList { title, .. } => title.as_deref(),
        }
    }

    pub fn title_mut(&mut self) -> Option<&mut Option<String>> {
        match self {
            OpenApiSchema::AnyOf { .. } => None,
            OpenApiSchema::String { title, .. }
            | OpenApiSchema::Boolean { title, .. }
            | OpenApiSchema::Number { title, .. }
            | OpenApiSchema::Const { title, .. }
            | OpenApiSchema::Ref { title, .. }
            | OpenApiSchema::Any { title, .. }
            | OpenApiSchema::ArrayTuple { title, .. }
            | OpenApiSchema::Object { title, .. }
            | OpenApiSchema::ArrayList { title, .. } => Some(title),
        }
    }
}

fn default_true() -> bool {
    true
}

fn strip_schema_ref_prefix(sref: &str) -> Result<&str, Box<dyn Error>> {
    sref.strip_prefix(SCHEMA_REF_PREFIX)
        .ok_or_else(|| format!("unsupported reference: {sref}").into())
}
