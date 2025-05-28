use std::{collections::BTreeMap, error::Error};

use crate::{codegen::OpenApiSchema, util::codegen_buf::CodegenBuf};

pub fn render(_schemas: BTreeMap<String, OpenApiSchema>) -> Result<CodegenBuf, Box<dyn Error>> {
    Err("unimplemented".into())
}
