use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process;

use clap::{Args, Parser, Subcommand};
use mdxport::{
    compile::compile_typst_to_pdf,
    convert::{ConvertOptions, convert_markdown_to_typst},
    frontmatter::{ParsedMarkdown, split_frontmatter},
    template::{Style, compose_document},
    watch::{WatchCommand, watch_inputs},
};

mod update;

#[derive(Debug, Parser)]
#[command(name = "mdxport")]
#[command(version)]
#[command(about = "Markdown to Typst PDF converter")]
#[command(subcommand_precedence_over_arg = true)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    #[command(flatten)]
    convert: ConvertArgs,
}

#[derive(Debug, Subcommand)]
enum Command {
    Convert(ConvertArgs),
    Fonts(FontsArgs),
}

#[derive(Debug, Args, Clone)]
struct ConvertArgs {
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

    #[arg(
        long = "template",
        help = "Path to a custom Typst template file (.typ)."
    )]
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

    #[arg(
        long,
        default_value_t = false,
        help = "Suppress non-essential output (including update checks)."
    )]
    quiet: bool,
}

#[derive(Debug, Args)]
struct FontsArgs {
    #[command(subcommand)]
    command: FontsCommand,
}

#[derive(Debug, Subcommand)]
enum FontsCommand {
    Install,
    List,
}

const CJK_FONT_WARNING: &str = "Warning: CJK characters detected but no CJK fonts found. Run mdxport fonts install to download Noto CJK fonts (~60MB).";
const FONT_DOWNLOADS: [(&str, &str); 4] = [
    (
        "NotoSansCJKsc-Regular.otf",
        "https://github.com/notofonts/noto-cjk/raw/main/Sans/OTF/SimplifiedChinese/NotoSansCJKsc-Regular.otf",
    ),
    (
        "NotoSansCJKsc-Bold.otf",
        "https://github.com/notofonts/noto-cjk/raw/main/Sans/OTF/SimplifiedChinese/NotoSansCJKsc-Bold.otf",
    ),
    (
        "NotoSerifCJKsc-Regular.otf",
        "https://github.com/notofonts/noto-cjk/raw/main/Serif/OTF/SimplifiedChinese/NotoSerifCJKsc-Regular.otf",
    ),
    (
        "NotoSerifCJKsc-Bold.otf",
        "https://github.com/notofonts/noto-cjk/raw/main/Serif/OTF/SimplifiedChinese/NotoSerifCJKsc-Bold.otf",
    ),
];

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
    has_user_fonts: bool,
}

fn main() {
    let cli = Cli::parse();

    // No subcommand, no input files, and stdin is a terminal → show help
    if cli.command.is_none() && cli.convert.inputs.is_empty() && atty::is(atty::Stream::Stdin) {
        Cli::parse_from(["mdxport", "--help"]);
        return;
    }

    if let Err(error) = run(cli) {
        eprintln!("[mdxport] {error}");
        process::exit(1);
    }
}

fn run(cli: Cli) -> Result<(), String> {
    let Cli { command, convert } = cli;
    match command {
        Some(Command::Fonts(fonts)) => run_fonts(fonts),
        Some(Command::Convert(convert)) => run_convert(convert),
        None => run_convert(convert),
    }
}

fn run_convert(cli: ConvertArgs) -> Result<(), String> {
    let ConvertArgs {
        inputs,
        output,
        style,
        custom_template,
        title,
        author,
        lang,
        toc,
        no_toc,
        watch,
        verbose,
        quiet,
    } = cli;

    if inputs.is_empty() && watch {
        return Err("watch mode requires at least one input file".to_string());
    }

    let multiple_inputs = inputs.len() > 1;
    if multiple_inputs
        && let Some(output) = &output
        && output.extension().is_some()
    {
        return Err("multiple input files require output directory path".to_string());
    }

    let style = Style::try_from(style.as_str()).map_err(|e| e.to_string())?;
    let force_toc = resolve_force_toc(no_toc, toc);

    let input_sources = if inputs.is_empty() {
        vec![InputSource::Stdin(read_stdin()?)]
    } else {
        inputs
            .into_iter()
            .map(InputSource::File)
            .collect::<Vec<_>>()
    };

    if watch {
        let files = input_sources
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
            output: output.clone(),
            multiple_inputs,
            title_override: title.clone(),
            author_override: author.clone(),
            lang_override: lang.clone(),
            force_toc,
            verbose,
        };

        return watch_inputs(&files, &command).map_err(|e| format!("watch failed: {e}"));
    }

    let custom_template = custom_template
        .map(|p| fs::read_to_string(&p).map_err(|e| format!("read template: {e}")))
        .transpose()?;

    let process_options = ProcessOptions {
        output: &output,
        title: &title,
        author: &author,
        lang: &lang,
        force_toc,
        verbose,
        style,
        custom_template,
        multiple_inputs,
        has_user_fonts: user_font_dir_has_font_files(),
    };

    let mut warned_about_missing_fonts = false;
    input_sources.iter().try_for_each(|input| {
        process_one(input, &process_options, &mut warned_about_missing_fonts)
    })?;

    if !quiet {
        update::check_for_updates();
    }

    Ok(())
}

