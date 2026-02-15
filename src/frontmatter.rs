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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_frontmatter() {
        let parsed = split_frontmatter("# Hello\n\nWorld").unwrap();
        assert!(parsed.frontmatter.title.is_none());
        assert_eq!(parsed.body, "# Hello\n\nWorld");
    }

    #[test]
    fn empty_frontmatter() {
        let parsed = split_frontmatter("---\n---\n# Hello").unwrap();
        assert!(parsed.frontmatter.title.is_none());
        assert_eq!(parsed.body, "# Hello");
    }

    #[test]
    fn full_frontmatter() {
        let input = "---\ntitle: My Title\nauthor: Alice\nlang: zh\ntoc: true\n---\nBody";
        let parsed = split_frontmatter(input).unwrap();
        assert_eq!(parsed.frontmatter.title.as_deref(), Some("My Title"));
        assert_eq!(parsed.frontmatter.author.as_deref(), Some("Alice"));
        assert_eq!(parsed.frontmatter.lang.as_deref(), Some("zh"));
        assert_eq!(parsed.frontmatter.toc, Some(true));
        assert_eq!(parsed.body, "Body");
    }

    #[test]
    fn multiple_authors() {
        let input = "---\nauthors:\n  - Alice\n  - Bob\n---\nBody";
        let parsed = split_frontmatter(input).unwrap();
        assert_eq!(parsed.frontmatter.authors, vec!["Alice", "Bob"]);
    }

    #[test]
    fn unclosed_frontmatter_errors() {
        let result = split_frontmatter("---\ntitle: Oops\nno closing");
        assert!(result.is_err());
    }

    #[test]
    fn invalid_yaml_errors() {
        let result = split_frontmatter("---\n: [invalid\n---\nBody");
        assert!(result.is_err());
    }

    #[test]
    fn bom_stripped() {
        let input = "\u{feff}---\ntitle: BOM\n---\nBody";
        let parsed = split_frontmatter(input).unwrap();
        assert_eq!(parsed.frontmatter.title.as_deref(), Some("BOM"));
    }

    #[test]
    fn unknown_fields_ignored() {
        let input = "---\ntitle: Test\ncustom_field: whatever\n---\nBody";
        let parsed = split_frontmatter(input).unwrap();
        assert_eq!(parsed.frontmatter.title.as_deref(), Some("Test"));
    }
}
