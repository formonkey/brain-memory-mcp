use anyhow::{bail, Result};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TextChunk {
    pub index: usize,
    pub text: String,
    pub start: usize,
    pub end: usize,
}

pub fn chunk_markdown(text: &str, chunk_size: usize, overlap: usize) -> Result<Vec<TextChunk>> {
    if chunk_size < 200 {
        bail!("chunk_size must be at least 200 characters");
    }
    if overlap >= chunk_size {
        bail!("overlap must be smaller than chunk_size");
    }

    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    let normalized = normalized.trim();
    if normalized.is_empty() {
        return Ok(Vec::new());
    }

    let sections = split_on_markdown_boundaries(normalized);
    let mut chunks = Vec::new();
    let mut buffer = String::new();
    let mut buffer_start = 0usize;
    let mut search_offset = 0usize;

    for section in sections {
        if buffer.is_empty() {
            buffer_start = find_from(normalized, &section, search_offset).unwrap_or(0);
        }
        let candidate = if buffer.is_empty() {
            section.clone()
        } else {
            format!("{}\n\n{}", buffer, section).trim().to_string()
        };

        if candidate.len() <= chunk_size {
            buffer = candidate;
            search_offset = search_offset.max(buffer_start + buffer.len());
            continue;
        }

        if !buffer.is_empty() {
            chunks.extend(slice_chunk(
                &buffer,
                chunks.len(),
                buffer_start,
                chunk_size,
                overlap,
            ));
        }
        let section_start = find_from(normalized, &section, search_offset).unwrap_or(search_offset);
        chunks.extend(slice_chunk(
            &section,
            chunks.len(),
            section_start,
            chunk_size,
            overlap,
        ));
        buffer.clear();
        search_offset = search_offset.max(section_start + section.len());
    }

    if !buffer.is_empty() {
        chunks.extend(slice_chunk(
            &buffer,
            chunks.len(),
            buffer_start,
            chunk_size,
            overlap,
        ));
    }

    Ok(chunks)
}

fn split_on_markdown_boundaries(text: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let mut current = Vec::new();

    for line in text.lines() {
        if line.starts_with('#') && !current.is_empty() {
            blocks.push(current.join("\n").trim().to_string());
            current.clear();
        }
        current.push(line);
    }

    if !current.is_empty() {
        blocks.push(current.join("\n").trim().to_string());
    }

    blocks
        .into_iter()
        .filter(|block| !block.is_empty())
        .collect()
}

fn slice_chunk(
    text: &str,
    start_index: usize,
    absolute_start: usize,
    chunk_size: usize,
    overlap: usize,
) -> Vec<TextChunk> {
    let mut chunks = Vec::new();
    let mut cursor = 0usize;

    while cursor < text.len() {
        let mut end = (cursor + chunk_size).min(text.len());
        while end > cursor && !text.is_char_boundary(end) {
            end -= 1;
        }

        if end < text.len() {
            let min_boundary = cursor + (chunk_size as f32 * 0.55) as usize;
            if let Some(boundary) = best_boundary(text, cursor, end, min_boundary) {
                end = boundary;
            }
        }

        let chunk_text = text[cursor..end].trim().to_string();
        if !chunk_text.is_empty() {
            chunks.push(TextChunk {
                index: start_index + chunks.len(),
                text: chunk_text,
                start: absolute_start + cursor,
                end: absolute_start + end,
            });
        }

        if end >= text.len() {
            break;
        }
        cursor = end.saturating_sub(overlap).max(cursor + 1);
        while cursor < text.len() && !text.is_char_boundary(cursor) {
            cursor += 1;
        }
    }

    chunks
}

fn best_boundary(text: &str, cursor: usize, end: usize, min_boundary: usize) -> Option<usize> {
    ["\n\n", "\n", ". ", " "]
        .iter()
        .filter_map(|needle| {
            text[cursor..end]
                .rfind(needle)
                .map(|idx| (cursor + idx, *needle))
        })
        .filter(|(idx, _)| *idx >= min_boundary)
        .max_by_key(|(idx, _)| *idx)
        .map(|(idx, needle)| idx + needle.len())
}

fn find_from(haystack: &str, needle: &str, offset: usize) -> Option<usize> {
    haystack.get(offset..)?.find(needle).map(|idx| idx + offset)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunks_prefer_headings() {
        let text = format!(
            "# Intro\n\nSmall intro.\n\n## Details\n\n{}",
            "alpha beta gamma. ".repeat(80)
        );
        let chunks = chunk_markdown(&text, 320, 40).unwrap();
        assert!(!chunks.is_empty());
        assert_eq!(chunks[0].index, 0);
        assert!(chunks[0].text.contains("# Intro"));
    }
}
