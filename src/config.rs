use std::{collections::HashMap, path::PathBuf};

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub domains: HashMap<String, DomainConfig>,
}

#[derive(Debug, Deserialize)]
pub struct DomainConfig {
    pub path: PathBuf,
    pub module: String,
    pub sarzak: SarzakCompilerOptions,
}

#[derive(Debug, Deserialize)]
pub struct SarzakCompilerOptions {
    /// Enable output for domains sarzak and drawing
    ///
    /// Specifically this flag affects how objects are imported across domains.
    pub meta: Option<bool>,
    /// Generate documentation tests
    ///
    /// Currently this includes tests for the `new` associated function on generated
    /// structs. A function `test_default` is generated for enums, that creates
    /// instances, in a manner similar to `new` for structs.
    ///
    /// Tests are also generated for the relationship navigation macros.
    pub doc_tests: Option<bool>,
    /// Control emitting new implementations
    ///
    /// This is orthogonal to `doc_tests`. While the latter relies on this,
    /// this does not rely on it.
    pub new: Option<bool>,
}
