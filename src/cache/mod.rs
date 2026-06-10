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
        Ok(())
    }

    pub fn load_raw_cache(&self, path: &Path) -> Option<(Vec<u8>, i32, i32)> {
        let mut file = fs::File::open(path).ok()?;
        let mut width_buf = [0u8; 4];
        let mut height_buf = [0u8; 4];
        file.read_exact(&mut width_buf).ok()?;
        file.read_exact(&mut height_buf).ok()?;

        let width = i32::from_ne_bytes(width_buf);
        let height = i32::from_ne_bytes(height_buf);

        let expected_size = (width as u64) * (height as u64) * 4;
        if width <= 0 || height <= 0 || expected_size > 100 * 1024 * 1024 {
            return None;
        }

        let mut raw_data = Vec::with_capacity(expected_size as usize);
        file.read_to_end(&mut raw_data).ok()?;

        if raw_data.len() == expected_size as usize {
            Some((raw_data, width, height))
        } else {
            None
        }
    }
}
