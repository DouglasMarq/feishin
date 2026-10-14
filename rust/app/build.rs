use std::error::Error;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use slint_build::{CompilerConfiguration, DefaultTranslationContext};

const TRANSLATIONS_DIR: &str = "translations";
const TRANSLATION_DOMAIN: &str = "feishin";

fn main() -> Result<(), Box<dyn Error>> {
    compile_ui()?;
    write_catalogs(Path::new(TRANSLATIONS_DIR))?;
    println!("cargo:rerun-if-changed={TRANSLATIONS_DIR}");
    Ok(())
}

fn compile_ui() -> Result<(), Box<dyn Error>> {
    let debug_build = std::env::var("PROFILE").is_ok_and(|profile| profile == "debug");
    let config = CompilerConfiguration::new()
        .with_style("fluent-dark".into())
        .with_debug_info(debug_build)
        .with_bundled_translations(TRANSLATIONS_DIR)
        .with_default_translation_context(DefaultTranslationContext::None);
    slint_build::compile_with_config("ui/app.slint", config).map_err(|error| error.to_string())?;
    Ok(())
}

fn catalog_files(translations_dir: &Path) -> Result<Vec<(String, PathBuf)>, Box<dyn Error>> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(translations_dir)? {
        let language_dir = entry?.path();
        let catalog = language_dir
            .join("LC_MESSAGES")
            .join(format!("{TRANSLATION_DOMAIN}.po"));
        let language = language_dir
            .file_name()
            .map(|name| name.to_string_lossy().into_owned());
        if let (Some(language), true) = (language, catalog.exists()) {
            println!("cargo:rerun-if-changed={}", catalog.display());
            files.push((language, catalog));
        }
    }
    files.sort();
    Ok(files)
}

fn translated_forms(entry: &rspolib::POEntry) -> Option<Vec<String>> {
    let forms = match entry.msgid_plural {
        Some(_) => entry.msgstr_plural.clone(),
        None => entry.msgstr.clone().into_iter().collect(),
    };
    let complete = !entry.msgid.is_empty()
        && !entry.obsolete
        && !forms.is_empty()
        && forms.iter().all(|form| !form.is_empty());
    complete.then_some(forms)
}

fn catalog_source(language: &str, catalog: &Path) -> Result<String, Box<dyn Error>> {
    let file = rspolib::pofile(catalog).map_err(|error| error.to_string())?;
    let mut source = format!("    super::Catalog {{ language: {language:?}, messages: &[\n");
    for entry in &file.entries {
        let Some(forms) = translated_forms(entry) else {
            continue;
        };
        let plural = entry.msgid_plural.clone().unwrap_or_default();
        writeln!(
            source,
            "        super::Message {{ singular: {:?}, plural: {plural:?}, forms: &{forms:?} }},",
            entry.msgid
        )?;
    }
    source.push_str("    ] },\n");
    Ok(source)
}

fn write_catalogs(translations_dir: &Path) -> Result<(), Box<dyn Error>> {
    let mut source = String::from("pub static CATALOGS: &[super::Catalog] = &[\n");
    for (language, catalog) in catalog_files(translations_dir)? {
        source.push_str(&catalog_source(&language, &catalog)?);
    }
    source.push_str("];\n");
    let out_dir = PathBuf::from(std::env::var("OUT_DIR")?);
    std::fs::write(out_dir.join("catalogs.rs"), source)?;
    Ok(())
}
