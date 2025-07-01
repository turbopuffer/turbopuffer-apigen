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
    // Extract named types for any tuples inside of a top-level `anyOf`, and
    // replace the tuples with references to the new types. We only have limited
    // support for rendering `anyOf`s in Go and Java, and this gives us a chance
    // to fall into `render_any_of_refs` when we later attempt to render the
    // `anyOf`.

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
                if let OpenApiSchema::ArrayTuple {
                    prefix_items,
                    x_turbopuffer_variant_name,
                    ..
                } = item
                {
                    let mut sconsts = prefix_items.iter().filter_map(|item| match item {
                        OpenApiSchema::Const { sconst, .. } => Some(sconst),
                        _ => None,
                    });
                    let Some(sconst) = sconsts.next() else {
                        continue;
                    };
                    if sconsts.next().is_some() {
                        continue;
                    }
                    let ref_title = format!("{name}{sconst}");
                    let mut name = x_turbopuffer_variant_name
                        .as_ref()
                        .map(|variant_name| format!("{name}{variant_name}"))
                        .unwrap_or_else(|| format!("{name}{sconst}"));
                    if conflict_behavior == ConflictBehavior::AppendSuffix {
                        let mut new_name = name.clone();
                        let mut suffix = 2;
                        while new_schemas.contains_key(&new_name) {
                            new_name = format!("{name}{suffix}");
                            suffix += 1;
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
                            "extraction on array tuples from anyOf failed: duplicate schema name: {name}"
                        ))?
                    }
                }
            }
        }
    }
    for (name, schema) in new_schemas {
        if schemas.insert(name.clone(), schema).is_some() {
            Err(format!(
                "extraction on array tuples from anyOf failed: duplicate schema name: {name}"
            ))?
        }
    }
    Ok(())
}

#[derive(Debug, Clone)]
pub enum TupleField<'a> {
    Normal {
        name: String,
        schema: &'a OpenApiSchema,
    },
    Const(&'a str),
    StartIndent,
    EndIndent,
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
            OpenApiSchema::ArrayTuple {
                x_turbopuffer_flatten: true,
                prefix_items,
                ..
            } => {
                out.push(TupleField::StartIndent);
                build_tuple_fields_inner(out, i, prefix_items);
                out.push(TupleField::EndIndent);
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

pub fn build_tuple_fields(prefix_items: &[OpenApiSchema]) -> Vec<TupleField> {
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
