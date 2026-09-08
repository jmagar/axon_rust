use super::*;

#[test]
fn ranged_window_allocates_only_the_window_not_the_original_section() {
    let source = "élève body ".repeat(50_000);
    let chunk = DocumentChunk::new(
        source.clone(),
        crate::text::source_range(&source, 0, source.len()),
    )
    .with_title("Title")
    .with_heading_path(vec!["Parent".into(), "Title".into()])
    .with_symbol("symbol")
    .with_metadata("retained", true.into());
    let positions = SourcePositions::new(&source);
    let end = "élève body ".len() * 10;
    let (window, work) = crate::performance_measurement::measure(|| {
        ranged_clone(&source, &positions, &chunk, 0, 0, end).unwrap()
    });
    assert!(
        work.allocated_bytes < 16_384,
        "small window allocated {} bytes from a {}-byte section",
        work.allocated_bytes,
        source.len()
    );
    assert_eq!(window.content, source[..end].trim());
    assert_eq!(window.title, chunk.title);
    assert_eq!(window.heading_path, chunk.heading_path);
    assert_eq!(window.symbol, chunk.symbol);
    assert_eq!(window.metadata, chunk.metadata);
    assert_eq!(window.range, crate::text::source_range(&source, 0, end - 1));
}
