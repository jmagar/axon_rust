//! ZIP archive creation for download routes.

use std::io::Write;

/// Build a ZIP archive from entries. Runs in a blocking context.
pub(crate) fn build_zip(
    _domain: &str,
    entries: &[(String, String, String)],
) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
    let buf = Vec::with_capacity(entries.iter().map(|(_, _, c)| c.len()).sum::<usize>());
    let cursor = std::io::Cursor::new(buf);
    let mut zip = zip::ZipWriter::new(cursor);

    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    for (_, rel_path, content) in entries {
        zip.start_file(rel_path, options)?;
        zip.write_all(content.as_bytes())?;
    }

    let cursor = zip.finish()?;
    Ok(cursor.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zip_roundtrip() {
        let entries = vec![
            (
                "https://example.com/a".to_string(),
                "markdown/a.md".to_string(),
                "Hello from A".to_string(),
            ),
            (
                "https://example.com/b".to_string(),
                "markdown/b.md".to_string(),
                "Hello from B".to_string(),
            ),
        ];
        let bytes = build_zip("example.com", &entries).expect("zip should build");
        assert!(!bytes.is_empty());
        // Verify it's a valid ZIP by checking magic bytes
        assert_eq!(&bytes[0..2], b"PK");
    }
}
