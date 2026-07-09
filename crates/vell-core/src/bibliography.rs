// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Samin Yeasar

//! Bibliography management for Vell documents.
//!
//! Parses bibliography entries from YAML-like inline definitions,
//! BibTeX format, and resolves DOIs via content negotiation.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A parsed bibliographic entry with structured fields.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct BibEntry {
    /// Citation key used in `[[key]]` references.
    pub key: String,
    /// Entry type (article, book, inproceedings, phdthesis, etc.).
    pub entry_type: String,
    /// All raw fields from the source.
    pub fields: HashMap<String, String>,
    /// Structured author string (parsed from BibTeX author field).
    pub author: Option<String>,
    /// Publication title.
    pub title: Option<String>,
    /// Publication year.
    pub year: Option<String>,
    /// Journal name (for articles).
    pub journal: Option<String>,
    /// Book title (for book chapters / proceedings).
    pub booktitle: Option<String>,
    /// Publisher name.
    pub publisher: Option<String>,
    /// DOI identifier.
    pub doi: Option<String>,
    /// URL.
    pub url: Option<String>,
    /// Page range.
    pub pages: Option<String>,
    /// Volume number.
    pub volume: Option<String>,
    /// Issue/number.
    pub number: Option<String>,
    /// Editor names.
    pub editor: Option<String>,
    /// Institution or school (for theses).
    pub institution: Option<String>,
    /// Series name.
    pub series: Option<String>,
    /// Address / location.
    pub address: Option<String>,
    /// Edition.
    pub edition: Option<String>,
    /// ISBN.
    pub isbn: Option<String>,
    /// ISSN.
    pub issn: Option<String>,
    /// Abstract.
    pub abstract_: Option<String>,
    /// Note.
    pub note: Option<String>,
    /// Annotation.
    pub annotation: Option<String>,
}

/// A collection of bibliography entries with rendering metadata.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Bibliography {
    /// Citation style name (apa, ieee, chicago, nature, numeric, etc.).
    pub style: String,
    /// Custom title for the reference list.
    pub title: Option<String>,
    /// Parsed bibliography entries keyed by citation key.
    pub entries: HashMap<String, BibEntry>,
    /// Ordered list of citation keys (in document order of definition).
    pub order: Vec<String>,
}

/// Parse a YAML-like source string into a Bibliography.
///
/// Format:
/// ```yaml
/// smith2023:
///   type: article
///   author: Smith, John
///   title: A Study
///   journal: Journal of Examples
///   year: 2023
///   doi: 10.1000/xyz123
/// ```
pub fn parse_bibliography_source(source: &str, style: &str, title: Option<&str>) -> Bibliography {
    let mut bib = Bibliography {
        style: style.to_string(),
        title: title.map(|s| s.to_string()),
        ..Default::default()
    };

    let mut current_key: Option<String> = None;
    let mut current_fields: HashMap<String, String> = HashMap::new();

    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("//") {
            continue;
        }

        // Check if this line starts a new entry (key: at start)
        if !trimmed.starts_with(' ') && !trimmed.starts_with('\t') && trimmed.contains(':') {
            // Save previous entry
            if let Some(ref key) = current_key {
                let entry = build_bib_entry(key, &current_entry_type(&current_fields), &current_fields);
                bib.entries.insert(key.clone(), entry);
                bib.order.push(key.clone());
            }
            current_fields.clear();

            // Parse new key
            let colon_pos = trimmed.find(':').unwrap_or(0);
            current_key = Some(trimmed[..colon_pos].trim().to_string());
            let rest = trimmed[colon_pos + 1..].trim();
            if !rest.is_empty() {
                // Inline type: `key: type`
                current_fields.insert("type".to_string(), rest.to_string());
            }
        } else if let Some(ref key) = current_key {
            // Parse field: `  field: value`
            let trimmed_line = trimmed;
            if let Some(colon_pos) = trimmed_line.find(':') {
                let field_name = trimmed_line[..colon_pos].trim().to_lowercase();
                let field_value = trimmed_line[colon_pos + 1..].trim().to_string();
                if !field_name.is_empty() {
                    current_fields.insert(field_name, field_value);
                }
            }
        }
    }

    // Save last entry
    if let Some(ref key) = current_key {
        let entry = build_bib_entry(key, &current_entry_type(&current_fields), &current_fields);
        bib.entries.insert(key.clone(), entry);
        bib.order.push(key.clone());
    }

    bib
}

