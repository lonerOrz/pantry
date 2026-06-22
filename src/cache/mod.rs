use crate::constants::CACHE_MAX_SIZE_BYTES;
use std::fs;
use std::io::{BufWriter, Read, Write};
use std::path::{Path, PathBuf};

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

    pub fn get_cache_path(&self, category: &str, original_path: &Path) -> PathBuf {
        self.cache_dir.join(format!(
            "{}_{}.raw",
            category,
            crate::utils::path_to_safe_filename(original_path)
        ))
    }

    pub fn is_cache_valid(&self, cache_path: &Path, original_path: &Path) -> bool {
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

    pub fn save_raw_cache(
        &self,
        path: &Path,
        raw_data: &[u8],
        width: i32,
        height: i32,
    ) -> std::io::Result<()> {
        let file = fs::File::create(path)?;
        let mut writer = BufWriter::new(file);
        writer.write_all(&width.to_ne_bytes())?;
        writer.write_all(&height.to_ne_bytes())?;
        writer.write_all(raw_data)?;
        writer.flush()?;

        self.evict_if_needed();

        Ok(())
    }

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

    pub fn load_raw_cache(&self, path: &Path) -> Option<(Vec<u8>, i32, i32)> {
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

        let mut raw_data = Vec::with_capacity(expected_size as usize);
        if let Err(e) = file.read_to_end(&mut raw_data) {
            eprintln!("Failed to read data from cache {}: {}", path.display(), e);
            return None;
        }

        if raw_data.len() == expected_size as usize {
            Some((raw_data, width, height))
        } else {
            None
        }
    }
}
