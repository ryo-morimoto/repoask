use repoask_core::types::DocSection;

/// Parse a markdown file into sections split by headings.
pub fn parse_markdown(source: &str, filepath: &str) -> Vec<DocSection> {
    let mut sections = Vec::new();
    let mut heading_stack: Vec<(u8, String)> = Vec::new();
    let mut current_title = String::new();
    let mut _current_depth: u8 = 0;
    let mut current_content = String::new();
    let mut current_code_symbols: Vec<String> = Vec::new();
    let mut section_start_line: u32 = 1;

    let mut in_code_block = false;
    let mut code_block_content = String::new();

    for (line_num, line) in source.lines().enumerate() {
        let line_1based = line_num as u32 + 1;

        // Track fenced code block boundaries
        if line.trim_start().starts_with("```") {
            if in_code_block {
                // Ending code block — extract identifiers
                let syms = extract_code_block_identifiers(&code_block_content);
                current_code_symbols.extend(syms);
                code_block_content.clear();
                in_code_block = false;
            } else {
                in_code_block = true;
            }
            current_content.push_str(line);
            current_content.push('\n');
            continue;
        }

        if in_code_block {
            code_block_content.push_str(line);
            code_block_content.push('\n');
            current_content.push_str(line);
            current_content.push('\n');
            continue;
        }

        // Check for heading
        if let Some((depth, title)) = parse_heading(line) {
            // Flush previous section
            if !current_title.is_empty() || !current_content.trim().is_empty() {
                let hierarchy = heading_stack.iter().map(|(_, t)| t.clone()).collect();
                sections.push(DocSection {
                    filepath: filepath.to_string(),
                    section_title: current_title.clone(),
                    heading_hierarchy: hierarchy,
                    content: current_content.trim().to_string(),
                    code_symbols: current_code_symbols.clone(),
                    start_line: section_start_line,
                    end_line: line_1based.saturating_sub(1),
                });
            }

            // Update heading stack: pop everything at same or deeper level
            while heading_stack.last().is_some_and(|(d, _)| *d >= depth) {
                heading_stack.pop();
            }
            heading_stack.push((depth, title.clone()));

            current_title = title;
            _current_depth = depth;
            current_content.clear();
            current_code_symbols.clear();
            section_start_line = line_1based;
            continue;
        }

        current_content.push_str(line);
        current_content.push('\n');
    }

    // Flush last section
    let total_lines = source.lines().count() as u32;
    if !current_title.is_empty() || !current_content.trim().is_empty() {
        let hierarchy = heading_stack.iter().map(|(_, t)| t.clone()).collect();
        sections.push(DocSection {
            filepath: filepath.to_string(),
            section_title: if current_title.is_empty() {
                "Introduction".to_string()
            } else {
                current_title
            },
            heading_hierarchy: hierarchy,
            content: current_content.trim().to_string(),
            code_symbols: current_code_symbols,
            start_line: section_start_line,
            end_line: total_lines,
        });
    }

    // Handle files with no headings at all
    if sections.is_empty() && !source.trim().is_empty() {
        sections.push(DocSection {
            filepath: filepath.to_string(),
            section_title: "Introduction".to_string(),
            heading_hierarchy: vec![],
            content: source.trim().to_string(),
            code_symbols: extract_code_block_identifiers(source),
            start_line: 1,
            end_line: total_lines,
        });
    }

    sections
}

fn parse_heading(line: &str) -> Option<(u8, String)> {
    let trimmed = line.trim_start();
    let hashes = trimmed.bytes().take_while(|&b| b == b'#').count();
    if hashes >= 1 && hashes <= 6 {
        let rest = &trimmed[hashes..];
        if rest.starts_with(' ') {
            let title = rest.trim().to_string();
            if !title.is_empty() {
                return Some((hashes as u8, title));
            }
        }
    }
    None
}

