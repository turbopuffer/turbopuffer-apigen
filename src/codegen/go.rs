use std::{collections::BTreeMap, error::Error};

use crate::{
    codegen::{
        OpenApiSchema,
        shared::{self, ConflictBehavior, TupleField},
        strip_schema_ref_prefix,
    },
    util::codegen_buf::CodegenBuf,
};

pub fn render(mut schemas: BTreeMap<String, OpenApiSchema>) -> Result<CodegenBuf, Box<dyn Error>> {
    shared::extract_any_of_tuples(&mut schemas, ConflictBehavior::Drop)?;

    let mut buf = CodegenBuf::with_indent("\t");

    buf.writeln("// Code generated by turbopuffer-apigen. DO NOT EDIT.");
    buf.writeln("");
    buf.writeln("package turbopuffer");
    buf.writeln("");
    buf.writeln(
        r#"import shimjson "github.com/turbopuffer/turbopuffer-go/internal/encoding/json""#,
    );
    buf.writeln("");

    for (name, schema) in &schemas {
        buf.start_line();
        buf.write(format!("type {name} "));
        render_schema(&schemas, &mut buf, Some(name), &schema)?;
        buf.end_line();
    }

    Ok(buf)
}

fn render_schema(
    schemas: &BTreeMap<String, OpenApiSchema>,
    buf: &mut CodegenBuf,
    name: Option<&str>,
    schema: &OpenApiSchema,
) -> Result<(), Box<dyn Error>> {
    match schema {
        OpenApiSchema::AnyOf {
            _description: _,
            any_of,
        } => {
            if any_of
                .iter()
                .all(|s| matches!(s, OpenApiSchema::Const { .. }))
            {
                render_any_of_const_enum(buf, name, any_of)?;
            } else if any_of
                .iter()
                .all(|s| matches!(s, OpenApiSchema::Ref { .. }))
            {
                render_any_of_refs(schemas, buf, name, any_of)?;
            } else {
                Err("unsupported anyOf")?
            }
        }
        OpenApiSchema::Object {
            _description: _,
            _type: _,
            properties,
            title: _,
            required,
        } => {
            let Some(name) = name else {
                Err("object schema in unsupported position")?
            };
            if properties.len() != 1 || required.len() != 1 {
                Err("object schemas only supported with a required property")?
            };
            let (prop_name, prop_schema) = properties.into_iter().next().unwrap();
            if !required.contains(prop_name) {
                Err("object schemas only supported with a single required property")?
            };

            let prop_name_munged = prop_name.replace("$", "");
            let prop_name_munged = shared::snake_to_camel_case(&prop_name_munged);

            // Struct definition.
            buf.write_block("struct", |buf| {
                buf.start_line();
                buf.write(format!("{prop_name_munged} "));
                render_schema(schemas, buf, None, prop_schema)?;
                buf.end_line();
                Ok::<_, Box<dyn Error>>(())
            })?;

            // Constructor function.
            buf.writeln(format!("func New{name}("));
            buf.indent();
            buf.start_line();
            buf.write(format!("{prop_name_munged} "));
            render_schema(schemas, buf, None, prop_schema)?;
            buf.write(",");
            buf.end_line();
            buf.unindent();
            buf.write_block(format!(") {name}"), |buf| {
                buf.write_block(format!("return {name}"), |buf| {
                    buf.writeln(format!("{prop_name_munged}: {prop_name_munged},"));
                })
            });

            buf.write_block(
                format!("func (v {name}) MarshalJSON() ([]byte, error)"),
                |buf| {
                    buf.writeln("return shimjson.Marshal(map[string]any{");
                    buf.indent();
                    buf.writeln(format!("\"{prop_name}\": v.{prop_name_munged},"));
                    buf.unindent();
                    buf.writeln("})");
                },
            );
        }
        OpenApiSchema::ArrayList {
            _description: _,
            _type: _,
            items,
            title: _,
        } => {
            buf.write("[]");
            render_schema(schemas, buf, None, &*items)?;
        }
        OpenApiSchema::ArrayTuple {
            additional_items: true,
            ..
        } => Err("tuple-type arrays with `items: true` unsupported")?,
        OpenApiSchema::ArrayTuple {
            _description: _,
            _type: _,
            additional_items: false,
            prefix_items,
            x_turbopuffer_variant_name: _,
            x_turbopuffer_variant_drop_on_conflict: _,
            x_turbopuffer_flatten: _,
            title: _,
        } => {
            // Since Go doesn't natively support tuples, we convert each tuple
            // to a struct with private fields and a constructor function that
            // takes the fields as arguments, in the order the tuple defines
            // them.

            let Some(name) = name else {
                Err("tuple-type arrays in unsupported position")?
            };

            let fields = shared::build_tuple_fields(prefix_items);
            let fields_no_consts = fields
                .iter()
                .filter(|f| !matches!(f, TupleField::Const { .. }))
                .collect::<Vec<_>>();

            // Struct definition.
            buf.write_block("struct", |buf| {
                for field in &fields_no_consts {
                    if let TupleField::Normal { name, schema, .. } = field {
                        buf.start_line();
                        buf.write(format!("{name} "));
                        render_schema(schemas, buf, None, schema)?;
                        buf.end_line();
                    }
                }
                Ok::<_, Box<dyn Error>>(())
            })?;

            // Constructor function.
            buf.writeln(format!("func New{name}("));
            buf.indent();
            for field in &fields_no_consts {
                if let TupleField::Normal { name, schema, .. } = field {
                    buf.start_line();
                    buf.write(format!("{name} "));
                    render_schema(schemas, buf, None, schema)?;
                    buf.write(",");
                    buf.end_line();
                }
            }
            buf.unindent();
            buf.write_block(format!(") {name}"), |buf| {
                buf.write_block(format!("return {name}"), |buf| {
                    for field in &fields_no_consts {
                        if let TupleField::Normal { name, .. } = field {
                            buf.writeln(format!("{name},"));
                        }
                    }
                    Ok::<_, Box<dyn Error>>(())
                })?;
                Ok::<_, Box<dyn Error>>(())
            })?;

            buf.write_block(
                format!("func (v {name}) MarshalJSON() ([]byte, error)"),
                |buf| {
                    buf.writeln("return shimjson.Marshal([]any{");
                    buf.indent();
                    for field in &fields {
                        match field {
                            TupleField::Const(sconst) => {
                                buf.writeln(format!("\"{sconst}\","));
                            }
                            TupleField::StartIndent => {
                                buf.writeln("[]any{");
                                buf.indent();
                            }
                            TupleField::EndIndent => {
                                buf.unindent();
                                buf.writeln("},");
                            }
                            TupleField::Normal { name, schema: _ } => {
                                buf.writeln(format!("v.{name},"));
                            }
                        }
                    }
                    buf.unindent();
                    buf.writeln("})");
                },
            );
        }
        OpenApiSchema::String {
            _description: _,
            _type: _,
            title: _,
        } => buf.write("string"),
        OpenApiSchema::Number {
            _description: _,
            _type: _,
            title: _,
            x_turbopuffer_width,
        } => match x_turbopuffer_width {
            Some(32) => buf.write("float32"),
            None | Some(64) => buf.write("float64"),
            Some(w) => Err(format!("unsupported number width: {w}"))?,
        },
        OpenApiSchema::Const {
            _description: _,
            sconst: _,
            title: _,
        } => Err("const in unsupported position")?,
        OpenApiSchema::Ref { sref, title: _ } => {
            let name = strip_schema_ref_prefix(sref)?;
            buf.write(name)
        }
        OpenApiSchema::Any { .. } => buf.write("any"),
    }
    Ok(())
}