/// Parse BibTeX source string into a Bibliography.
///
/// Format:
/// ```bibtex
/// @article{smith2023,
///   author = {Smith, John},
///   title = {A Study},
///   journal = {Journal of Examples},
///   year = {2023}
/// }
/// ```
pub fn parse_bibtex_source(source: &str) -> Bibliography {
    let mut bib = Bibliography::default();
    let mut i = 0;
    let chars: Vec<char> = source.chars().collect();

    while i < chars.len() {
        // Skip whitespace and comments
        while i < chars.len() && (chars[i].is_whitespace() || chars[i] == '%') {
            if chars[i] == '%' {
                while i < chars.len() && chars[i] != '\n' { i += 1; }
            } else {
                i += 1;
            }
        }
        if i >= chars.len() { break; }

        // Look for @type{
        if chars[i] == '@' {
            i += 1;
            let mut entry_type = String::new();
            while i < chars.len() && chars[i].is_alphabetic() {
                entry_type.push(chars[i]);
                i += 1;
            }
            let entry_type = entry_type.to_lowercase();

            // Skip whitespace
            while i < chars.len() && chars[i].is_whitespace() { i += 1; }

            if i < chars.len() && chars[i] == '{' {
                i += 1;
                // Parse key
                while i < chars.len() && chars[i].is_whitespace() { i += 1; }
                let mut key = String::new();
                while i < chars.len() && chars[i] != ',' && chars[i] != '}' {
                    key.push(chars[i]);
                    i += 1;
                }
                let key = key.trim().to_string();

                let mut fields = HashMap::new();
                let mut depth = 0;

                while i < chars.len() {
                    let ch = chars[i];
                    if ch == '{' { depth += 1; }
                    else if ch == '}' {
                        if depth == 0 { break; }
                        depth -= 1;
                    }
                    else if ch == ',' && depth == 0 {
                        // Parse field
                        i += 1;
                        while i < chars.len() && chars[i].is_whitespace() { i += 1; }
                        let mut field_name = String::new();
                        while i < chars.len() && chars[i] != '=' && chars[i] != ',' && chars[i] != '}' {
                            if !chars[i].is_whitespace() { field_name.push(chars[i]); }
                            i += 1;
                        }
                        while i < chars.len() && chars[i].is_whitespace() { i += 1; }
                        if i < chars.len() && chars[i] == '=' {
                            i += 1;
                            while i < chars.len() && chars[i].is_whitespace() { i += 1; }
                            if i < chars.len() && chars[i] == '{' {
                                let value = parse_bibtex_braced_value(&chars, &mut i);
                                fields.insert(field_name.to_lowercase(), value);
                            } else if i < chars.len() && chars[i] == '"' {
                                i += 1;
                                let mut value = String::new();
                                while i < chars.len() && chars[i] != '"' {
                                    value.push(chars[i]);
                                    i += 1;
                                }
                                i += 1; // skip closing "
                                fields.insert(field_name.to_lowercase(), value.trim().to_string());
                            } else {
                                // Number value
                                let mut value = String::new();
                                while i < chars.len() && chars[i] != ',' && chars[i] != '}' && !chars[i].is_whitespace() {
                                    value.push(chars[i]);
                                    i += 1;
                                }
                                fields.insert(field_name.to_lowercase(), value);
                            }
                        }
                        continue;
                    }
                    i += 1;
                }
                i += 1; // skip closing }

                if !key.is_empty() {
                    let entry = build_bib_entry(&key, &entry_type, &fields);
                    if !bib.entries.contains_key(&key) {
                        bib.entries.insert(key.clone(), entry);
                        bib.order.push(key);
                    }
                }
            }
        } else {
            i += 1;
        }
    }

    bib
}

fn parse_bibtex_braced_value(chars: &[char], i: &mut usize) -> String {
    let mut depth = 1;
    let mut value = String::new();
    while *i < chars.len() && depth > 0 {
        if chars[*i] == '{' { depth += 1; if depth > 1 { value.push('{'); } }
        else if chars[*i] == '}' { depth -= 1; if depth > 0 { value.push('}'); } }
        else { value.push(chars[*i]); }
        *i += 1;
    }
    value.trim().to_string()
}

