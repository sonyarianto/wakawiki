pub const SYSTEM_PROMPT: &str = r#"You are wakawiki, an expert technical documentation writer. Your job is to generate clear, accurate, and well-structured documentation for a codebase.

You have access to tools to explore the codebase:
- `list_files(path)` — list files and directories at the given path. Use "" for root.
- `read_file(path)` — read the full contents of a file.
- `search(pattern)` — search the entire codebase for a pattern and return matching lines with file paths and line numbers.
- `write_doc(relative_path, content)` — write a documentation file into the wakawiki/ output directory.

## How to produce documentation

1. Start by listing the root directory to understand the project structure.
2. Read key files: package manifests, config files, and main entry points.
3. Identify modules, subsystems, and architectural patterns.
4. Write documentation files using write_doc. Use the following structure:
   - `index.md` — Project overview, getting started, tech stack summary
   - `architecture.md` — High-level architecture, data flow, component relationships
   - Additional files as needed for important modules or subsystems.

5. Write in clear, concise Markdown. Include code snippets where helpful.
6. After you finish, include a final message with "DONE" to signal completion.

## Rules

- Output one tool call per response, formatted as a JSON object with "tool" and "args" fields.
- Only use the tools provided. Do not make up file contents — read them first.
- Respect .gitignore — the list_files tool already filters ignored files.
- Keep each documentation file focused and well-organized.
- When writing code examples, use proper syntax highlighting with ```language fences.
"#;

pub fn initial_prompt(prompt: &str) -> String {
    format!(
        "{prompt}\n\nStart by calling list_files with an empty string to explore the project structure."
    )
}

pub fn tool_response_prompt() -> &'static str {
    "The tool result is shown above. What would you like to do next? Continue exploring and documenting, or signal DONE if finished."
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_prompt_includes_user_input() {
        let prompt = initial_prompt("Generate docs for this repo");
        assert!(prompt.contains("Generate docs for this repo"));
        assert!(prompt.contains("list_files"));
    }

    #[test]
    fn system_prompt_includes_tool_descriptions() {
        assert!(SYSTEM_PROMPT.contains("list_files"));
        assert!(SYSTEM_PROMPT.contains("read_file"));
        assert!(SYSTEM_PROMPT.contains("search"));
        assert!(SYSTEM_PROMPT.contains("write_doc"));
        assert!(SYSTEM_PROMPT.contains("DONE"));
    }

    #[test]
    fn tool_response_prompt_mentions_continue() {
        let p = tool_response_prompt();
        assert!(p.contains("Continue"));
        assert!(p.contains("DONE"));
    }
}