fn run_fonts(fonts: FontsArgs) -> Result<(), String> {
    match fonts.command {
        FontsCommand::Install => install_fonts(),
        FontsCommand::List => list_fonts(),
    }
}

fn process_one(
    input: &InputSource,
    options: &ProcessOptions<'_>,
    warned_about_missing_fonts: &mut bool,
) -> Result<(), String> {
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

    maybe_warn_missing_cjk_fonts(&source, options.has_user_fonts, warned_about_missing_fonts);

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

fn install_fonts() -> Result<(), String> {
    let font_dir = user_font_dir()?;
    fs::create_dir_all(&font_dir).map_err(|e| format!("create fonts dir: {e}"))?;

    let client = reqwest::blocking::Client::new();
    let mut downloaded_any = false;

    for (file_name, url) in FONT_DOWNLOADS {
        let target = font_dir.join(file_name);
        if target.is_file() {
            println!("{file_name} already installed, skipping.");
            continue;
        }

        download_font(&client, url, file_name, &target)?;
        downloaded_any = true;
    }

    if downloaded_any {
        println!("Fonts installed. CJK rendering ready.");
    } else {
        println!("Fonts already installed.");
    }

    Ok(())
}

fn list_fonts() -> Result<(), String> {
    let font_dir = user_font_dir()?;
    let fonts = list_user_font_files(&font_dir).map_err(|e| format!("list fonts: {e}"))?;

    if fonts.is_empty() {
        println!("No fonts found in {}", font_dir.display());
        return Ok(());
    }

    for font in fonts {
        println!("{}", font.display());
    }

    Ok(())
}

fn download_font(
    client: &reqwest::blocking::Client,
    url: &str,
    file_name: &str,
    destination: &Path,
) -> Result<(), String> {
    let mut response = client
        .get(url)
        .send()
        .map_err(|e| format!("download {file_name}: {e}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "download {file_name}: server returned {}",
            response.status()
        ));
    }

    let temp_path = destination.with_extension("part");
    let mut output =
        fs::File::create(&temp_path).map_err(|e| format!("create {file_name}: {e}"))?;

    let total = response.content_length();
    let mut downloaded = 0_u64;
    let mut buf = [0_u8; 64 * 1024];

    loop {
        let count = response
            .read(&mut buf)
            .map_err(|e| format!("read {file_name}: {e}"))?;
        if count == 0 {
            break;
        }

        output
            .write_all(&buf[..count])
            .map_err(|e| format!("write {file_name}: {e}"))?;
        downloaded += count as u64;
        print_download_progress(file_name, downloaded, total);
    }

    output
        .flush()
        .map_err(|e| format!("flush {file_name}: {e}"))?;

    fs::rename(&temp_path, destination).map_err(|e| format!("finalize {file_name}: {e}"))?;
    eprintln!();

    Ok(())
}

fn print_download_progress(file_name: &str, downloaded: u64, total: Option<u64>) {
    match total {
        Some(total) if total > 0 => {
            let percent = (downloaded as f64 / total as f64) * 100.0;
            eprint!("\rDownloading {file_name}: {percent:.1}% ({downloaded}/{total} bytes)");
        }
        _ => {
            eprint!("\rDownloading {file_name}: {downloaded} bytes");
        }
    }
    let _ = io::stderr().flush();
}

fn maybe_warn_missing_cjk_fonts(markdown: &str, has_user_fonts: bool, warned: &mut bool) {
    if !*warned && !has_user_fonts && contains_cjk_char(markdown) {
        eprintln!("{CJK_FONT_WARNING}");
        *warned = true;
    }
}

fn contains_cjk_char(text: &str) -> bool {
    text.chars().any(|ch| {
        let code = ch as u32;
        (0x4E00..=0x9FFF).contains(&code)
            || (0x3040..=0x309F).contains(&code)
            || (0x30A0..=0x30FF).contains(&code)
            || (0xAC00..=0xD7AF).contains(&code)
            || (0x3000..=0x303F).contains(&code)
    })
}

fn user_font_dir_has_font_files() -> bool {
    let Ok(font_dir) = user_font_dir() else {
        return false;
    };
    list_user_font_files(&font_dir)
        .map(|fonts| !fonts.is_empty())
        .unwrap_or(false)
}

fn list_user_font_files(dir: &Path) -> io::Result<Vec<PathBuf>> {
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut files = Vec::new();
    collect_user_font_files(dir, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_user_font_files(dir: &Path, out: &mut Vec<PathBuf>) -> io::Result<()> {
    let entries = fs::read_dir(dir)?;
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_user_font_files(&path, out)?;
            continue;
        }
        if is_supported_user_font_file(&path) {
            out.push(path);
        }
    }
    Ok(())
}

fn is_supported_user_font_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| matches!(ext.to_ascii_lowercase().as_str(), "otf" | "ttf"))
        .unwrap_or(false)
}

fn user_font_dir() -> Result<PathBuf, String> {
    home_dir()
        .map(|home| home.join(".mdxport").join("fonts"))
        .ok_or_else(|| "unable to determine home directory".to_string())
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}