fn current_entry_type(fields: &HashMap<String, String>) -> String {
    fields.get("type").cloned().unwrap_or_else(|| String::from("misc"))
}

fn build_bib_entry(key: &str, entry_type: &str, fields: &HashMap<String, String>) -> BibEntry {
    BibEntry {
        key: key.to_string(),
        entry_type: entry_type.to_string(),
        fields: fields.clone(),
        author: fields.get("author").cloned(),
        title: fields.get("title").cloned(),
        year: fields.get("year").cloned(),
        journal: fields.get("journal").cloned(),
        booktitle: fields.get("booktitle").or_else(|| fields.get("book_title")).cloned(),
        publisher: fields.get("publisher").cloned(),
        doi: fields.get("doi").cloned(),
        url: fields.get("url").cloned(),
        pages: fields.get("pages").cloned(),
        volume: fields.get("volume").cloned(),
        number: fields.get("number").cloned(),
        editor: fields.get("editor").cloned(),
        institution: fields.get("institution").or_else(|| fields.get("school")).cloned(),
        series: fields.get("series").cloned(),
        address: fields.get("address").cloned(),
        edition: fields.get("edition").cloned(),
        isbn: fields.get("isbn").cloned(),
        issn: fields.get("issn").cloned(),
        abstract_: fields.get("abstract").cloned(),
        note: fields.get("note").cloned(),
        annotation: fields.get("annotation").cloned(),
    }
}

/// Returns a formatted citation string for an entry in the given style.
pub fn format_citation(entry: &BibEntry, style: &str, index: Option<usize>) -> String {
    match style {
        "numeric" | "ieee" => {
            if let Some(n) = index {
                format!("[{}]", n + 1)
            } else {
                format!("[{}]", get_author_last(entry))
            }
        }
        "chicago" => {
            let author = get_author_last(entry);
            let year = entry.year.as_deref().unwrap_or("n.d.");
            format!("({} {})", author, year)
        }
        "nature" => {
            if let Some(n) = index {
                format!("{}.", n + 1)
            } else {
                format!("{}.", get_author_last(entry))
            }
        }
        _ => {
            // Default: APA-style author-year
            let author = get_author_last(entry);
            let year = entry.year.as_deref().unwrap_or("n.d.");
            format!("({}, {})", author, year)
        }
    }
}

/// Returns a formatted reference list entry.
pub fn format_reference(entry: &BibEntry, style: &str, index: Option<usize>) -> String {
    let number_prefix = match style {
        "numeric" | "ieee" => {
            if let Some(n) = index {
                format!("[{}] ", n + 1)
            } else {
                String::new()
            }
        }
        "nature" => {
            if let Some(n) = index {
                format!("{}. ", n + 1)
            } else {
                String::new()
            }
        }
        _ => String::new(),
    };

    let authors = entry.author.as_deref().unwrap_or("Unknown");
    let title = entry.title.as_deref().unwrap_or("Untitled");
    let year = entry.year.as_deref().unwrap_or("n.d.");

    match entry.entry_type.as_str() {
        "article" => {
            let journal = entry.journal.as_deref().unwrap_or("Unknown Journal");
            let volume = entry.volume.as_ref().map(|v| format!(" *{}*", v)).unwrap_or_default();
            let pages = entry.pages.as_ref().map(|p| format!(", {}", p)).unwrap_or_default();
            format!("{}{} ({}) \"{}\" *{}*{}{}.", number_prefix, authors, year, title, journal, volume, pages)
        }
        "book" => {
            let publisher = entry.publisher.as_deref().unwrap_or("Unknown Publisher");
            format!("{}{}. ({}) *{}.* {}: {}.", number_prefix, authors, year, title, publisher, entry.address.as_deref().unwrap_or(""))
        }
        "inproceedings" | "inbook" | "incollection" => {
            let booktitle = entry.booktitle.as_deref().unwrap_or("Unknown Proceedings");
            let publisher = entry.publisher.as_deref().unwrap_or("");
            let pages = entry.pages.as_ref().map(|p| format!(", {}", p)).unwrap_or_default();
            let pub_str = if publisher.is_empty() { String::new() } else { format!(" {}:", publisher) };
            format!("{}{}. ({}) \"{}\" In *{}*{}{}.", number_prefix, authors, year, title, booktitle, pub_str, pages)
        }
        "phdthesis" | "mastersthesis" => {
            let school = entry.institution.as_deref().unwrap_or("Unknown Institution");
            format!("{}{}. ({}) \"{}\" (PhD thesis, {}).", number_prefix, authors, year, title, school)
        }
        "misc" | "online" | "webpage" => {
            let url_part = entry.url.as_ref().map(|u| format!(" Retrieved from {}", u)).unwrap_or_default();
            format!("{}{}. ({}) \"{}\".{}{}", number_prefix, authors, year, title, url_part, if entry.doi.is_some() { "" } else { "" })
        }
        _ => {
            format!("{}{}. ({}) \"{}\".", number_prefix, authors, year, title)
        }
    }
}

