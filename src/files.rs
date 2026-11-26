//! Folder tree and find-in-files support for the side panel.

use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::search::{Finder, SearchOptions};

#[derive(Clone, Debug)]
pub struct Entry {
    pub path: PathBuf,
    pub name: String,
    pub is_dir: bool,
}

/// Lazily read folder tree. Directory listings are cached until refreshed.
pub struct FileTree {
    pub root: PathBuf,
    cache: HashMap<PathBuf, Vec<Entry>>,
}

impl FileTree {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            cache: HashMap::new(),
        }
    }

    pub fn name(&self) -> String {
        self.root
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.root.display().to_string())
    }

    pub fn refresh(&mut self) {
        self.cache.clear();
    }

    pub fn entries(&mut self, dir: &Path) -> &[Entry] {
        if !self.cache.contains_key(dir) {
            self.cache.insert(dir.to_path_buf(), read_dir_sorted(dir));
        }
        self.cache.get(dir).map(|v| v.as_slice()).unwrap_or(&[])
    }
}

fn read_dir_sorted(dir: &Path) -> Vec<Entry> {
    let mut entries: Vec<Entry> = match fs::read_dir(dir) {
        Ok(read) => read
            .flatten()
            .filter_map(|entry| {
                let path = entry.path();
                let name = entry.file_name().to_string_lossy().into_owned();
                if name.starts_with('.') {
                    return None;
                }
                let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
                Some(Entry { path, name, is_dir })
            })
            .collect(),
        Err(_) => Vec::new(),
    };
    entries.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    entries.truncate(1000);
    entries
}

#[derive(Clone, Debug)]
pub struct FileHit {
    pub path: PathBuf,
    pub line: usize,
    pub column: usize,
    pub line_text: String,
}

/// Walk a folder and return matching lines. Bounded by `limit` hits and a
/// 2 MiB per-file size limit.
pub fn find_in_files(
    root: &Path,
    query: &str,
    options: &SearchOptions,
    limit: usize,
) -> io::Result<Vec<FileHit>> {
    let mut hits = Vec::new();
    if query.is_empty() {
        return Ok(hits);
    }
    let mut finder = Finder::new();
    walk(root, 0, &mut |path| {
        if hits.len() >= limit {
            return;
        }
        let Ok(metadata) = fs::metadata(path) else {
            return;
        };
        if metadata.len() > 2 * 1024 * 1024 {
            return;
        }
        let Ok(bytes) = fs::read(path) else {
            return;
        };
        if bytes.iter().take(4096).any(|b| *b == 0) {
            return;
        }
        let Ok(text) = std::str::from_utf8(&bytes) else {
            return;
        };
        let mut line_no = 1usize;
        let mut last_line_start = 0usize;
        for range in finder.matches(text, query, options) {
            let line_start = text[..range.start].rfind('\n').map(|i| i + 1).unwrap_or(0);
            if line_start > last_line_start {
                line_no += text[last_line_start..line_start].matches('\n').count();
                last_line_start = line_start;
            }
            let line_end = text[range.start..]
                .find('\n')
                .map(|i| range.start + i)
                .unwrap_or(text.len());
            let line_text = text[line_start..line_end].replace('\t', "    ");
            let column = text[line_start..range.start].chars().count() + 1;
            hits.push(FileHit {
                path: path.to_path_buf(),
                line: line_no,
                column,
                line_text,
            });
            if hits.len() >= limit {
                return;
            }
        }
    })?;
    Ok(hits)
}

fn walk(dir: &Path, depth: usize, visit: &mut dyn FnMut(&Path)) -> io::Result<()> {
    if depth > 12 {
        return Ok(());
    }
    let mut entries: Vec<(PathBuf, bool)> = Vec::new();
    for entry in fs::read_dir(dir)?.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        let Ok(ft) = entry.file_type() else {
            continue;
        };
        if ft.is_dir() {
            if matches!(
                name.as_str(),
                ".git"
                    | ".svn"
                    | ".hg"
                    | "node_modules"
                    | "target"
                    | "build"
                    | "dist"
                    | ".venv"
                    | "__pycache__"
            ) {
                continue;
            }
            entries.push((path, true));
        } else if ft.is_file() {
            entries.push((path, false));
        }
    }
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    for (path, is_dir) in entries {
        if is_dir {
            walk(&path, depth + 1, visit)?;
        } else {
            visit(&path);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_lines_with_positions() {
        let dir = std::env::current_dir()
            .unwrap()
            .join("target")
            .join(format!("nprs-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("a.txt");
        fs::write(&file, "alpha\nbeta alpha\n").unwrap();
        let hits = find_in_files(&dir, "alpha", &SearchOptions::default(), 10).unwrap();
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].line, 1);
        assert_eq!(hits[1].line, 2);
        assert_eq!(hits[1].column, 6);
        let _ = fs::remove_dir_all(&dir);
    }
}
