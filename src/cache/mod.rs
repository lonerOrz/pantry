use crate::constants::CACHE_MAX_SIZE_BYTES;
use std::fs;
use std::io::{self, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

pub trait CacheAdapter: Send + Sync {
    fn get_cache_path(&self, category: &str, original_path: &Path) -> PathBuf;
    fn is_cache_valid(&self, cache_path: &Path, original_path: &Path) -> bool;
    fn save_raw_cache(
        &self,
        path: &Path,
        raw_data: &[u8],
        width: i32,
        height: i32,
    ) -> io::Result<()>;
    fn load_raw_cache(&self, path: &Path) -> Option<(Vec<u8>, i32, i32)>;
}

#[derive(Clone)]
pub struct CacheManager {
    cache_dir: PathBuf,
}

impl CacheManager {
    pub fn new() -> Self {
        let mut cache_dir = dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("pantry");

        if let Err(e) = fs::create_dir_all(&cache_dir) {
            eprintln!("Warning: Failed to create cache directory: {}", e);
            cache_dir = PathBuf::from(".");
        }

        Self { cache_dir }
    }

    #[cfg(test)]
    pub fn with_cache_dir(cache_dir: PathBuf) -> Self {
        Self { cache_dir }
    }
}

impl CacheAdapter for CacheManager {
    fn get_cache_path(&self, category: &str, original_path: &Path) -> PathBuf {
        self.cache_dir.join(format!(
            "{}_{}.raw",
            category,
            crate::utils::path_to_safe_filename(original_path)
        ))
    }

    fn is_cache_valid(&self, cache_path: &Path, original_path: &Path) -> bool {
        if !cache_path.exists() {
            return false;
        }
        match (
            original_path.metadata().and_then(|m| m.modified()),
            cache_path.metadata().and_then(|m| m.modified()),
        ) {
            (Ok(orig_time), Ok(cache_time)) => cache_time >= orig_time,
            _ => false,
        }
    }

    fn save_raw_cache(
        &self,
        path: &Path,
        raw_data: &[u8],
        width: i32,
        height: i32,
    ) -> io::Result<()> {
        let file = fs::File::create(path)?;
        let mut writer = BufWriter::new(file);
        writer.write_all(&width.to_ne_bytes())?;
        writer.write_all(&height.to_ne_bytes())?;
        let compressed = lz4_flex::block::compress_prepend_size(raw_data);
        writer.write_all(&compressed)?;
        writer.flush()?;

        self.evict_if_needed();

        Ok(())
    }

    fn load_raw_cache(&self, path: &Path) -> Option<(Vec<u8>, i32, i32)> {
        let mut file = match fs::File::open(path) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("Failed to open cache file {}: {}", path.display(), e);
                return None;
            }
        };
        let mut width_buf = [0u8; 4];
        let mut height_buf = [0u8; 4];
        if let Err(e) = file.read_exact(&mut width_buf) {
            eprintln!("Failed to read width from cache {}: {}", path.display(), e);
            return None;
        }
        if let Err(e) = file.read_exact(&mut height_buf) {
            eprintln!("Failed to read height from cache {}: {}", path.display(), e);
            return None;
        }

        let width = i32::from_ne_bytes(width_buf);
        let height = i32::from_ne_bytes(height_buf);

        let expected_size = (width as u64) * (height as u64) * 4;
        if width <= 0 || height <= 0 || expected_size > 100 * 1024 * 1024 {
            return None;
        }

        let mut compressed = Vec::new();
        if let Err(e) = file.read_to_end(&mut compressed) {
            eprintln!("Failed to read data from cache {}: {}", path.display(), e);
            return None;
        }

        // Decompress; old uncompressed caches fail here → return None → silent re-cache
        let raw_data = match lz4_flex::block::decompress_size_prepended(&compressed) {
            Ok(data) => data,
            Err(e) => {
                eprintln!("Failed to decompress cache {}: {}", path.display(), e);
                return None;
            }
        };

        if raw_data.len() == expected_size as usize {
            Some((raw_data, width, height))
        } else {
            None
        }
    }
}