/// Extracts the last name of the first author for citation purposes.
pub fn get_author_last(entry: &BibEntry) -> String {
    if let Some(ref author) = entry.author {
        // Try "Last, First" format first
        if let Some(comma_pos) = author.find(',') {
            return author[..comma_pos].trim().to_string();
        }
        // Try "First Last" format
        let parts: Vec<&str> = author.split_whitespace().collect();
        if let Some(last) = parts.last() {
            return last.to_string();
        }
        return author.clone();
    }
    "Unknown".to_string()
}

/// Resolves a DOI via content negotiation, returning the CSL-JSON record.
/// Uses the doi.org content negotiation API.
#[cfg(feature = "http")]
pub fn resolve_doi(doi: &str) -> Result<HashMap<String, serde_json::Value>, String> {
    use std::io::Read;

    let url = format!("https://doi.org/{}", doi.trim().trim_start_matches("https://doi.org/"));
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(std::time::Duration::from_secs(10))
        .build();

    let response = agent
        .get(&url)
        .set("Accept", "application/vnd.citationstyles.csl+json")
        .call()
        .map_err(|e| format!("DOI request failed: {}", e))?;

    let mut body = String::new();
    response.into_reader().read_to_string(&mut body)
        .map_err(|e| format!("Failed to read DOI response: {}", e))?;

    serde_json::from_str::<HashMap<String, serde_json::Value>>(&body)
        .map_err(|e| format!("Failed to parse DOI response: {}", e))
}

