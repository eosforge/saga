//! Incremental file ingestion without blocking the window thread.

use std::io::{self, Read};
use std::path::Path;

/// Rejects unsupported formats before interpreting binary content as TXT.
pub(super) fn validate(path: &Path) -> Result<(), String> {
    if path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("txt"))
    {
        Ok(())
    } else {
        Err("目前仅支持 TXT 文件；EPUB 解析尚未实现".into())
    }
}

/// Reports bytes actually read, including zero and the final byte count.
pub(super) fn read(mut source: impl Read, total: u64, mut progress: impl FnMut(u64, u64)) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    let mut buffer = vec![0; 256 * 1024];
    progress(0, total);
    loop {
        let count = match source.read(&mut buffer) {
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            result => result?,
        };
        if count == 0 {
            break;
        }
        bytes.extend_from_slice(&buffer[..count]);
        progress(bytes.len() as u64, total);
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn reports_incremental_reads_without_changing_content() {
        let input = vec![42; 800_000];
        let mut updates = Vec::new();
        let output = read(Cursor::new(&input), input.len() as u64, |done, total| {
            updates.push((done, total));
        })
        .unwrap();
        assert_eq!(output, input);
        assert_eq!(updates.first(), Some(&(0, 800_000)));
        assert_eq!(updates.last(), Some(&(800_000, 800_000)));
        assert!(updates.iter().any(|&(done, _)| done > 0 && done < 800_000));
        assert!(updates.windows(2).all(|pair| pair[0].0 < pair[1].0));
    }

    #[test]
    fn handles_empty_and_failed_reads() {
        assert!(read(Cursor::new([]), 0, |_, _| {}).unwrap().is_empty());
        struct Failure;
        impl Read for Failure {
            fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
                Err(io::Error::new(io::ErrorKind::PermissionDenied, "fixture"))
            }
        }
        let error = read(Failure, 10, |_, _| {}).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    }

    #[test]
    fn rejects_unsupported_formats_without_reading_the_source() {
        assert!(validate(Path::new("book.TXT")).is_ok());
        assert!(validate(Path::new("book.epub")).is_err());
        assert!(validate(Path::new("book")).is_err());
    }
}
