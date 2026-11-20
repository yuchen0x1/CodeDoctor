#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn deflate_zip() -> Vec<u8> {
        let mut writer = ::zip::ZipWriter::new(Cursor::new(Vec::new()));
        writer
            .start_file(
                "src/中文.py",
                ::zip::write::SimpleFileOptions::default()
                    .compression_method(::zip::CompressionMethod::Deflated),
            )
            .unwrap();
        writer.write_all(b"print('offline')\n").unwrap();
        writer.finish().unwrap().into_inner()
    }

    #[test]
    fn deflated_archive_extracts_exact_source_bytes() {
        let root = tempdir();
        let archive = root.join("project.zip");
        std::fs::write(&archive, deflate_zip()).unwrap();
        let extracted = extract_zip(&archive).unwrap();
        assert_eq!(
            std::fs::read(extracted.join("src/中文.py")).unwrap(),
            b"print('offline')\n"
        );
        std::fs::remove_dir_all(extracted).unwrap();
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn descriptor_archive_extracts_exact_source_bytes() {
        let root = tempdir();
        let archive = root.join("descriptor.zip");
        let mut bytes = deflate_zip();
        let central = bytes
            .windows(4)
            .position(|window| window == 0x02014b50_u32.to_le_bytes())
            .unwrap();
        let crc = bytes[14..18].to_vec();
        let compressed = bytes[18..22].to_vec();
        let expanded = bytes[22..26].to_vec();
        let mut descriptor = 0x08074b50_u32.to_le_bytes().to_vec();
        descriptor.extend_from_slice(&crc);
        descriptor.extend_from_slice(&compressed);
        descriptor.extend_from_slice(&expanded);
        bytes[6] |= 8;
        bytes[14..26].fill(0);
        bytes.splice(central..central, descriptor);
        bytes[central + 16 + 8] |= 8;
        let end = bytes
            .windows(4)
            .position(|window| window == 0x06054b50_u32.to_le_bytes())
            .unwrap();
        bytes[end + 16..end + 20].copy_from_slice(&((central + 16) as u32).to_le_bytes());
        std::fs::write(&archive, bytes).unwrap();
        let extracted = extract_zip(&archive).unwrap();
        assert_eq!(
            std::fs::read(extracted.join("src/中文.py")).unwrap(),
            b"print('offline')\n"
        );
        std::fs::remove_dir_all(extracted).unwrap();
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn corrupt_compressed_content_is_rejected() {
        let root = tempdir();
        let archive = root.join("corrupt.zip");
        let mut bytes = deflate_zip();
        let name_len = u16::from_le_bytes(bytes[26..28].try_into().unwrap()) as usize;
        let extra_len = u16::from_le_bytes(bytes[28..30].try_into().unwrap()) as usize;
        bytes[30 + name_len + extra_len] ^= 0xff;
        std::fs::write(&archive, bytes).unwrap();
        assert!(extract_zip(&archive).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
    fn tempdir() -> std::path::PathBuf {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        tempdir_at(timestamp)
    }

    fn tempdir_at(timestamp: u128) -> std::path::PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT_ID: AtomicU64 = AtomicU64::new(0);

        loop {
            let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "codedoctor-zip-test-{}-{timestamp}-{id}",
                std::process::id(),
            ));
            // Reserve a fresh directory without deleting another test's files.
            match std::fs::create_dir(&path) {
                Ok(()) => return path,
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("could not create ZIP test directory: {error}"),
            }
        }
    }

    #[test]
    fn test_tempdirs_with_same_timestamp_are_isolated() {
        let first = tempdir_at(0);
        let archive = first.join("crc.zip");
        let mut bytes = stored_zip(&[("data.txt", b"correct")]);
        bytes[30 + "data.txt".len()] ^= 1;
        std::fs::write(&archive, bytes).unwrap();

        let second = tempdir_at(0);
        let result = extract_zip(&archive);
        let isolated = first != second && archive.is_file();

        std::fs::remove_dir_all(&first).unwrap();
        if first != second {
            std::fs::remove_dir_all(second).unwrap();
        }
        assert!(
            isolated && matches!(&result, Err(AppError::Zip(_))),
            "same-timestamp test directories must preserve the first archive; got {result:?}"
        );
    }

    fn stored_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
        zip_fixture(entries, false, false)
    }

    fn crc32(data: &[u8]) -> u32 {
        let mut crc = !0_u32;
        for byte in data {
            crc ^= u32::from(*byte);
            for _ in 0..8 {
                crc = (crc >> 1) ^ (0xedb88320 & 0_u32.wrapping_sub(crc & 1));
            }
        }
        !crc
    }

    // A final uncompressed DEFLATE block is a valid method-8 stream.
    fn deflate_block(data: &[u8]) -> Vec<u8> {
        let len = u16::try_from(data.len()).unwrap();
        let mut compressed = vec![1];
        compressed.extend_from_slice(&len.to_le_bytes());
        compressed.extend_from_slice(&(!len).to_le_bytes());
        compressed.extend_from_slice(data);
        compressed
    }

    fn zip_fixture(entries: &[(&str, &[u8])], deflated: bool, descriptor: bool) -> Vec<u8> {
        let mut bytes = Vec::new();
        let mut central = Vec::new();
        for (name, data) in entries {
            let local_offset = bytes.len() as u32;
            let compressed = if deflated {
                deflate_block(data)
            } else {
                data.to_vec()
            };
            let flags = if descriptor { 8_u16 } else { 0 };
            let method = if deflated { 8_u16 } else { 0 };
            let checksum = crc32(data);
            bytes.extend_from_slice(&0x04034b50_u32.to_le_bytes());
            bytes.extend_from_slice(&20_u16.to_le_bytes());
            bytes.extend_from_slice(&flags.to_le_bytes());
            bytes.extend_from_slice(&method.to_le_bytes());
            bytes.extend_from_slice(&[0; 4]);
            bytes.extend_from_slice(&(if descriptor { 0 } else { checksum }).to_le_bytes());
            bytes.extend_from_slice(
                &(if descriptor {
                    0
                } else {
                    compressed.len() as u32
                })
                .to_le_bytes(),
            );
            bytes
                .extend_from_slice(&(if descriptor { 0 } else { data.len() as u32 }).to_le_bytes());
            bytes.extend_from_slice(&(name.len() as u16).to_le_bytes());
            bytes.extend_from_slice(&0_u16.to_le_bytes());
            bytes.extend_from_slice(name.as_bytes());
            bytes.extend_from_slice(&compressed);
            if descriptor {
                bytes.extend_from_slice(&0x08074b50_u32.to_le_bytes());
                bytes.extend_from_slice(&checksum.to_le_bytes());
                bytes.extend_from_slice(&(compressed.len() as u32).to_le_bytes());
                bytes.extend_from_slice(&(data.len() as u32).to_le_bytes());
            }
            central.extend_from_slice(&0x02014b50_u32.to_le_bytes());
            central.extend_from_slice(&20_u16.to_le_bytes());
            central.extend_from_slice(&20_u16.to_le_bytes());
            central.extend_from_slice(&flags.to_le_bytes());
            central.extend_from_slice(&method.to_le_bytes());
            central.extend_from_slice(&[0; 4]);
            central.extend_from_slice(&checksum.to_le_bytes());
            central.extend_from_slice(&(compressed.len() as u32).to_le_bytes());
            central.extend_from_slice(&(data.len() as u32).to_le_bytes());
            central.extend_from_slice(&(name.len() as u16).to_le_bytes());
            central.extend_from_slice(&0_u16.to_le_bytes());
            central.extend_from_slice(&0_u16.to_le_bytes());
            central.extend_from_slice(&[0; 8]);
            central.extend_from_slice(&local_offset.to_le_bytes());
            central.extend_from_slice(name.as_bytes());
        }
        let central_offset = bytes.len() as u32;
        bytes.extend_from_slice(&central);
        bytes.extend_from_slice(&0x06054b50_u32.to_le_bytes());
        bytes.extend_from_slice(&[0; 4]);
        bytes.extend_from_slice(&(entries.len() as u16).to_le_bytes());
        bytes.extend_from_slice(&(entries.len() as u16).to_le_bytes());
        bytes.extend_from_slice(&(central.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&central_offset.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes
    }

    #[test]
    fn deflate_entry_extracts_original_bytes() {
        let root = tempdir();
        let archive = root.join("deflate.zip");
        fs::write(
            &archive,
            zip_fixture(&[("src/main.py", b"print('hello')\n")], true, false),
        )
        .unwrap();
        let extracted = extract_zip(&archive).unwrap();
        assert_eq!(
            fs::read(extracted.join("src/main.py")).unwrap(),
            b"print('hello')\n"
        );
        fs::remove_dir_all(extracted).unwrap();
    }

    #[test]
    fn deflate_entry_with_data_descriptor_extracts_original_bytes() {
        let root = tempdir();
        let archive = root.join("descriptor.zip");
        fs::write(
            &archive,
            zip_fixture(&[("hello.txt", b"streamed ZIP content\n")], true, true),
        )
        .unwrap();
        let extracted = extract_zip(&archive).unwrap();
        assert_eq!(
            fs::read(extracted.join("hello.txt")).unwrap(),
            b"streamed ZIP content\n"
        );
        fs::remove_dir_all(extracted).unwrap();
    }

    #[test]
    fn corrupt_stored_entry_is_rejected() {
        let root = tempdir();
        let archive = root.join("crc.zip");
        let mut bytes = stored_zip(&[("data.txt", b"correct")]);
        bytes[30 + "data.txt".len()] ^= 1;
        fs::write(&archive, bytes).unwrap();
        let result = extract_zip(&archive);
        fs::remove_dir_all(root).unwrap();
        assert!(
            matches!(result, Err(AppError::Zip(_))),
            "expected corrupt stored content to be rejected as a ZIP error, got {result:?}"
        );
    }

    #[test]
    fn windows_reserved_device_components_are_rejected_before_extraction() {
        let reserved = [
            "CON",
            "PRN.md",
            "aux.txt",
            "NUL .log",
            "COM1.bin",
            "LPT9",
            "COM¹.dat",
            "LPT².csv",
            "COM³",
            "conin$",
            "CONOUT$.txt",
        ];

        for name in reserved {
            let root = tempdir();
            let archive = root.join("reserved.zip");
            fs::write(&archive, stored_zip(&[(name, b"must not reach a device")])).unwrap();
            let result = extract_zip(&archive);
            let rejected_as_zip = matches!(&result, Err(AppError::Zip(_)));
            if let Ok(extracted) = &result {
                let _ = fs::remove_dir_all(extracted);
            }
            fs::remove_dir_all(root).unwrap();
            assert!(
                rejected_as_zip,
                "expected reserved device path {name:?} to be rejected as a ZIP path, got {result:?}"
            );
        }
    }

    #[test]
    fn ordinary_names_resembling_reserved_devices_are_allowed() {
        let root = tempdir();
        let archive = root.join("ordinary.zip");
        fs::write(
            &archive,
            stored_zip(&[("console.py", b"console"), ("company.ts", b"company")]),
        )
        .unwrap();
        let extracted = extract_zip(&archive).unwrap();
        assert_eq!(fs::read(extracted.join("console.py")).unwrap(), b"console");
        assert_eq!(fs::read(extracted.join("company.ts")).unwrap(), b"company");
        fs::remove_dir_all(extracted).unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn windows_device_component_filter_covers_port_aliases_and_normal_names() {
        for name in [
            "CON",
            "con.txt",
            "AUX .md",
            "COM9",
            "LPT1.csv",
            "COM¹",
            "LPT².log",
            "COM³",
            "LPT⁴",
            "COM⁵",
            "LPT⁶",
            "COM⁷",
            "LPT⁸",
            "COM⁹",
            "CLOCK$",
            "CONFIG$.sys",
            "CONIN$",
            "CONOUT$",
        ] {
            assert!(
                unsafe_windows_component(name),
                "expected {name:?} to be unsafe"
            );
        }
        for name in [
            "console.py",
            "company.ts",
            "comet.rs",
            "lpt0.txt",
            "nulled.txt",
        ] {
            assert!(
                !unsafe_windows_component(name),
                "expected ordinary name {name:?} to be allowed"
            );
        }
    }

    #[test]
    fn failed_extraction_removes_temporary_directory() {
        for deflated in [false, true] {
            for descriptor in [false, true] {
                let root = tempdir();
                let archive = root.join("corrupt-second-entry.zip");
                let mut bytes = zip_fixture(
                    &[("first.txt", b"first"), ("second.txt", b"second")],
                    deflated,
                    descriptor,
                );
                // Both supported methods must pass CRC validation, including when
                // sizes and checksums are supplied through a data descriptor.
                let second_header = 30
                    + "first.txt".len()
                    + b"first".len()
                    + if deflated { 5 } else { 0 }
                    + if descriptor { 16 } else { 0 };
                let second_content = second_header + 30 + "second.txt".len();
                bytes[second_content + if deflated { 5 } else { 0 }] ^= 1;
                std::fs::write(&archive, bytes).unwrap();

                let extraction_base = root.join("extraction-root");
                std::fs::create_dir_all(&extraction_base).unwrap();
                let result =
                    extract_zip_with_root(&archive, || unique_temp_dir_in(&extraction_base));
                let remaining = std::fs::read_dir(&extraction_base).unwrap().count();
                std::fs::remove_dir_all(root).unwrap();
                assert!(
                    matches!(result, Err(AppError::Zip(_))),
                    "expected a ZIP error for deflated={deflated}, descriptor={descriptor}; got {result:?}"
                );
                assert_eq!(
                    remaining, 0,
                    "failed extraction left files for deflated={deflated}, descriptor={descriptor}"
                );
            }
        }
    }

    #[test]
    fn local_and_central_headers_must_agree() {
        let root = tempdir();
        let archive = root.join("mismatch.zip");
        let mut bytes = stored_zip(&[("same.txt", b"a")]);
        let central = bytes
            .windows(4)
            .position(|window| window == 0x02014b50_u32.to_le_bytes())
            .unwrap();
        bytes[central + 20..central + 24].copy_from_slice(&99_u32.to_le_bytes());
        std::fs::write(&archive, bytes).unwrap();
        assert!(extract_zip(&archive).is_err());
    }

    #[test]
    fn raw_absolute_and_traversal_entry_names_are_rejected() {
        for name in [
            "/root.txt",
            r#"\root.txt"#,
            r#"\\server\share\file.txt"#,
            "C:/drive-absolute.txt",
            r#"C:\drive-absolute.txt"#,
            "C:drive-relative.txt",
            "../escape.txt",
            r#"..\escape.txt"#,
            "nested/../../escape.txt",
            "nested/../escape.txt",
            r#"nested\..\escape.txt"#,
        ] {
            let root = tempdir();
            let archive = root.join("unsafe.zip");
            std::fs::write(&archive, stored_zip(&[(name, b"must be rejected")])).unwrap();
            let result = extract_zip(&archive);
            if let Ok(extracted) = &result {
                let _ = std::fs::remove_dir_all(extracted);
            }
            std::fs::remove_dir_all(&root).unwrap();
            assert!(
                matches!(result, Err(AppError::Zip(_))),
                "expected raw archive path {name:?} to be rejected, got {result:?}"
            );
        }
    }

    #[test]
    fn nested_relative_entry_is_still_extracted() {
        let root = tempdir();
        let archive = root.join("nested.zip");
        std::fs::write(
            &archive,
            stored_zip(&[("src/nested/main.rs", b"fn main() {}")]),
        )
        .unwrap();
        let extracted = extract_zip(&archive).unwrap();
        assert_eq!(
            std::fs::read(extracted.join("src/nested/main.rs")).unwrap(),
            b"fn main() {}"
        );
        std::fs::remove_dir_all(extracted).unwrap();
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn zip_symlink_entries_are_rejected() {
        let root = tempdir();
        let archive = root.join("symlink.zip");
        let mut bytes = stored_zip(&[("linked.txt", b"target")]);
        let central = bytes
            .windows(4)
            .position(|window| window == 0x02014b50_u32.to_le_bytes())
            .unwrap();
        bytes[central + 5] = 3;
        bytes[central + 38..central + 42].copy_from_slice(&(0o120777_u32 << 16).to_le_bytes());
        std::fs::write(&archive, bytes).unwrap();
        assert!(matches!(
            extract_zip(&archive),
            Err(AppError::Zip(message)) if message == "archive links and special files are unsupported"
        ));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn zip_limits_are_checked_before_writing() {
        let root = tempdir();
        let archive = root.join("many.zip");
        let names: Vec<(String, Vec<u8>)> = (0..(MAX_ZIP_ENTRIES + 1))
            .map(|i| (format!("{i}.txt"), vec![b'x']))
            .collect();
        let refs: Vec<(&str, &[u8])> = names
            .iter()
            .map(|(name, data)| (name.as_str(), data.as_slice()))
            .collect();
        std::fs::write(&archive, stored_zip(&refs)).unwrap();
        assert!(extract_zip(&archive).is_err());
    }
}
use crate::error::AppError;
use ::zip::{CompressionMethod, ZipArchive};
use std::collections::HashSet;
use std::fs::{self, File, OpenOptions};
use std::io::{Cursor, Read, Write};
use std::path::{Path, PathBuf};

pub const MAX_ZIP_ENTRIES: usize = 10_000;
pub const MAX_ZIP_COMPRESSED_BYTES: u64 = 100 * 1024 * 1024;
pub const MAX_ZIP_UNCOMPRESSED_BYTES: u64 = 512 * 1024 * 1024;
pub const MAX_ZIP_FILE_BYTES: u64 = 25 * 1024 * 1024;

pub fn extract_zip(path: impl AsRef<Path>) -> Result<PathBuf, AppError> {
    extract_zip_with_root(path, unique_temp_dir)
}

fn extract_zip_with_root(
    path: impl AsRef<Path>,
    allocate_root: impl FnOnce() -> Result<PathBuf, AppError>,
) -> Result<PathBuf, AppError> {
    extract_zip_checked(path, allocate_root, || Ok(()))
}

pub(crate) fn extract_zip_checked(
    path: impl AsRef<Path>,
    allocate_root: impl FnOnce() -> Result<PathBuf, AppError>,
    check: impl Fn() -> Result<(), AppError>,
) -> Result<PathBuf, AppError> {
    check()?;
    let (mut archive, entries) = validate_archive(File::open(path)?, &check)?;
    let temp_root = allocate_root()?;
    let extraction = (|| -> Result<(), AppError> {
        let mut actual_total = 0u64;
        for (index, member) in entries.iter().enumerate() {
            check()?;
            let destination = temp_root.join(&member.relative_path);
            if !destination.starts_with(&temp_root) {
                return Err(zip_error("archive entry escapes extraction root"));
            }
            let content = decode_entry(&mut archive, index, member.size)?;
            actual_total += content.len() as u64;
            if actual_total > MAX_ZIP_UNCOMPRESSED_BYTES {
                return Err(zip_error("archive expands beyond size limit"));
            }
            if member.is_dir {
                if !content.is_empty() {
                    return Err(zip_error("archive directory contains data"));
                }
                fs::create_dir_all(&destination)?;
                continue;
            }
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut output = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&destination)?;
            output.write_all(&content)?;
            output.flush()?;
        }
        Ok(())
    })();
    if let Err(error) = extraction {
        let _ = fs::remove_dir_all(&temp_root);
        return Err(error);
    }
    Ok(temp_root)
}

type Archive = ZipArchive<Cursor<Vec<u8>>>;

struct ArchiveMember {
    relative_path: String,
    is_dir: bool,
    size: u64,
}

/// Read one scan member in memory after applying the same archive preflight as import.
#[cfg(any(feature = "desktop", test))]
pub(crate) fn read_zip_member(
    file: File,
    relative_path: &str,
    expected_size: u64,
) -> Result<Option<Vec<u8>>, AppError> {
    let (mut archive, entries) = validate_archive(file, &|| Ok(()))?;
    let Some((index, member)) = entries
        .iter()
        .enumerate()
        .find(|(_, member)| member.relative_path == relative_path && !member.is_dir)
    else {
        return Ok(None);
    };
    if member.size != expected_size {
        return Ok(None);
    }
    decode_entry(&mut archive, index, member.size).map(Some)
}

fn validate_archive(
    file: File,
    check: &impl Fn() -> Result<(), AppError>,
) -> Result<(Archive, Vec<ArchiveMember>), AppError> {
    if file.metadata()?.len() > MAX_ZIP_COMPRESSED_BYTES {
        return Err(zip_error("compressed archive exceeds size limit"));
    }
    // Snapshot at a hard bound, including when the archive grows during reading.
    let mut bytes = Vec::new();
    file.take(MAX_ZIP_COMPRESSED_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_ZIP_COMPRESSED_BYTES {
        return Err(zip_error("compressed archive exceeds size limit"));
    }
    let mut archive = ZipArchive::new(Cursor::new(bytes))
        .map_err(|_| zip_error("archive directory is invalid"))?;
    if archive.is_empty() || archive.len() > MAX_ZIP_ENTRIES {
        return Err(zip_error("archive entry count is outside limits"));
    }
    archive
        .validate_header_consistency()
        .map_err(|_| zip_error("archive headers are inconsistent"))?;

    // Validate all names, metadata, and cumulative declared sizes before writing.
    let mut entries = Vec::with_capacity(archive.len());
    let mut names = HashSet::new();
    let mut total = 0u64;
    for index in 0..archive.len() {
        check()?;
        let entry = archive
            .by_index_data(index)
            .map_err(|_| zip_error("archive entry is invalid"))?;
        if entry.encrypted() {
            return Err(zip_error("encrypted archive entries are unsupported"));
        }
        if entry.is_symlink()
            || entry.unix_mode().is_some_and(|mode| {
                let kind = mode & 0o170000;
                kind != 0 && kind != 0o100000 && kind != 0o040000
            })
        {
            return Err(zip_error("archive links and special files are unsupported"));
        }
        if !matches!(
            entry.compression(),
            CompressionMethod::Stored | CompressionMethod::Deflated
        ) {
            return Err(zip_error("archive compression method is unsupported"));
        }
        let raw_name = entry
            .name()
            .map_err(|_| zip_error("archive entry name cannot be decoded"))?;
        if raw_name.is_empty()
            || raw_name.starts_with(['/', '\\'])
            || raw_name.contains(':')
            || raw_name.split(['/', '\\']).any(|part| part == "..")
        {
            return Err(zip_error("archive entry path is invalid"));
        }
        let safe_name = entry
            .enclosed_name()
            .ok_or_else(|| zip_error("archive entry path is invalid"))?;
        let relative = crate::import::path::normalize_relative_path(&safe_name)
            .ok_or_else(|| zip_error("archive entry path is invalid"))?;
        // Windows aliases and alternate streams must never target another entry.
        if relative.split('/').any(unsafe_windows_component)
            || !names.insert(relative.to_lowercase())
        {
            return Err(zip_error("archive entry path is invalid or duplicated"));
        }
        if entry.size() > MAX_ZIP_FILE_BYTES {
            return Err(zip_error("archive file exceeds size limit"));
        }
        total = total
            .checked_add(entry.size())
            .ok_or_else(|| zip_error("archive expands beyond size limit"))?;
        if total > MAX_ZIP_UNCOMPRESSED_BYTES {
            return Err(zip_error("archive expands beyond size limit"));
        }
        entries.push(ArchiveMember {
            relative_path: relative,
            is_dir: entry.is_dir(),
            size: entry.size(),
        });
    }
    Ok((archive, entries))
}

fn decode_entry(archive: &mut Archive, index: usize, size: u64) -> Result<Vec<u8>, AppError> {
    let entry = archive
        .by_index(index)
        .map_err(|_| zip_error("archive entry cannot be decoded"))?;
    let mut content = Vec::new();
    // Read one extra byte to distinguish an exact limit from overflow.
    entry
        .take(size + 1)
        .read_to_end(&mut content)
        .map_err(|_| zip_error("archive content is corrupt or truncated"))?;
    if content.len() as u64 != size {
        return Err(zip_error("archive content size is invalid"));
    }
    Ok(content)
}

fn unsafe_windows_component(component: &str) -> bool {
    if component.contains(':') || component.ends_with(['.', ' ']) || component.contains('\0') {
        return true;
    }

    // Win32 device names remain reserved with an extension. Spaces before the
    // extension and the documented superscript port aliases also resolve to devices.
    let basename = component
        .split('.')
        .next()
        .unwrap_or_default()
        .trim_end_matches(['.', ' '])
        .to_ascii_uppercase();
    if matches!(
        basename.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$" | "CLOCK$" | "CONFIG$"
    ) {
        return true;
    }
    basename
        .strip_prefix("COM")
        .or_else(|| basename.strip_prefix("LPT"))
        .is_some_and(|suffix| {
            matches!(
                suffix,
                "1" | "2"
                    | "3"
                    | "4"
                    | "5"
                    | "6"
                    | "7"
                    | "8"
                    | "9"
                    | "¹"
                    | "²"
                    | "³"
                    | "⁴"
                    | "⁵"
                    | "⁶"
                    | "⁷"
                    | "⁸"
                    | "⁹"
            )
        })
}

fn zip_error(message: &str) -> AppError {
    AppError::Zip(message.to_owned())
}

pub(crate) fn unique_temp_dir() -> Result<PathBuf, AppError> {
    unique_temp_dir_in(&std::env::temp_dir().join("codedoctor-imports"))
}

fn unique_temp_dir_in(base: &Path) -> Result<PathBuf, AppError> {
    fs::create_dir_all(&base)?;
    for attempt in 0..100_u32 {
        let candidate = base.join(format!("{}-{}", std::process::id(), attempt));
        match fs::create_dir(&candidate) {
            Ok(()) => return Ok(candidate),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        }
    }
    Err(AppError::Zip(
        "could not allocate temporary extraction directory".to_owned(),
    ))
}