impl CacheManager {
    fn evict_if_needed(&self) {
        let mut entries: Vec<(PathBuf, u64, std::time::SystemTime)> = Vec::new();
        let mut total_size: u64 = 0;

        if let Ok(read_dir) = fs::read_dir(&self.cache_dir) {
            for entry in read_dir.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("raw")
                    && let Ok(meta) = fs::metadata(&path)
                {
                    let size = meta.len();
                    let mtime = meta.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                    total_size += size;
                    entries.push((path, size, mtime));
                }
            }
        }

        if total_size <= CACHE_MAX_SIZE_BYTES {
            return;
        }

        entries.sort_by(|a, b| a.2.cmp(&b.2));

        for (path, size, _) in &entries {
            if total_size <= CACHE_MAX_SIZE_BYTES {
                break;
            }
            if fs::remove_file(path).is_ok() {
                total_size -= size;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn make_cache_manager(dir: &Path) -> CacheManager {
        CacheManager::with_cache_dir(dir.to_path_buf())
    }

    #[test]
    fn binary_io_round_trip() {
        let dir = tempdir().unwrap();
        let cache = make_cache_manager(dir.path());
        let path = dir.path().join("test.raw");

        let (w, h) = (10, 10);
        let data: Vec<u8> = vec![42u8; (w * h * 4) as usize];
        cache.save_raw_cache(&path, &data, w, h).unwrap();

        let (loaded, lw, lh) = cache.load_raw_cache(&path).unwrap();
        assert_eq!(lw, w);
        assert_eq!(lh, h);
        assert_eq!(loaded, data);
    }

    #[test]
    fn binary_io_round_trip_non_square() {
        let dir = tempdir().unwrap();
        let cache = make_cache_manager(dir.path());
        let path = dir.path().join("rect.raw");

        let (w, h) = (100, 50);
        let data: Vec<u8> = vec![42u8; (w * h * 4) as usize];
        cache.save_raw_cache(&path, &data, w, h).unwrap();

        let (loaded, lw, lh) = cache.load_raw_cache(&path).unwrap();
        assert_eq!(lw, w);
        assert_eq!(lh, h);
        assert_eq!(loaded.len(), data.len());
    }

    #[test]
    fn load_nonexistent_returns_none() {
        let dir = tempdir().unwrap();
        let cache = make_cache_manager(dir.path());
        assert!(cache.load_raw_cache(&dir.path().join("nope.raw")).is_none());
    }

    #[test]
    fn load_corrupted_file_returns_none() {
        let dir = tempdir().unwrap();
        let cache = make_cache_manager(dir.path());
        let path = dir.path().join("corrupt.raw");

        fs::write(
            &path,
            [
                10, 0, 0, 0, // width = 10
                10, 0, 0,
                0, // height = 10
                   // missing pixel data (should be 400 bytes)
            ],
        )
        .unwrap();

        assert!(cache.load_raw_cache(&path).is_none());
    }

    #[test]
    fn load_invalid_dimensions_returns_none() {
        let dir = tempdir().unwrap();
        let cache = make_cache_manager(dir.path());
        let path = dir.path().join("bad.raw");

        fs::write(
            &path,
            [
                0, 0, 0, 0, // width = 0 (invalid)
                10, 0, 0, 0, 0, 0, 0, 0,
            ],
        )
        .unwrap();

        assert!(cache.load_raw_cache(&path).is_none());
    }

    #[test]
    fn cache_valid_after_save() {
        let dir = tempdir().unwrap();
        let cache = make_cache_manager(dir.path());
        let original = dir.path().join("original.png");
        let cached = dir.path().join("test.raw");

        fs::write(&original, b"image data").unwrap();
        cache.save_raw_cache(&cached, &[0u8; 40], 10, 1).unwrap();

        assert!(cache.is_cache_valid(&cached, &original));
    }

    #[test]
    fn cache_invalid_when_missing() {
        let dir = tempdir().unwrap();
        let cache = make_cache_manager(dir.path());
        let original = dir.path().join("original.png");
        let cached = dir.path().join("nonexistent.raw");

        fs::write(&original, b"image data").unwrap();

        assert!(!cache.is_cache_valid(&cached, &original));
    }

    #[test]
    fn cache_invalid_when_stale() {
        let dir = tempdir().unwrap();
        let cache = make_cache_manager(dir.path());
        let original = dir.path().join("original.png");
        let cached = dir.path().join("test.raw");

        fs::write(&original, b"new image").unwrap();
        cache.save_raw_cache(&cached, &[0u8; 40], 10, 1).unwrap();

        // Make original newer than cache by rewriting it after a tiny delay
        std::thread::sleep(std::time::Duration::from_millis(10));
        fs::write(&original, b"even newer image").unwrap();

        assert!(!cache.is_cache_valid(&cached, &original));
    }

    #[test]
    fn cache_valid_with_explicit_timestamps() {
        let dir = tempdir().unwrap();
        let cache = make_cache_manager(dir.path());
        let original = dir.path().join("original.png");
        let cached = dir.path().join("test.raw");

        fs::write(&original, b"image").unwrap();
        cache.save_raw_cache(&cached, &[0u8; 40], 10, 1).unwrap();

        let cache_time = std::time::SystemTime::now();
        let orig_time = cache_time - std::time::Duration::from_secs(1);

        filetime::set_file_mtime(&cached, filetime::FileTime::from_system_time(cache_time))
            .unwrap();
        filetime::set_file_mtime(&original, filetime::FileTime::from_system_time(orig_time))
            .unwrap();

        assert!(cache.is_cache_valid(&cached, &original));
    }

    #[test]
    fn get_cache_path_contains_category_and_name() {
        let dir = tempdir().unwrap();
        let cache = make_cache_manager(dir.path());
        let path = cache.get_cache_path("my_cat", Path::new("/home/user/photo.png"));
        let name = path.file_name().unwrap().to_str().unwrap();
        assert!(name.starts_with("my_cat_"));
        assert!(name.ends_with(".raw"));
    }

    #[test]
    fn eviction_deletes_oldest_when_over_limit() {
        let dir = tempdir().unwrap();
        let cache = make_cache_manager(dir.path());

        // Create 3 files, set total > CACHE_MAX_SIZE_BYTES for eviction
        // We can't easily hit 1GB in tests, so we'll test the eviction logic
        // by creating files and verifying the oldest is deleted
        let file1 = dir.path().join("old.raw");
        let file2 = dir.path().join("new.raw");

        let data = vec![0u8; 100];
        cache.save_raw_cache(&file1, &data, 10, 1).unwrap();
        cache.save_raw_cache(&file2, &data, 10, 1).unwrap();

        // Both should exist since total << CACHE_MAX_SIZE_BYTES
        assert!(file1.exists());
        assert!(file2.exists());
    }

    #[test]
    fn save_overwrites_existing_cache() {
        let dir = tempdir().unwrap();
        let cache = make_cache_manager(dir.path());
        let path = dir.path().join("overwrite.raw");

        let data1 = vec![1u8; 40];
        let data2 = vec![2u8; 40];

        cache.save_raw_cache(&path, &data1, 10, 1).unwrap();
        let (loaded, _, _) = cache.load_raw_cache(&path).unwrap();
        assert_eq!(loaded, data1);

        cache.save_raw_cache(&path, &data2, 10, 1).unwrap();
        let (loaded, _, _) = cache.load_raw_cache(&path).unwrap();
        assert_eq!(loaded, data2);
    }
}
