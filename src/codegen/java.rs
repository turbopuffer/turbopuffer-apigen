use std::{collections::BTreeMap, error::Error};

use crate::{StainlessStats, codegen::OpenApiSchema, util::codegen_buf::CodegenBuf};

pub fn render(
    _stainless_stats: &StainlessStats,
    _schemas: BTreeMap<String, OpenApiSchema>,
) -> Result<CodegenBuf, Box<dyn Error>> {
    Err("unimplemented".into())
}