fn render_any_of_const_enum(
    buf: &mut CodegenBuf,
    name: Option<&str>,
    schema: &[OpenApiSchema],
) -> Result<(), Box<dyn Error>> {
    // When all the items in an `anyOf` are consts, we can generate a string
    // enum for the `anyOf`.
    // This is a workaround for Go's lack of sum types.

    let Some(name) = name else {
        Err("const enum in unsupported position")?
    };

    // Definition of the enum type.
    buf.write("string");
    buf.end_line();

    // Definition of constants for each enum variant.
    buf.writeln("const (");
    buf.indent();
    for item in schema {
        let OpenApiSchema::Const {
            _description: _,
            title: x_turbopuffer_name,
            sconst,
        } = item
        else {
            unreachable!("validated by render_schema");
        };
        let sconst_name = x_turbopuffer_name.clone().unwrap_or_else(|| {
            let mut sconst_name = name.to_string();
            let mut chars = sconst.chars();
            if let Some(first_char) = chars.next() {
                sconst_name.extend(first_char.to_uppercase());
            }
            sconst_name.extend(chars);
            sconst_name
        });
        buf.writeln(format!("{sconst_name} {name} = \"{sconst}\""))
    }
    buf.unindent();
    buf.writeln(")");

    Ok(())
}

fn render_any_of_refs(
    schemas: &BTreeMap<String, OpenApiSchema>,
    buf: &mut CodegenBuf,
    name: Option<&str>,
    schema: &[OpenApiSchema],
) -> Result<(), Box<dyn Error>> {
    // When all the items in an `anyOf` are refs, we can generate a sealed
    // interface for the `anyOf` and implement it for all the referenced types.
    // This is a workaround for Go's lack of sum types.

    let Some(name) = name else {
        Err("anyOf with refs in unsupported position")?
    };

    // The name of the function that will define the sealed interface.
    let fn_name = format!("sealed_{}", name);

    // Interface declaration.
    buf.write_block("interface", |buf| buf.writeln(format!("{fn_name}()")));

    // Implementations.
    fn render(
        schemas: &BTreeMap<String, OpenApiSchema>,
        buf: &mut CodegenBuf,
        fn_name: &str,
        schema: &[OpenApiSchema],
    ) -> Result<(), Box<dyn Error>> {
        for item in schema {
            let OpenApiSchema::Ref { sref, .. } = item else {
                unreachable!("validated by render_schema");
            };
            let sref = strip_schema_ref_prefix(sref)?;
            match &schemas[sref] {
                OpenApiSchema::AnyOf {
                    _description: _,
                    any_of,
                } if any_of
                    .iter()
                    .all(|s| matches!(s, OpenApiSchema::Ref { .. })) =>
                {
                    render(schemas, buf, fn_name, any_of)?;
                }
                _ => buf.writeln(format!("func (v {sref}) {fn_name}() {{}}")),
            }
        }
        Ok(())
    }
    render(schemas, buf, &fn_name, schema)?;

    Ok(())
}
