use std::{collections::HashMap, path::PathBuf};

use serde::{Deserialize, Serialize};

use grace::GraceCompilerOptions;

#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
    pub domains: HashMap<String, DomainConfig>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct DomainConfig {
    /// Path to the model file
    ///
    pub path: PathBuf,
    /// Name of the generated module
    ///
    /// Defaults to the name of the domain.
    pub module: String,
    /// The compiler to use for this domain
    ///
    pub compiler: Compiler,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "compiler")]
#[serde(rename_all = "lowercase")]
pub enum Compiler {
    /// Grace Model Compiler
    ///
    /// This is a feature-rich, general purpose model compiler that generates
    /// Rust code.
    Grace(GraceCompilerOptions),
}
