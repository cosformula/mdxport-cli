use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FrontMatter {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub authors: Vec<String>,
    #[serde(default)]
    pub lang: Option<String>,
    #[serde(default)]
    pub toc: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct ParsedMarkdown {
    pub frontmatter: FrontMatter,
    pub body: String,
}

#[derive(Debug)]
pub struct FrontMatterError {
    message: String,
}

impl std::fmt::Display for FrontMatterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for FrontMatterError {}

pub fn split_frontmatter(input: &str) -> Result<ParsedMarkdown, FrontMatterError> {
    let normalized = input.trim_start_matches('\u{feff}');
    let mut lines = normalized.lines();
    let first = lines.next();

    if first != Some("---") {
        return Ok(ParsedMarkdown {
            frontmatter: FrontMatter::default(),
            body: normalized.to_string(),
        });
    }

    let mut frontmatter_block = String::new();
    let mut found_end = false;
    let mut remaining_lines: Vec<&str> = Vec::new();

    for line in lines {
        if !found_end {
            if line == "---" {
                found_end = true;
                continue;
            }
            frontmatter_block.push_str(line);
            frontmatter_block.push('\n');
        } else {
            remaining_lines.push(line);
        }
    }

    if !found_end {
        return Err(FrontMatterError {
            message: "frontmatter must have opening and closing ---".to_string(),
        });
    }

    let mut frontmatter = FrontMatter::default();
    if !frontmatter_block.trim().is_empty() {
        frontmatter = serde_yaml::from_str(&frontmatter_block).map_err(|e| FrontMatterError {
            message: format!("yaml parse error: {e}"),
        })?;
    }

    Ok(ParsedMarkdown {
        frontmatter,
        body: remaining_lines.join("\n"),
    })
}
