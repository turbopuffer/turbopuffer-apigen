use std::{collections::BTreeMap, error::Error, mem};

use crate::codegen::{OpenApiSchema, SCHEMA_REF_PREFIX, strip_schema_ref_prefix};

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
                // Pick a `title_suffix` (drives the generated factory function
                // name) and a `name_suffix` (drives the extracted schema's
                // identifier). Keeping them separate lets siblings discriminated
                // only by value shape (e.g. scalar vs array, with/without params)
                // collapse into a single factory and surface as JVM overloads
                // to Java/Kotlin callers, while their schemas stay unique via
                // `x-turbopuffer-variant-name`.
                let (title_suffix, name_suffix) = match item {
                    OpenApiSchema::ArrayTuple {
                        prefix_items,
                        x_turbopuffer_variant_name,
                        ..
                    } => {
                        let Some(sconst) = single_const(prefix_items) else {
                            continue;
                        };
                        let title = sconst.to_owned();
                        let name = x_turbopuffer_variant_name
                            .clone()
                            .unwrap_or_else(|| title.clone());
                        (title, name)
                    }
                    OpenApiSchema::String {
                        x_turbopuffer_variant_name: Some(name),
                        ..
                    }
                    | OpenApiSchema::Map {
                        x_turbopuffer_variant_name: Some(name),
                        ..
                    } => (name.clone(), name.clone()),
                    _ => continue,
                };
                let ref_title = format!("{name}{title_suffix}");
                let mut name = format!("{name}{name_suffix}");
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

/// For each `anyOf`-of-`$ref` schema, records which variant inherits from
/// which abstract base. The result maps each variant schema's name to its
/// enclosing union's name.
pub fn compute_inherits(
    schemas: &BTreeMap<String, OpenApiSchema>,
) -> Result<BTreeMap<String, String>, Box<dyn Error>> {
    let mut result = BTreeMap::new();
    for (name, schema) in schemas {
        let OpenApiSchema::AnyOf { any_of, .. } = schema else {
            continue;
        };
        for item in any_of {
            let OpenApiSchema::Ref { sref, .. } = item else {
                continue;
            };
            let sref = strip_schema_ref_prefix(sref)?;
            if let Some(existing) = result.insert(sref.into(), name.into()) {
                Err(format!(
                    "duplicate inheritance for {sref}: {existing} and {name}"
                ))?
            }
        }
    }
    Ok(result)
}

/// Rewrites object schemas with a single required field as 1-tuples, returning
/// a map from each rewritten schema's name to a `pascal -> original` JSON
/// property-name override map (so emitters that serialize tuples as objects
/// can recover the original key).
pub fn rewrite_single_field_objects_to_tuples(
    schemas: &mut BTreeMap<String, OpenApiSchema>,
) -> Result<BTreeMap<String, BTreeMap<String, String>>, Box<dyn Error>> {
    let mut names = BTreeMap::new();
    for (name, schema) in schemas {
        let OpenApiSchema::Object {
            properties,
            required,
            ..
        } = schema
        else {
            continue;
        };
        if properties.len() != 1 || required.len() != 1 {
            continue;
        }
        let (prop_name, prop_schema) = properties.iter().next().unwrap();
        if !required.contains(prop_name) {
            continue;
        }
        let prop_name = prop_name.clone();
        let prop_name_munged = snake_to_camel_case(&prop_name);
        let mut prop_schema = prop_schema.clone();
        if let Some(title) = prop_schema.title_mut() {
            *title = Some(prop_name_munged.clone());
        }
        *schema = OpenApiSchema::ArrayTuple {
            prefix_items: vec![prop_schema],
            _description: None,
            _type: Default::default(),
            additional_items: false,
            x_turbopuffer_variant_name: None,
            x_turbopuffer_variant_drop_on_conflict: false,
            title: None,
        };
        names.insert(
            name.clone(),
            BTreeMap::from([(prop_name_munged, prop_name)]),
        );
    }
    Ok(names)
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

/// Walks `schemas` and assigns a generic type parameter (`T`, `U`, `V`, ...) to
/// every `items: any` array. The assignment is stashed in the `ArrayList`'s
/// `description` field as `#GEN:T`; callers extract it later via
/// [`array_list_generic`] when rendering the list element type.
///
/// Returns the generic letter names that were assigned, in order, so the
/// caller can format the decl/inst suffix in the target language (e.g. `<T,
/// U>` for C#/Kotlin, `[T any, U any]`/`[T, U]` for Go).
pub fn assign_generics(schemas: &mut [OpenApiSchema]) -> Vec<&'static str> {
    // Add more letters if necessary. But the odds of actually needing more
    // than 7 generic parameters are minuscule.
    const GENERICS: &[&str] = &["T", "U", "V", "W", "X", "Y", "Z"];

    fn assign(index: &mut usize, schema: &mut OpenApiSchema) {
        match schema {
            OpenApiSchema::AnyOf { any_of, .. } => {
                for item in any_of {
                    assign(index, item);
                }
            }
            OpenApiSchema::Object { properties, .. } => {
                for (_, prop_schema) in properties {
                    assign(index, prop_schema);
                }
            }
            OpenApiSchema::Map {
                additional_properties,
                ..
            } => {
                assign(index, additional_properties);
            }
            OpenApiSchema::ArrayList {
                items, description, ..
            } => {
                if let OpenApiSchema::Any { .. } = &**items {
                    // NOTE: it's a bit of a hack to jam this into the
                    // `description` field, but it's very convenient.
                    *description = Some(format!("{GENERIC_DESCRIPTION_PREFIX}{}", GENERICS[*index]));
                    *index += 1;
                } else {
                    assign(index, items);
                }
            }
            OpenApiSchema::ArrayTuple { prefix_items, .. } => {
                for item in prefix_items {
                    assign(index, item);
                }
            }
            OpenApiSchema::String { .. }
            | OpenApiSchema::Number { .. }
            | OpenApiSchema::Boolean { .. }
            | OpenApiSchema::Const { .. }
            | OpenApiSchema::Ref { .. }
            | OpenApiSchema::Any { .. } => {}
        }
    }

    let mut index = 0;
    for schema in schemas {
        assign(&mut index, schema);
    }
    GENERICS[..index].to_vec()
}

/// Returns the generic letter name (`"T"`, `"U"`, ...) stashed on an
/// `ArrayList`'s `description` by [`assign_generics`], if any.
pub fn array_list_generic(description: &Option<String>) -> Option<&str> {
    description
        .as_ref()
        .and_then(|d| d.strip_prefix(GENERIC_DESCRIPTION_PREFIX))
}

const GENERIC_DESCRIPTION_PREFIX: &str = "#GEN:";