/// Converts a CSL-JSON record from a DOI resolution into a BibEntry.
#[cfg(feature = "http")]
pub fn csl_json_to_bib_entry(csl: &HashMap<String, serde_json::Value>, key: &str) -> BibEntry {
    let mut entry = BibEntry {
        key: key.to_string(),
        ..Default::default()
    };

    if let Some(v) = csl.get("type") {
        entry.entry_type = v.as_str().unwrap_or("article").to_string();
    }

    if let Some(v) = csl.get("title") {
        entry.title = Some(v.as_str().unwrap_or("").to_string());
    }

    if let Some(v) = csl.get("container-title") {
        entry.journal = Some(v.as_str().unwrap_or("").to_string());
    }

    if let Some(v) = csl.get("publisher") {
        entry.publisher = Some(v.as_str().unwrap_or("").to_string());
    }

    if let Some(v) = csl.get("DOI") {
        entry.doi = Some(v.as_str().unwrap_or("").to_string());
    }

    if let Some(v) = csl.get("URL") {
        entry.url = Some(v.as_str().unwrap_or("").to_string());
    }

    if let Some(v) = csl.get("volume") {
        entry.volume = Some(v.as_str().unwrap_or("").to_string());
    }

    if let Some(v) = csl.get("issue") {
        entry.number = Some(v.as_str().unwrap_or("").to_string());
    }

    if let Some(v) = csl.get("page") {
        entry.pages = Some(v.as_str().unwrap_or("").to_string());
    }

    if let Some(v) = csl.get("issued") {
        if let Some(date_parts) = v.get("date-parts").and_then(|dp| dp.as_array()) {
            if let Some(parts) = date_parts.first().and_then(|p| p.as_array()) {
                if let Some(year) = parts.first().and_then(|y| y.as_i64()) {
                    entry.year = Some(year.to_string());
                }
            }
        }
    }

    if let Some(v) = csl.get("author") {
        if let Some(authors) = v.as_array() {
            let names: Vec<String> = authors.iter().filter_map(|a| {
                let family = a.get("family").and_then(|f| f.as_str());
                let given = a.get("given").and_then(|g| g.as_str());
                match (family, given) {
                    (Some(f), Some(g)) => Some(format!("{}, {}", f, g)),
                    (Some(f), None) => Some(f.to_string()),
                    _ => None,
                }
            }).collect();
            if !names.is_empty() {
                entry.author = Some(names.join(" and "));
            }
        }
    }

    // Map CSL types to BibTeX types
    let type_map: HashMap<&str, &str> = [
        ("article-journal", "article"),
        ("article-magazine", "article"),
        ("article-newspaper", "article"),
        ("book", "book"),
        ("chapter", "incollection"),
        ("paper-conference", "inproceedings"),
        ("thesis", "phdthesis"),
        ("report", "techreport"),
        ("webpage", "misc"),
        ("dataset", "misc"),
        ("patent", "misc"),
    ].iter().cloned().collect();

    if let Some(bt) = type_map.get(entry.entry_type.as_str()) {
        entry.entry_type = bt.to_string();
    }

    entry
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_yaml_source() {
        let source = "\
smith2023:
  type: article
  author: Smith, John
  title: A Study of Examples
  journal: Journal of Examples
  year: 2023
  doi: 10.1000/xyz123

doe2024:
  type: book
  author: Doe, Jane
  title: The Big Book
  publisher: Academic Press
  year: 2024
";
        let bib = parse_bibliography_source(source, "apa", None);
        assert_eq!(bib.entries.len(), 2);
        assert!(bib.entries.contains_key("smith2023"));
        assert!(bib.entries.contains_key("doe2024"));

        let smith = &bib.entries["smith2023"];
        assert_eq!(smith.author.as_deref(), Some("Smith, John"));
        assert_eq!(smith.title.as_deref(), Some("A Study of Examples"));
        assert_eq!(smith.year.as_deref(), Some("2023"));
        assert_eq!(smith.journal.as_deref(), Some("Journal of Examples"));
    }

    #[test]
    fn test_parse_bibtex() {
        let source = "\
@article{smith2023,
  author = {Smith, John},
  title  = {A Study of Examples},
  journal = {Journal of Examples},
  year   = {2023},
  doi    = {10.1000/xyz123}
}

@book{doe2024,
  author    = {Doe, Jane},
  title     = {The Big Book},
  publisher = {Academic Press},
  year      = {2024}
}
";
        let bib = parse_bibtex_source(source);
        assert_eq!(bib.entries.len(), 2);
        assert!(bib.entries.contains_key("smith2023"));
        assert!(bib.entries.contains_key("doe2024"));

        let smith = &bib.entries["smith2023"];
        assert_eq!(smith.author.as_deref(), Some("Smith, John"));
        assert_eq!(smith.year.as_deref(), Some("2023"));
        assert_eq!(smith.entry_type, "article");
    }

    #[test]
    fn test_format_citation_apa() {
        let entry = BibEntry {
            key: "test".to_string(),
            entry_type: "article".to_string(),
            author: Some("Smith, John".to_string()),
            year: Some("2023".to_string()),
            ..Default::default()
        };
        let citation = format_citation(&entry, "apa", None);
        assert_eq!(citation, "(Smith, 2023)");
    }

    #[test]
    fn test_format_reference() {
        let entry = BibEntry {
            key: "test".to_string(),
            entry_type: "article".to_string(),
            author: Some("Smith, John".to_string()),
            title: Some("A Study".to_string()),
            year: Some("2023".to_string()),
            journal: Some("Journal of Examples".to_string()),
            volume: Some("42".to_string()),
            pages: Some("123-145".to_string()),
            ..Default::default()
        };
        let ref_text = format_reference(&entry, "apa", None);
        assert!(ref_text.contains("Smith, John"));
        assert!(ref_text.contains("A Study"));
        assert!(ref_text.contains("Journal of Examples"));
    }

    #[test]
    fn test_empty_bibliography() {
        let bib = parse_bibliography_source("", "apa", None);
        assert_eq!(bib.entries.len(), 0);
    }
}
