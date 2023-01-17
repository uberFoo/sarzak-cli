use std::{
    ffi::OsString, fs, fs::File, io::Write, os::unix::ffi::OsStringExt, path::PathBuf, process,
};

use anyhow::{Context, Result};
use clap::{ArgAction, Parser, Subcommand};
use heck::{ToSnakeCase, ToTitleCase};
use log::debug;
use pretty_env_logger;
use uuid::Uuid;

use nut::codegen::{emitln, CachingContext, SarzakModel, WriteSarzakModel};
use nut::domain::{generate_macros, generate_store, generate_types};

const BLANK_MODEL: &str = include_str!("../models/blank.json");
const MODEL_DIR: &str = "models";

const TYPES: &str = "types";
const MACROS: &str = "macros";
const STORE: &str = "store";

const RS_EXT: &str = "rs";
const JSON_EXT: &str = "json";

#[derive(Debug, Parser)]
#[command(author, version, about)]
#[command(propagate_version = true)]
struct Args {
    /// Test mode
    ///
    /// Don't execute commands, but instead print what commands would be executed.
    #[clap(long, short, action=ArgAction::SetTrue)]
    test: bool,

    /// Path to package
    ///
    /// If included, `sarzak` will create a new domain in the specified
    /// location. It must exist, and must be part of a Rust package.
    #[arg(short, long)]
    package_dir: Option<PathBuf>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Create a new domain
    ///
    /// This involves creating a new module within a Rust package, as well as a
    /// new model file.
    ///
    /// This command needs to be run within a rust package, i.e., someplace
    /// below a `Cargo.toml`. Hopefully we're smart enough to sort out the
    /// details.
    New {
        /// Domain name
        ///
        /// The name of your new domain! Name it anything you like, although I
        /// haven't yet tried unicode... 🤔 One way or another we'll sort out
        /// the name, and create a new Rust module. which will coincide with the name of
        /// the Rust package. It will contain a blank model file the the `models`
        /// subdirectory.
        domain: String,
    },
    /// Generate code
    ///
    /// Generate domain code from the model.
    #[command(name = "gen")]
    Generate {
        /// Domain name(s)
        ///
        /// The comma separated list of domains for which code will be generated.
        /// If this argument is not included, and there are multiple domain models,
        /// then code will be generated for all models in the domain.
        #[arg(use_value_delimiter = true, value_delimiter = ',')]
        domains: Option<Vec<String>>,

        /// Generate Code for "meta" domain
        ///
        /// This flag changes code generation for domains that are considered meta
        /// domains. At the moment those include the Sarzak and Drawing Domains.
        ///
        /// You probably don't want this unless your name is Keith.
        #[arg(short, long)]
        meta: bool,

        /// Doc Tests
        ///
        /// This flag controls the generation of doc tests. It is disabled by default.
        /// Therefore, use this flag to enable generation of tests.
        #[clap(long, short, action=ArgAction::SetTrue)]
        doc_tests: bool,
    },
}

fn main() -> Result<()> {
    pretty_env_logger::init();

    let args = Args::parse();

    if args.test {
        println!("Running in test mode 🧪.");
    }

    match args.command {
        Command::New { domain } => execute_command_new(&domain, &args.package_dir, args.test)?,
        Command::Generate {
            domains,
            meta,
            doc_tests,
        } => execute_command_generate(&domains, &args.package_dir, meta, args.test, doc_tests)?,
    }

    Ok(())
}

