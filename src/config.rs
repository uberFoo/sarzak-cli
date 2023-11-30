use std::{collections::HashMap, path::PathBuf};

use serde::{Deserialize, Serialize};

// use chacha::dwarf::DwarfOptions;
use grace::GraceCompilerOptions;

#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
    pub modules: HashMap<String, ModuleConfig>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ModuleConfig {
    /// Path to the model file
    ///
    pub model: PathBuf,
    /// The compiler to use for this domain
    ///
    pub compiler: Vec<Compiler>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "compiler")]
#[serde(rename_all = "lowercase")]
pub enum Compiler {
    /// Grace Model Compiler
    ///
    /// This is a feature-rich, general purpose model compiler that generates
    /// Rust code -- for now. It's eventually going to be general purpose. Although
    /// it might get archived before that happens...  You just never know.
    Grace(GraceCompilerOptions),
    //    /// Dwarf Language Compiler
    //    ///
    //    /// This compiles the dwarf code into a Lu-Dog model, which is basically an
    //    /// AST.
    // Dwarf(DwarfOptions),
}
