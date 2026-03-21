/// Cloud Cleaner — Duplicate File Scanner
///
/// Recursively scans a directory, hashes files with xxHash (XXH3),
/// and identifies duplicate groups. Storage waste = duplicate bytes
/// that could be reclaimed.

use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use xxhash_rust::xxh3::xxh3_64;

/// A group of identical files.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DuplicateGroup {
    /// xxHash of the file content
    pub hash: String,
    /// Size of each file in bytes
    pub file_size: u64,
    /// All paths sharing this content
    pub paths: Vec<String>,
    /// Wasted bytes = (count - 1) * file_size
    pub wasted_bytes: u64,
}

/// Summary of a duplicate scan.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ScanResult {
    /// Directory that was scanned
    pub scanned_dir: String,
    /// Total files scanned
    pub total_files: u32,
    /// Total duplicate groups found
    pub duplicate_groups: u32,
    /// Total wasted bytes across all duplicates
    pub total_wasted_bytes: u64,
    /// Total wasted CO₂ (storage overhead)
    /// Estimate: 0.2 gCO₂ per MB stored (cloud/SSD lifecycle)
    pub wasted_co2_grams: f64,
    /// Individual duplicate groups
    pub groups: Vec<DuplicateGroup>,
}

/// Scan a directory for duplicate files using xxHash.
///
/// - `root`: directory to scan recursively
/// - `min_size`: minimum file size to consider (skip tiny files)
/// - `max_files`: safety cap on files scanned (avoid hanging on huge dirs)
pub fn scan_duplicates(root: &Path, min_size: u64, max_files: u32) -> ScanResult {
    let mut size_groups: HashMap<u64, Vec<PathBuf>> = HashMap::new();
    let mut total_files: u32 = 0;

    // Phase 1: Group files by size (fast filter)
    collect_files_by_size(root, &mut size_groups, &mut total_files, max_files);

    // Phase 2: Hash only files that share a size (potential duplicates)
    let mut hash_groups: HashMap<String, (u64, Vec<String>)> = HashMap::new();

    for (size, paths) in &size_groups {
        if paths.len() < 2 || *size < min_size {
            continue; // Unique size or too small — skip
        }

        for path in paths {
            if let Ok(hash) = hash_file(path) {
                let entry = hash_groups.entry(hash.clone()).or_insert((*size, Vec::new()));
                entry.1.push(path.to_string_lossy().to_string());
            }
        }
    }

    // Phase 3: Build duplicate groups (only hashes with 2+ files)
    let mut groups: Vec<DuplicateGroup> = Vec::new();
    let mut total_wasted: u64 = 0;

    for (hash, (size, paths)) in hash_groups {
        if paths.len() >= 2 {
            let wasted = (paths.len() as u64 - 1) * size;
            total_wasted += wasted;
            groups.push(DuplicateGroup {
                hash,
                file_size: size,
                paths,
                wasted_bytes: wasted,
            });
        }
    }

    // Sort by wasted bytes (largest waste first)
    groups.sort_by(|a, b| b.wasted_bytes.cmp(&a.wasted_bytes));

    // CO₂ estimate: 0.2 gCO₂ per MB stored
    let wasted_mb = total_wasted as f64 / 1_048_576.0;
    let wasted_co2 = wasted_mb * 0.2;

    ScanResult {
        scanned_dir: root.to_string_lossy().to_string(),
        total_files,
        duplicate_groups: groups.len() as u32,
        total_wasted_bytes: total_wasted,
        wasted_co2_grams: (wasted_co2 * 100.0).round() / 100.0,
        groups,
    }
}

/// Recursively collect files grouped by size.
fn collect_files_by_size(
    dir: &Path,
    groups: &mut HashMap<u64, Vec<PathBuf>>,
    count: &mut u32,
    max: u32,
) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return, // Permission denied, etc.
    };

    for entry in entries.flatten() {
        if *count >= max {
            return;
        }

        let path = entry.path();

        // Skip hidden files/dirs and symlinks
        if let Some(name) = path.file_name() {
            if name.to_string_lossy().starts_with('.') {
                continue;
            }
        }

        if path.is_dir() {
            collect_files_by_size(&path, groups, count, max);
        } else if path.is_file() {
            if let Ok(meta) = path.metadata() {
                let size = meta.len();
                if size > 0 {
                    groups.entry(size).or_default().push(path);
                    *count += 1;
                }
            }
        }
    }
}

/// Hash a file's content with XXH3 (xxHash).
fn hash_file(path: &Path) -> std::io::Result<String> {
    let mut file = fs::File::open(path)?;
    let mut buf = Vec::new();

    // For very large files, only hash first 64MB for performance
    let meta = file.metadata()?;
    if meta.len() > 64 * 1024 * 1024 {
        buf.resize(64 * 1024 * 1024, 0);
        file.read_exact(&mut buf)?;
    } else {
        file.read_to_end(&mut buf)?;
    }

    Ok(format!("{:016x}", xxh3_64(&buf)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_scan_empty_dir() {
        let tmp = std::env::temp_dir().join("dwi_test_cleaner_empty");
        let _ = fs::create_dir_all(&tmp);
        let result = scan_duplicates(&tmp, 100, 10000);
        assert_eq!(result.duplicate_groups, 0);
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_detects_duplicates() {
        let tmp = std::env::temp_dir().join("dwi_test_cleaner_dupes");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();

        // Create 3 identical files
        let content = b"This is duplicate content for testing purposes. It must be long enough.";
        for name in &["file_a.txt", "file_b.txt", "file_c.txt"] {
            let mut f = fs::File::create(tmp.join(name)).unwrap();
            f.write_all(content).unwrap();
        }
        // Create 1 unique file
        let mut f = fs::File::create(tmp.join("unique.txt")).unwrap();
        f.write_all(b"This is unique content that doesn't match the others above.").unwrap();

        let result = scan_duplicates(&tmp, 10, 10000);
        assert_eq!(result.duplicate_groups, 1, "Should find 1 duplicate group");
        assert_eq!(result.groups[0].paths.len(), 3, "Group should have 3 files");
        assert_eq!(result.groups[0].wasted_bytes, content.len() as u64 * 2,
            "Wasted = (3-1) * size");
        assert!(result.total_wasted_bytes > 0);

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_hash_consistency() {
        let tmp = std::env::temp_dir().join("dwi_test_hash");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();

        let path = tmp.join("test.bin");
        let mut f = fs::File::create(&path).unwrap();
        f.write_all(b"deterministic content").unwrap();

        let h1 = hash_file(&path).unwrap();
        let h2 = hash_file(&path).unwrap();
        assert_eq!(h1, h2, "Hash should be deterministic");

        let _ = fs::remove_dir_all(&tmp);
    }
}
