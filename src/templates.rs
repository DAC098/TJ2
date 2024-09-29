use std::collections::HashSet;
use std::fs::read_dir;
use std::path::{PathBuf, Path};

use tera::Tera;

use crate::error::{self, Context};
use crate::config;
use crate::path::metadata;

pub fn initialize(config: &config::Config) -> Result<Tera, error::Error> {
    let mut tera = Tera::default();
    let mut files = Vec::new();

    load_dir(
        &mut files,
        &config.settings.templates.directory,
        &config.settings.templates.directory
    )?;

    tera.add_template_files(files)
        .context("failed to add template files")?;

    let mut required = HashSet::from([
        "pages/index",
        "pages/login",
        "pages/entries",
    ]);

    for name in tera.get_template_names() {
        tracing::debug!("template name: {name}");

        required.remove(name);
    }

    if !required.is_empty() {
        let mut msg = String::from("missing required templates:");

        for name in required {
            msg.push_str("\n    ");
            msg.push_str(name);
        }

        return Err(error::Error::context(msg));
    }

    Ok(tera)
}

fn load_dir(files: &mut Vec<(PathBuf, Option<String>)>, root: &Path, dir: &Path) -> Result<(), error::Error> {
    let reader = read_dir(dir)
        .context(format!("failed to read directory: \"{}\"", dir.display()))?;

    tracing::debug!("traversing: \"{}\" root: \"{}\"", dir.display(), root.display());

    for result_entry in reader {
        let entry = result_entry.context("failed to retrieve directory entry")?;
        let entry_path = entry.path();

        let meta = metadata(&entry_path)
            .context(format!("failed to read metadata for directory entry: \"{}\"", entry_path.display()))?
            .unwrap();

        if meta.is_file() {
            let Some(ext) = entry_path.extension() else {
                continue;
            };

            if ext.eq("html") {
                let Some(name) = get_template_name(root, &entry_path)? else {
                    continue;
                };

                let name = name.to_owned();

                files.push((entry_path, Some(name)));

                //tera.add_template_file(&entry_path, Some(&name))
                    //.context(format!("failed to add template file: \"{}\"", entry_path.display()))?;
            }
        } else if meta.is_dir() {
            load_dir(files, root, &entry_path)?;
        }
    }

    Ok(())
}

fn get_template_name(root: &Path, path: &Path) -> Result<Option<String>, error::Error> {
    let parent_name = path.strip_prefix(root)
        .unwrap()
        .parent()
        .unwrap()
        .to_str()
        .context(format!("failed to convert parent path name to UTF-8 string: \"{}\"", path.display()))?;

    let Some(stem) = path.file_stem() else {
        return Ok(None);
    };

    let stem_str = stem.to_str()
        .context(format!("failed to convert file stem to UTF-8 string: \"{}\"", path.display()))?;

    Ok(Some(format!("{parent_name}/{stem_str}")))
}
