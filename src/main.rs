use std::fs;
use std::io::{self, Read};
use std::path::PathBuf;
use std::process;

use clap::Parser;
use mdxport::{
    compile::compile_typst_to_pdf,
    convert::{ConvertOptions, convert_markdown_to_typst},
    frontmatter::{ParsedMarkdown, split_frontmatter},
    template::{Style, compose_document},
    watch::{WatchCommand, watch_inputs},
};

#[derive(Debug, Parser)]
#[command(name = "mdxport")]
#[command(about = "Markdown to Typst PDF converter")]
struct Cli {
    #[arg(help = "Input markdown files. If omitted, read from stdin.")]
    inputs: Vec<PathBuf>,

    #[arg(
        short,
        long,
        help = "Output path. Defaults to <input>.pdf for file input."
    )]
    output: Option<PathBuf>,

    #[arg(short, long, default_value = "modern-tech", value_name = "style", value_parser = clap::builder::PossibleValuesParser::new(["modern-tech", "classic-editorial"]))]
    style: String,

    #[arg(long = "template", help = "Path to a custom Typst template file (.typ).")]
    custom_template: Option<PathBuf>,

    #[arg(
        short = 't',
        long,
        help = "Override document title (frontmatter fallback)."
    )]
    title: Option<String>,

    #[arg(
        short = 'a',
        long,
        help = "Override document author (frontmatter fallback)."
    )]
    author: Option<String>,

    #[arg(long, help = "Document language: zh or en.")]
    lang: Option<String>,

    #[arg(long, help = "Force table of contents.")]
    toc: bool,

    #[arg(long = "no-toc", help = "Disable table of contents.")]
    no_toc: bool,

    #[arg(short, long, help = "Watch input files and recompile on change.")]
    watch: bool,

    #[arg(short, long, default_value_t = false, help = "Verbose diagnostics.")]
    verbose: bool,
}

#[derive(Debug)]
enum InputSource {
    File(PathBuf),
    Stdin(String),
}

struct ProcessOptions<'a> {
    output: &'a Option<PathBuf>,
    title: &'a Option<String>,
    author: &'a Option<String>,
    lang: &'a Option<String>,
    force_toc: Option<bool>,
    verbose: bool,
    style: Style,
    custom_template: Option<String>,
    multiple_inputs: bool,
}

fn main() {
    let cli = Cli::parse();
    if let Err(error) = run(cli) {
        eprintln!("[mdxport] {error}");
        process::exit(1);
    }
}

fn run(cli: Cli) -> Result<(), String> {
    if cli.inputs.is_empty() && cli.watch {
        return Err("watch mode requires at least one input file".to_string());
    }

    let multiple_inputs = cli.inputs.len() > 1;
    if multiple_inputs
        && let Some(output) = &cli.output
        && output.extension().is_some()
    {
        return Err("multiple input files require output directory path".to_string());
    }

    let style = Style::try_from(cli.style.as_str()).map_err(|e| e.to_string())?;
    let force_toc = resolve_force_toc(cli.no_toc, cli.toc);

    let inputs = if cli.inputs.is_empty() {
        vec![InputSource::Stdin(read_stdin()?)]
    } else {
        cli.inputs
            .into_iter()
            .map(InputSource::File)
            .collect::<Vec<_>>()
    };

    if cli.watch {
        let files = inputs
            .iter()
            .filter_map(|i| match i {
                InputSource::File(path) => Some(path.clone()),
                InputSource::Stdin(_) => None,
            })
            .collect::<Vec<_>>();

        if files.is_empty() {
            return Err("watch mode requires file inputs (stdin cannot be watched)".to_string());
        }

        let command = WatchCommand {
            style,
            output: cli.output.clone(),
            multiple_inputs,
            title_override: cli.title.clone(),
            author_override: cli.author.clone(),
            lang_override: cli.lang.clone(),
            force_toc,
            verbose: cli.verbose,
        };

        return watch_inputs(&files, &command).map_err(|e| format!("watch failed: {e}"));
    }

    let custom_template = cli
        .custom_template
        .map(|p| fs::read_to_string(&p).map_err(|e| format!("read template: {e}")))
        .transpose()?;

    let process_options = ProcessOptions {
        output: &cli.output,
        title: &cli.title,
        author: &cli.author,
        lang: &cli.lang,
        force_toc,
        verbose: cli.verbose,
        style,
        custom_template,
        multiple_inputs,
    };

    inputs
        .iter()
        .try_for_each(|input| process_one(input, &process_options))?;

    Ok(())
}

fn process_one(input: &InputSource, options: &ProcessOptions<'_>) -> Result<(), String> {
    let path_hint = match input {
        InputSource::File(path) => Some(path.as_path()),
        InputSource::Stdin(_) => None,
    };

    let source = match input {
        InputSource::File(path) => {
            fs::read_to_string(path).map_err(|e| format!("read markdown failed: {e}"))?
        }
        InputSource::Stdin(markdown) => markdown.clone(),
    };

    let ParsedMarkdown { frontmatter, body } =
        split_frontmatter(&source).map_err(|e| format!("frontmatter parse: {e}"))?;

    let conversion = convert_markdown_to_typst(
        &body,
        &frontmatter,
        &ConvertOptions {
            title_override: options.title.clone(),
            author_override: options.author.clone(),
            lang_override: options.lang.clone(),
            force_toc: options.force_toc,
        },
    )
    .map_err(|e| format!("markdown conversion failed: {e}"))?;

    let typst_source = if let Some(ref tmpl) = options.custom_template {
        mdxport::template::compose_document_with_custom(
            tmpl,
            conversion.title.as_deref(),
            &conversion.authors,
            &conversion.lang,
            conversion.toc,
            &conversion.body,
        )
    } else {
        compose_document(
            options.style,
            conversion.title.as_deref(),
            &conversion.authors,
            &conversion.lang,
            conversion.toc,
            &conversion.body,
        )
    };

    let out_path = match (options.output, path_hint) {
        (Some(path), Some(path_hint)) if options.multiple_inputs => path
            .join(path_hint.file_name().unwrap_or_default())
            .with_extension("pdf"),
        (Some(path), _) => path.clone(),
        (None, Some(path)) => path.with_extension("pdf"),
        (None, None) => PathBuf::from("output.pdf"),
    };

    if let Some(parent) = out_path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).map_err(|e| format!("create output dir: {e}"))?;
    }

    let pdf = compile_typst_to_pdf(&typst_source, &out_path)
        .map_err(|e| format!("compile failed: {e}"))?;

    if options.verbose {
        println!("written {} ({} bytes)", out_path.display(), pdf.len());
    }

    Ok(())
}

fn resolve_force_toc(no_toc: bool, toc: bool) -> Option<bool> {
    if no_toc {
        Some(false)
    } else if toc {
        Some(true)
    } else {
        None
    }
}

fn read_stdin() -> Result<String, String> {
    let mut input = String::new();
    io::stdin()
        .read_to_string(&mut input)
        .map_err(|e| e.to_string())?;
    Ok(input)
}