fn execute_command_new(
    domain: &str,
    dir: &Option<PathBuf>,
    // meta: bool,
    test_mode: bool,
) -> Result<()> {
    let rust_name = domain.to_snake_case();

    // Find the package root
    //
    let package_root = find_package_dir(dir)?;

    println!(
        "Creating new domain ✨{}✨ in {}❗️",
        domain,
        package_root.to_string_lossy()
    );
    println!("The module will be called ✨{}✨.", rust_name);

    // Write a blank model file.
    //
    let mut model_file = package_root.clone();
    model_file.push(MODEL_DIR);

    // Make sure the directory exists.
    //
    fs::create_dir_all(&model_file).context(format!("😱Failed to create models directory."))?;

    // Interesting aside. PathBuf::set_file_name does a pop first.
    model_file.push("fubar");

    model_file.set_file_name(&rust_name);
    model_file.set_extension(JSON_EXT);

    debug!("Creating blank model 🐶 file at {:?}.", model_file);
    if !test_mode {
        let model = BLANK_MODEL.replace("Paper::blank", &domain);
        File::create(&model_file)
            .context(format!("😱Failed to create file: {:?}", model_file))?
            .write_all(model.as_bytes())
            .context(format!("😱Failed to write to file: {:?}", model_file))?;
    }

    // Create a new directory for the module
    //
    let mut src_dir = package_root.clone();
    src_dir.push("src");
    src_dir.push(&rust_name);
    debug!("Creating module directory {:?}.", src_dir);
    if !test_mode {
        fs::create_dir(&src_dir).context(format!("😱Failed to create directory: {:?}", src_dir))?;
    }

    // Generate a "module" .rs file
    //
    debug!("Creating {}.rs.🥳", rust_name);
    src_dir.set_file_name(&rust_name);
    src_dir.set_extension("rs");

    if !test_mode {
        let contents = generate_module_file(&domain);
        File::create(&src_dir)
            .context(format!("😱Failed to create file: {:?}", src_dir))?
            .write_all(contents.as_bytes())
            .context(format!("😱Failed to write to file: {:?}", src_dir))?;
    }

    // Update `lib.rs` with the new module.
    //
    // I wonder is there's a way to parse the file as rust code, edit
    // the tokenstream, and then write it back out? Nicely formatted?
    //
    // Thinking that this waits. There are issues to overcome. The first
    // is that we can't include the new module because it has no source
    // files. We can't generate source files until we have a model.
    // At least that's how the code gen code works now. They all fail
    // (panic) trying to read objects. In any case, code gen should
    // happen first.

    // Generate code for the blank model? So that everything is happy?
    //
    // Yes!
    // Mabye no...everything is already happy because lib.rs doesn't know about
    // this module yet. I don't want to generate code because I'm tired of
    // passing the same args to both functions.
    // debug!("Generating 🧬 code!");
    // if !test_mode {
    //     generate_domain_code(&package_root, &model_file, meta, test_mode)?;
    // }

    Ok(())
}

fn generate_module_file(domain: &str) -> String {
    let mut context = CachingContext::new();

    let uuid = Uuid::new_v5(&Uuid::NAMESPACE_OID, domain.as_bytes());

    emitln!(context, "//! {} Domain", domain.to_title_case());
    emitln!(context, "//!");
    emitln!(
        context,
        "//! This file was generated by: `sarzak new \"{}\"`.",
        domain
    );
    emitln!(context, "use uuid::{uuid, Uuid};");
    emitln!(context, "");
    emitln!(context, "pub mod macros;");
    emitln!(context, "pub mod store;");
    emitln!(context, "pub mod types;");
    emitln!(context, "");
    emitln!(context, "pub use store::ObjectStore;");
    emitln!(context, "pub use types::*;");
    emitln!(context, "pub use macros::*;");
    emitln!(context, "");
    emitln!(context, "// {}", domain);
    emitln!(context, "pub const UUID_NS: Uuid = uuid!(\"{}\");", uuid);
    emitln!(context, "");
    emitln!(context, "#[cfg(test)]");
    emitln!(context, "mod tests {");
    context.increase_indent();
    emitln!(context, "use super::*;");
    emitln!(context, "");
    emitln!(context, "#[test]");
    emitln!(context, "fn test() {");
    emitln!(context, "}");
    context.decrease_indent();
    emitln!(context, "}");

    context.commit()
}

fn execute_command_generate(
    domains: &Option<Vec<String>>,
    gen_dir: &Option<PathBuf>,
    meta: bool,
    test_mode: bool,
    doc_tests: bool,
) -> Result<()> {
    // Find the package root
    //
    let package_root = find_package_dir(gen_dir)?;

    // Ensure that we can find the models directory
    //
    let mut model_dir = package_root.clone();
    model_dir.push(MODEL_DIR);
    anyhow::ensure!(
        model_dir.exists(),
        format!("😱Unable to find models directory: {:?}.", model_dir)
    );

    // Ensure that we can find the model file(s)
    //
    if let Some(domains) = domains {
        for domain in domains {
            // Spaces between commas in the domain specification result in spaces
            // in our domains list. Just skip.
            if domain != "" {
                let mut model_file = model_dir.clone();
                // Don't forget about the pop.
                model_file.push("fubar");
                model_file.set_file_name(&domain);
                model_file.set_extension(JSON_EXT);

                debug!("⭐️ Found {:?}!", model_file);

                generate_domain_code(&package_root, &model_file, meta, test_mode, doc_tests)?;
            }
        }
    } else {
        // Iterate over all of the model files
        for entry in fs::read_dir(&model_dir)? {
            let path = &entry?.path();
            if let Some(ext) = path.extension() {
                if ext == "json" {
                    generate_domain_code(&package_root, &path, meta, test_mode, doc_tests)?;
                }
            }
        }
    }

    Ok(())
}

