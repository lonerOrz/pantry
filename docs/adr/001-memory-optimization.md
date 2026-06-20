# ADR-001: Memory Optimization Strategy

## Status

Accepted

## Context

pantry is a GTK4-based selector application. Three memory concerns were identified during architecture review:

1. **Preview image decode peak** — GIF path uses `image` crate which decodes the full GIF into memory before resizing. For large GIFs (e.g., 4K), peak memory can reach 100+ MB temporarily.
2. **Unbounded Item growth** — Dynamic sources (e.g., `cliphist list`) and directory traversal produce unbounded `Vec<Item>` with no cap.
3. **Unbounded disk cache** — `CacheManager` has no eviction, TTL, or max size. `~/.cache/pantry/` grows forever.

## Decision

### 1. Preview Image Decode: Header Guard

Add a pre-decode size check before decoding images. Read the image header to get dimensions, and reject images where `width × height × 4 > 50 MB`. This prevents the `image` crate from allocating massive intermediate buffers for oversized images.

- **Threshold**: 50 MB decoded RGBA (approximately 3500×3500 pixels)
- **Behavior**: When rejected, return `PreviewPayload::Text(value)` fallback
- **Scope**: Applies to both `load_gif_first_frame()` and `load_image_data_raw()` in `preview.rs`

### 2. Item Loading: Hard Cap

Add a `MAX_ITEMS = 10000` constant. All item sources (config, command, dynamic, directory traversal) are capped at this limit. When the limit is reached, stop loading and log a warning.

- **Constant**: `MAX_ITEMS = 10000` in `constants.rs`
- **Behavior**: Truncate item collection at limit, log warning
- **Scope**: `loader.rs`, `expansion.rs`, `item_service.rs`

### 3. Disk Cache: Max Size + LRU Eviction

Add startup eviction to `CacheManager`. On initialization, check total cache directory size. If it exceeds 1 GB, delete the oldest files (by mtime) until under the limit.

- **Max size**: 1 GB (`CACHE_MAX_SIZE_MB = 1024` in `constants.rs`)
- **Eviction**: Sorted by mtime ascending, delete until total ≤ max
- **Trigger**: `CacheManager::new()` or explicit `evict_if_needed()` call

### 4. Dynamic Source: 10 MB Output Cap

In `expansion.rs`, cap the command output reading at 10 MB. This prevents a runaway command from allocating a massive String before Item parsing begins.

- **Limit**: `DYNAMIC_OUTPUT_MAX_BYTES = 10 * 1024 * 1024` in `constants.rs`
- **Behavior**: Stop reading stdout when limit reached, log warning
- **Scope**: `expansion.rs` dynamic source path

## Consequences

### Positive
- GIF decode peak reduced from 100+ MB to ≤50 MB
- Item count bounded at 10,000 (worst case ~2.5 MB heap)
- Disk cache bounded at 1 GB
- Dynamic source output bounded at 10 MB
- All changes are backward-compatible, no API changes

### Negative
- Very large GIFs (>3500×3500) won't get image previews (text fallback only)
- Config-heavy users with >10,000 entries will be silently truncated
- Cache eviction at startup adds ~50-100ms for large cache directories

### Risks
- Header parsing may fail for corrupted images → fallback to current behavior (decode anyway)
- Mtime-based eviction may delete frequently-used cache files → acceptable for a selector tool

## Alternatives Considered

- **Streaming decode** for GIFs: More complex, requires changing `image` crate usage. Deferred to future work.
- **Lazy loading / virtual scrolling**: Would require rewriting Gio ListStore factory pattern. Too large a change for current scope.
- **Per-source Item caps**: More granular but adds complexity. Hard cap is simpler and sufficient.
- **TTL-based cache expiration**: Less predictable than size-based eviction. Users may have burst usage patterns.
- **CLI-only cache cleanup (`--clear-cache`)**: Pushes responsibility to user. Automatic eviction is better UX.
