use std::collections::HashMap;
use std::path::{Path, PathBuf};

use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};

use crate::{
    compile::compile_typst_to_pdf,
    convert::{ConvertOptions, convert_markdown_to_typst},
    frontmatter::split_frontmatter,
    template::{Style, compose_document},
};

pub struct WatchCommand {
    pub style: Style,
    pub output: Option<PathBuf>,
    pub multiple_inputs: bool,
    pub title_override: Option<String>,
    pub author_override: Option<String>,
    pub lang_override: Option<String>,
    pub force_toc: Option<bool>,
    pub verbose: bool,
}

#[derive(Debug)]
pub enum WatchError {
    Io(std::io::Error),
    Notify(notify::Error),
    Compile(String),
}

impl std::fmt::Display for WatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(err) => write!(f, "{err}"),
            Self::Notify(err) => write!(f, "{err}"),
            Self::Compile(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for WatchError {}

pub fn watch_inputs(paths: &[PathBuf], command: &WatchCommand) -> Result<(), WatchError> {
    let (tx, rx) = std::sync::mpsc::channel();
    let mut watcher = RecommendedWatcher::new(
        move |res| {
            let _ = tx.send(res);
        },
        Config::default(),
    )
    .map_err(WatchError::Notify)?;

    let mut tracked_files = HashMap::<PathBuf, PathBuf>::new();
    for path in paths {
        let watch_path = canonicalize(path);
        watcher
            .watch(&watch_path, RecursiveMode::NonRecursive)
            .map_err(WatchError::Notify)?;
        tracked_files.insert(watch_path.clone(), path.clone());
        println!("watching {}", watch_path.display());
    }

    loop {
        let event = rx
            .recv()
            .map_err(|e| WatchError::Io(std::io::Error::other(e.to_string())))?;
        match event {
            Ok(Event {
                kind: EventKind::Modify(_) | EventKind::Create(_),
                paths,
                ..
            }) => {
                for changed in paths {
                    let canonical = canonicalize(&changed);
                    let Some(source_path) = tracked_files.get(&canonical) else {
                        continue;
                    };

                    if let Err(err) = rebuild_one(source_path, command) {
                        eprintln!("[watch] failed: {err}");
                    } else if command.verbose {
                        println!("[watch] updated {}", source_path.display());
                    }
                }
            }
            Ok(_) => {}
            Err(err) => return Err(WatchError::Notify(err)),
        }
    }
}

fn rebuild_one(path: &Path, command: &WatchCommand) -> Result<(), String> {
    let source = std::fs::read_to_string(path).map_err(|e| format!("{e}"))?;
    let parsed = split_frontmatter(&source).map_err(|e| format!("frontmatter: {e}"))?;
    let converted = convert_markdown_to_typst(
        &parsed.body,
        &parsed.frontmatter,
        &ConvertOptions {
            title_override: command.title_override.clone(),
            author_override: command.author_override.clone(),
            lang_override: command.lang_override.clone(),
            force_toc: command.force_toc,
        },
    )
    .map_err(|e| format!("{e}"))?;
    let typst = compose_document(
        command.style,
        converted.title.as_deref(),
        &converted.authors,
        &converted.lang,
        converted.toc,
        &converted.body,
    );
    let output = resolve_output_path(path, command.output.as_deref(), command.multiple_inputs);
    if let Some(parent) = output.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).map_err(|e| format!("{e}"))?;
    }
    compile_typst_to_pdf(&typst, &output).map_err(|e| format!("{e}"))?;
    Ok(())
}

fn resolve_output_path(input: &Path, output: Option<&Path>, multiple_inputs: bool) -> PathBuf {
    match output {
        Some(path) if multiple_inputs => path
            .join(input.file_name().unwrap_or_default())
            .with_extension("pdf"),
        Some(path) => path.to_path_buf(),
        None => input.with_extension("pdf"),
    }
}

fn canonicalize(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}