/// Generate types.rs, store.rs, and macros.rs
///
/// There is an assumption here that the model file is named the same as the
/// module, and all of it's files. This assumption holds true assuming it was
/// all setup with this program.
fn generate_domain_code(
    root: &PathBuf,
    model_file: &PathBuf,
    meta: bool,
    test_mode: bool,
    doc_tests: bool,
) -> Result<()> {
    // Check that the path exists, and that it's a file. From there we just
    // have to trust...
    anyhow::ensure!(
        model_file.exists(),
        format!("😱Model file ({:?}) does not exist!", model_file)
    );
    anyhow::ensure!(
        model_file.is_file(),
        format!("😱{:?} is not a model file!", model_file)
    );
    if let Some(extension) = model_file.extension() {
        anyhow::ensure!(
            extension == JSON_EXT,
            format!("😱{:?} is not a json file!", model_file)
        );
    } else {
        anyhow::bail!(format!("😱{:?} is not a json file!", model_file));
    }

    let module = if let Some(stem) = model_file.file_stem() {
        stem
    } else {
        anyhow::bail!(format!(
            "😱Cannot extract the module name from the model file: {:?}!",
            model_file
        ));
    };

    // let mut output = root.clone();
    // output.push(MODEL_DIR);
    // output.push("fubar");
    // output.set_file_name(&module);
    // output.set_extension("sarzak");

    println!(
        "Generating 🧬 code for domain ✨{:?}✨!",
        module.to_string_lossy()
    );
    debug!("Generating 🧬 code for domain, {:?}!", model_file);

    let model = SarzakModel::load_cuckoo_model(&model_file).context("😱 reading model file")?;
    // File::create(&output).context("couldn't open file for writing")?.to_json(&model);

    let mut module_path = root.clone();
    module_path.push("src");
    module_path.push(&module);
    module_path.push("fubar");

    let package = root
        .as_path()
        .components()
        .last()
        .unwrap()
        .as_os_str()
        .to_string_lossy();

    // generate types.rs
    //
    module_path.set_file_name(TYPES);
    module_path.set_extension(RS_EXT);
    debug!("Writing 🖍️ {:?}!", module_path);
    if !test_mode {
        generate_types(&model, &module_path, &package, meta, doc_tests)?;
    }

    // generate store.rs
    //
    module_path.set_file_name(STORE);
    module_path.set_extension(RS_EXT);
    debug!("Writing ✏️ {:?}!", module_path);
    if !test_mode {
        generate_store(&model, &module_path, &package, meta, doc_tests)?;
    }

    // generate macros.rs
    //
    module_path.set_file_name(MACROS);
    module_path.set_extension(RS_EXT);
    debug!("Writing ✒️ {:?}!", module_path);
    if !test_mode {
        generate_macros(&model, &module_path, &package, meta, doc_tests)?;
    }

    Ok(())
}

fn find_package_dir(start_dir: &Option<PathBuf>) -> Result<PathBuf> {
    if let Some(dir) = start_dir {
        std::env::set_current_dir(&dir)?;
    }

    // Figure out where Cargo.toml is located.
    //
    let output = process::Command::new("cargo")
        .arg("locate-project")
        .arg("--message-format")
        .arg("plain")
        .output()
        .context(
            "😱Tried running `cargo locate-project to no avail. \
                Maybe you need to add cargo to you path?",
        )?;

    anyhow::ensure!(
        output.status.success(),
        format!(
            "😱Unable to find package in directory: {:?}.",
            std::env::current_dir()?
        )
    );

    let mut stdout = output.stdout;

    // I don't know if it's kosher, but this does nicely to get rid of
    // that newline character.
    stdout.pop();
    let os_string = OsString::from_vec(stdout);
    let mut package_root = PathBuf::from(os_string);
    // Get rid of Cargo.toml
    package_root.pop();

    debug!("Found root 🦀 at {:?}!", package_root);

    Ok(package_root)
}
