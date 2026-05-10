use std::{collections::BTreeMap, error::Error, mem};

use crate::codegen::{OpenApiSchema, SCHEMA_REF_PREFIX};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ConflictBehavior {
    Drop,
    AppendSuffix,
}

pub fn extract_any_of_tuples(
    schemas: &mut BTreeMap<String, OpenApiSchema>,
    conflict_behavior: ConflictBehavior,
) -> Result<(), Box<dyn Error>> {
    // Extract named types for any extractable variants inside of a top-level
    // `anyOf`, and replace the variants with references to the new types. We
    // only have limited support for rendering `anyOf`s in Go and Java, and this
    // gives us a chance to fall into `render_any_of_refs` when we later attempt
    // to render the `anyOf`.
    //
    // The following anyOf variants are extracted:
    //
    //   - `ArrayTuple` schemas with a single `Const` in `prefixItems`: the
    //     variant name is derived from `x-turbopuffer-variant-name` or the
    //     `Const` value.
    //   - `String` schemas: the variant name must be supplied explicitly via
    //     `x-turbopuffer-variant-name`.
    //   - `Map` schemas matching the alias-tuple pattern (`Map<String, [Const,
    //     attr]>`): the variant name is derived from
    //     `x-turbopuffer-variant-name` or the inner tuple's `Const` value.

    let mut new_schemas = BTreeMap::new();
    for (name, schema) in &mut *schemas {
        if let OpenApiSchema::AnyOf {
            _description: _,
            any_of,
        } = schema
        {
            if conflict_behavior == ConflictBehavior::Drop {
                any_of.retain(|item| {
                    !matches!(
                        item,
                        OpenApiSchema::ArrayTuple {
                            x_turbopuffer_variant_drop_on_conflict: true,
                            ..
                        }
                    )
                });
            }
            for item in any_of {
                let Some(suffix) = extractable_variant_suffix(item) else {
                    continue;
                };
                let ref_title = format!("{name}{suffix}");
                let mut name = format!("{name}{suffix}");
                if conflict_behavior == ConflictBehavior::AppendSuffix {
                    let mut new_name = name.clone();
                    let mut counter = 2;
                    while new_schemas.contains_key(&new_name) {
                        new_name = format!("{name}{counter}");
                        counter += 1;
                    }
                    name = new_name;
                }
                let item = mem::replace(
                    item,
                    OpenApiSchema::Ref {
                        sref: format!("{SCHEMA_REF_PREFIX}{name}"),
                        title: Some(ref_title),
                    },
                );
                if new_schemas.insert(name.clone(), item).is_some() {
                    Err(format!(
                        "extraction of variants from anyOf failed: duplicate schema name: {name}"
                    ))?
                }
            }
        }
    }
    for (name, schema) in new_schemas {
        if schemas.insert(name.clone(), schema).is_some() {
            Err(format!(
                "extraction of variants from anyOf failed: duplicate schema name: {name}"
            ))?
        }
    }
    Ok(())
}

/// Returns the variant name suffix that should be appended to the parent
/// `anyOf` schema's name when extracting `schema` to a top-level type.
///
/// Returns `None` if the variant is not extractable (e.g., a `Ref`, or an
/// `ArrayTuple` with no single `Const`).
fn extractable_variant_suffix(schema: &OpenApiSchema) -> Option<String> {
    match schema {
        OpenApiSchema::ArrayTuple {
            prefix_items,
            x_turbopuffer_variant_name,
            ..
        } => {
            if let Some(name) = x_turbopuffer_variant_name {
                return Some(name.clone());
            }
            single_const(prefix_items).map(|s| s.to_owned())
        }
        OpenApiSchema::String {
            x_turbopuffer_variant_name,
            ..
        } => x_turbopuffer_variant_name.clone(),
        OpenApiSchema::Map {
            x_turbopuffer_variant_name,
            ..
        } => x_turbopuffer_variant_name.clone(),
        _ => None,
    }
}

/// Returns the value of the only `Const` in `prefix_items`, or `None` if there
/// is no `Const` or more than one.
fn single_const(prefix_items: &[OpenApiSchema]) -> Option<&str> {
    let mut sconsts = prefix_items.iter().filter_map(|item| match item {
        OpenApiSchema::Const { sconst, .. } => Some(sconst.as_str()),
        _ => None,
    });
    let sconst = sconsts.next()?;
    if sconsts.next().is_some() {
        return None;
    }
    Some(sconst)
}

#[derive(Debug, Clone)]
pub enum TupleField<'a> {
    Normal {
        name: String,
        schema: &'a OpenApiSchema,
    },
    Const(&'a str),
}

fn build_tuple_fields_inner<'a>(
    out: &mut Vec<TupleField<'a>>,
    i: &mut usize,
    prefix_items: &'a [OpenApiSchema],
) {
    for item in prefix_items {
        match item {
            OpenApiSchema::Const { sconst, .. } => {
                out.push(TupleField::Const(sconst));
            }
            _ => {
                out.push(TupleField::Normal {
                    name: match item.title() {
                        Some(name) => name.to_string(),
                        None => format!("f{i}"),
                    },
                    schema: item,
                });
            }
        }
        *i += 1;
    }
}

pub fn build_tuple_fields(prefix_items: &[OpenApiSchema]) -> Vec<TupleField<'_>> {
    let mut fields = vec![];
    build_tuple_fields_inner(&mut fields, &mut 0, prefix_items);
    fields
}

pub fn lower_camel_case(input: &str) -> String {
    let mut s = String::new();
    let mut chars = input.chars();
    while let Some(c) = chars.next() {
        s.extend(c.to_lowercase());
        if !c.is_uppercase() {
            break;
        }
    }
    s.extend(chars);
    s
}

pub fn snake_to_camel_case(input: &str) -> String {
    let mut s = String::new();
    let input = input.replace("$", "");
    let mut chars = input.chars();
    while let Some(c) = chars.next() {
        if c == '_' {
            if let Some(c) = chars.next() {
                s.extend(c.to_uppercase());
            }
        } else {
            s.push(c);
        }
    }
    s
}

pub fn camel_to_snake_case(input: &str) -> String {
    let mut s = String::new();
    let mut chars = input.chars();
    if let Some(c) = chars.next() {
        s.extend(c.to_lowercase());
    }
    while let Some(c) = chars.next() {
        if c.is_uppercase() {
            s.push('_');
        }
        s.extend(c.to_lowercase());
    }
    s
}