/// Extract identifier-like tokens from a code block.
fn extract_code_block_identifiers(code: &str) -> Vec<String> {
    let mut identifiers = Vec::new();

    for word in code.split(|c: char| !c.is_alphanumeric() && c != '_') {
        let trimmed = word.trim();
        // Only keep things that look like identifiers (start with letter/underscore, 2+ chars)
        if trimmed.len() >= 2
            && trimmed
                .chars()
                .next()
                .is_some_and(|c| c.is_alphabetic() || c == '_')
            && trimmed.chars().all(|c| c.is_alphanumeric() || c == '_')
        {
            identifiers.push(trimmed.to_string());
        }
    }

    identifiers.sort();
    identifiers.dedup();
    identifiers
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "test assertions")]
mod tests {
    use super::*;

    #[test]
    fn test_basic_sections() {
        let md = "# Title\n\nSome intro text.\n\n## Setup\n\nInstall the package.\n";
        let sections = parse_markdown(md, "README.md");
        assert_eq!(sections.len(), 2);
        assert_eq!(sections[0].section_title, "Title");
        assert!(sections[0].content.contains("intro text"));
        assert_eq!(sections[1].section_title, "Setup");
        assert!(sections[1].content.contains("Install"));
    }

    #[test]
    fn test_heading_hierarchy() {
        let md = "# Root\n\n## Child\n\n### Grandchild\n\nDeep content.\n";
        let sections = parse_markdown(md, "doc.md");
        let grandchild = sections
            .iter()
            .find(|s| s.section_title == "Grandchild")
            .unwrap();
        assert_eq!(
            grandchild.heading_hierarchy,
            vec!["Root", "Child", "Grandchild"]
        );
    }

    #[test]
    fn test_code_block_not_split() {
        let md = "# API\n\n```\n# This is not a heading\nfoo()\n```\n";
        let sections = parse_markdown(md, "doc.md");
        // Should be 1 section, not 2 (the # inside code block is not a heading)
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].section_title, "API");
    }

    #[test]
    fn test_code_symbols_extracted() {
        let md = "# Auth\n\n```typescript\nconst token = validateJWT(secret);\n```\n";
        let sections = parse_markdown(md, "doc.md");
        assert!(
            sections[0]
                .code_symbols
                .contains(&"validateJWT".to_string())
        );
        assert!(sections[0].code_symbols.contains(&"token".to_string()));
    }

    #[test]
    fn test_no_headings() {
        let md = "Just some plain text without any headings.";
        let sections = parse_markdown(md, "notes.md");
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].section_title, "Introduction");
    }

    #[test]
    fn test_sibling_headings_reset_hierarchy() {
        let md = "# Root\n\n## A\n\nContent A.\n\n## B\n\nContent B.\n";
        let sections = parse_markdown(md, "doc.md");
        let b = sections.iter().find(|s| s.section_title == "B").unwrap();
        // B should have [Root, B], not [Root, A, B]
        assert_eq!(b.heading_hierarchy, vec!["Root", "B"]);
    }

    #[test]
    fn test_line_numbers() {
        let md = "# Title\n\nLine 3.\n\n## Section Two\n\nLine 7.\n";
        let sections = parse_markdown(md, "doc.md");
        assert_eq!(sections[0].start_line, 1);
        assert_eq!(sections[1].start_line, 5);
    }

    // -----------------------------------------------------------------------
    // Snapshot tests (insta)
    // -----------------------------------------------------------------------

    #[test]
    fn snapshot_readme_like() {
        let md = r#"# repoask

A code search tool.

## Installation

```bash
cargo install repoask
```

## Usage

### Basic search

```typescript
import { search } from "repoask";
const results = search("vercel/next.js", "middleware");
```

### Advanced options

Use `--dir` and `--ext` to filter.

## API Reference

### `search(repo, query)`

Returns matching symbols and documentation.
"#;
        let sections = parse_markdown(md, "README.md");
        insta::assert_json_snapshot!(sections);
    }
}
