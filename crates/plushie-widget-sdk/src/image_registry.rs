//! In-memory image handle storage.
//!
//! The renderer owns this registry. Hosts create images by sending
//! encoded bytes (PNG, JPEG, etc.) or raw RGBA pixel data via
//! `image_op` messages, which the renderer applies through mutable
//! registry methods. Each image is stored as an iced [`image::Handle`]
//! keyed by a host-chosen name. Widget nodes reference images by name
//! through the `source` prop, and render code receives a shared
//! [`ImageRegistry`] reference to resolve them through
//! [`ImageRegistry::get`].

use std::sync::Mutex;

use iced::widget::image;
use lru::LruCache;

/// Maximum number of images the registry will hold.
const MAX_IMAGES: usize = 4096;

/// Maximum aggregate byte usage across all images (1 GiB).
const MAX_TOTAL_BYTES: usize = 1024 * 1024 * 1024;

/// Sniff the image format from the first few bytes (magic bytes).
/// Returns `None` if the format is not recognized.
fn sniff_image_format(data: &[u8]) -> Option<&'static str> {
    if data.len() < 4 {
        return None;
    }
    if data.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
        return Some("PNG");
    }
    if data.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Some("JPEG");
    }
    if data.starts_with(b"GIF8") {
        return Some("GIF");
    }
    if data.starts_with(b"RIFF") && data.len() >= 12 && &data[8..12] == b"WEBP" {
        return Some("WebP");
    }
    if data.starts_with(b"BM") {
        return Some("BMP");
    }
    None
}

/// In-memory registry for image handles. Allows the host to send raw pixel
/// or encoded image data and reference them by name in the UI tree.
///
/// Enforces per-image size limits and aggregate limits (count and total bytes)
/// to prevent unbounded memory growth from a misbehaving host.
///
/// This is a renderer-owned store, not a generally thread-safe shared
/// container. Mutation APIs require `&mut self`. Render-time lookups use
/// `&self`; those lookups update only the internal LRU list under a small
/// lock, which is not a guarantee that callers may mutate the registry
/// concurrently.
pub struct ImageRegistry {
    /// Name -> (handle, byte cost). The LRU ordering keeps eviction
    /// O(1): `pop_lru` pulls the least recently touched candidate
    /// without scanning, and `get_or_peek` reorders the candidate
    /// chain on access.
    entries: Mutex<LruCache<String, ImageEntry>>,
    #[cfg(test)]
    max_total_bytes: usize,
    /// Running total of all image bytes in the registry. Maintained
    /// alongside `entries` so we don't have to walk the LRU to read
    /// the aggregate size.
    total_bytes: usize,
}

#[derive(Clone)]
struct ImageEntry {
    handle: image::Handle,
    bytes: usize,
}

impl std::fmt::Debug for ImageRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let guard = self.entries_lock();
        let names: Vec<&String> = guard.iter().map(|(name, _)| name).collect();
        f.debug_struct("ImageRegistry")
            .field("count", &guard.len())
            .field("total_bytes", &self.total_bytes)
            .field("names", &names)
            .finish()
    }
}

impl Default for ImageRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ImageRegistry {
    /// Create an empty image registry.
    pub fn new() -> Self {
        // Use an unbounded LRU and run our own count + byte-budget
        // checks in `make_room_for`. Letting `LruCache` auto-evict
        // would lose the size-aware logic that drops the largest
        // candidate that is not currently being inserted.
        Self {
            entries: Mutex::new(LruCache::unbounded()),
            #[cfg(test)]
            max_total_bytes: MAX_TOTAL_BYTES,
            total_bytes: 0,
        }
    }

    /// Maximum dimension (width or height) for a single image.
    const MAX_DIMENSION: u32 = 16384;

    /// Maximum pixel data size in bytes (256 MB).
    const MAX_PIXEL_BYTES: usize = 256 * 1024 * 1024;

    #[cfg(test)]
    fn with_total_byte_limit(max_total_bytes: usize) -> Self {
        Self {
            max_total_bytes,
            ..Self::new()
        }
    }

    #[cfg(test)]
    fn max_total_bytes(&self) -> usize {
        self.max_total_bytes
    }

    #[cfg(not(test))]
    fn max_total_bytes(&self) -> usize {
        MAX_TOTAL_BYTES
    }

    fn entries_lock(&self) -> std::sync::MutexGuard<'_, LruCache<String, ImageEntry>> {
        match self.entries.lock() {
            Ok(guard) => guard,
            Err(poisoned) => {
                log::warn!("image registry: recovering from poisoned LRU lock");
                poisoned.into_inner()
            }
        }
    }

    /// Pop the least-recently-used entry whose name does not match
    /// the in-flight insert. Returns the (name, bytes) of the
    /// evicted entry so the caller can update `total_bytes`.
    fn evict_lru_except(&mut self, name: &str) -> Option<(String, usize)> {
        let mut guard = self.entries_lock();
        // Walk the LRU end forward looking for an entry that isn't
        // the one we're making room for. The crate's `pop_lru`
        // returns the oldest, but we may need to skip past the
        // self-name to honour evict_lru_except's contract. In
        // practice the self-name is at the MRU end by construction,
        // so this loop runs at most twice.
        let mut skipped: Vec<(String, ImageEntry)> = Vec::new();
        let evicted = loop {
            let Some((candidate_name, candidate_entry)) = guard.pop_lru() else {
                break None;
            };
            if candidate_name == name {
                skipped.push((candidate_name, candidate_entry));
                continue;
            }
            break Some((candidate_name, candidate_entry));
        };
        // Restore any entries we skipped over so they keep their
        // (now-MRU) position relative to the rest. Re-inserting via
        // `put` puts them back at the MRU end which is fine here:
        // the only one we skip is the self-name, which is already
        // the most recently touched.
        for (n, e) in skipped {
            guard.put(n, e);
        }
        evicted.map(|(n, e)| (n, e.bytes))
    }

    fn projected_total(&self, name: &str, new_bytes: usize) -> usize {
        let existing = self.entries_lock().peek(name).map(|e| e.bytes).unwrap_or(0);
        self.total_bytes - existing + new_bytes
    }

    fn projected_count(&self, name: &str) -> usize {
        let guard = self.entries_lock();
        if guard.contains(name) {
            guard.len()
        } else {
            guard.len() + 1
        }
    }

    /// Evict old entries until the new image fits registry limits.
    fn make_room_for(&mut self, name: &str, new_bytes: usize) -> Result<(), String> {
        let max_total_bytes = self.max_total_bytes();

        if new_bytes > max_total_bytes {
            let msg = format!(
                "image registry: image '{}' exceeds total byte limit \
                 (bytes={new_bytes}, limit={max_total_bytes})",
                name
            );
            log::error!("{msg}");
            return Err(msg);
        }

        while self.projected_count(name) > MAX_IMAGES
            || self.projected_total(name, new_bytes) > max_total_bytes
        {
            match self.evict_lru_except(name) {
                Some((_, evicted_bytes)) => {
                    self.total_bytes -= evicted_bytes;
                }
                None => {
                    let msg = format!(
                        "image registry: cannot make room for '{}' \
                         (bytes={new_bytes}, count={}, total={}, limit={max_total_bytes})",
                        name,
                        self.entries_lock().len(),
                        self.total_bytes
                    );
                    log::error!("{msg}");
                    return Err(msg);
                }
            }
        }

        Ok(())
    }

    /// Insert a handle and update size tracking. Handles replacement of
    /// existing entries by subtracting the old size first.
    fn insert(&mut self, name: &str, handle: image::Handle, byte_count: usize) {
        let entry = ImageEntry {
            handle,
            bytes: byte_count,
        };
        let displaced_bytes = {
            let mut guard = self.entries_lock();
            guard.put(name.to_owned(), entry).map(|old| old.bytes)
        };
        if let Some(bytes) = displaced_bytes {
            self.total_bytes -= bytes;
        }
        self.total_bytes += byte_count;
    }

    fn validate_encoded_image(name: &str, data: &[u8]) -> Result<(), String> {
        if data.is_empty() {
            let msg = format!("encoded data for '{name}' is empty");
            log::error!("image registry: {msg}");
            return Err(msg);
        }

        ::image::load_from_memory(data).map_err(|err| {
            let msg = format!("encoded data for '{name}' failed image decode: {err}");
            log::error!("image registry: {msg}");
            msg
        })?;

        Ok(())
    }

    /// Store an image from encoded bytes (PNG, JPEG, etc.).
    ///
    /// Encoded data is decoded once up front so corrupt images are
    /// rejected before they can replace an existing handle.
    ///
    /// # Errors
    ///
    /// Returns a reason string when `data` is empty, exceeds
    /// `MAX_PIXEL_BYTES`, cannot be decoded, or cannot fit in the
    /// registry after evicting old handles.
    pub fn create_from_bytes(&mut self, name: &str, data: Vec<u8>) -> Result<(), String> {
        if data.len() > Self::MAX_PIXEL_BYTES {
            let msg = format!(
                "encoded data for '{}' exceeds 256 MB limit ({} bytes)",
                name,
                data.len()
            );
            log::error!("image registry: {msg}");
            return Err(msg);
        }
        Self::validate_encoded_image(name, &data)?;
        self.make_room_for(name, data.len())?;
        if sniff_image_format(&data).is_none() && !data.is_empty() {
            log::warn!(
                "image: unrecognized format (first bytes: {:02x?}), passing through [id={}]",
                &data[..data.len().min(4)],
                name
            );
        }
        let byte_count = data.len();
        self.insert(name, image::Handle::from_bytes(data), byte_count);
        Ok(())
    }

    /// Store an image from raw RGBA pixel data.
    ///
    /// # Errors
    ///
    /// Returns a reason string when `width`/`height` exceed
    /// `MAX_DIMENSION`, when the pixel buffer is the wrong length
    /// for the declared dimensions, when the pixel buffer exceeds
    /// `MAX_PIXEL_BYTES`, or when adding it would push the registry
    /// past its aggregate memory budget.
    pub fn create_from_rgba(
        &mut self,
        name: &str,
        width: u32,
        height: u32,
        pixels: Vec<u8>,
    ) -> Result<(), String> {
        if width > Self::MAX_DIMENSION || height > Self::MAX_DIMENSION {
            let msg = format!(
                "dimensions {}x{} for '{}' exceed max {}",
                width,
                height,
                name,
                Self::MAX_DIMENSION
            );
            log::error!("image registry: {msg}");
            return Err(msg);
        }

        let expected = (width as usize)
            .checked_mul(height as usize)
            .and_then(|n| n.checked_mul(4))
            .ok_or_else(|| format!("dimensions {}x{} overflow for '{name}'", width, height))?;
        if pixels.len() != expected {
            let msg = format!(
                "RGBA data size mismatch for '{}': expected {} bytes ({}x{}x4), got {}",
                name,
                expected,
                width,
                height,
                pixels.len()
            );
            log::error!("image registry: {msg}");
            return Err(msg);
        }

        if pixels.len() > Self::MAX_PIXEL_BYTES {
            let msg = format!(
                "pixel data for '{}' exceeds 256 MB limit ({} bytes)",
                name,
                pixels.len()
            );
            log::error!("image registry: {msg}");
            return Err(msg);
        }

        self.make_room_for(name, pixels.len())?;
        let byte_count = pixels.len();
        self.insert(
            name,
            image::Handle::from_rgba(width, height, pixels),
            byte_count,
        );
        Ok(())
    }

    /// Remove a named image handle.
    pub fn delete(&mut self, name: &str) {
        let removed_bytes = self.entries_lock().pop(name).map(|entry| entry.bytes);
        if let Some(bytes) = removed_bytes {
            self.total_bytes -= bytes;
        }
    }

    /// Look up a named image handle, cloning it for the caller.
    ///
    /// Takes `&self` so widgets can resolve images during render. The
    /// internal LRU promotion runs under a small lock; that lock does
    /// not make registry mutation safe from shared references.
    /// Cloning the [`image::Handle`] is cheap (it wraps an `Arc<Bytes>`
    /// internally), so the call site doesn't pay for a borrow into
    /// the LRU.
    pub fn get(&self, name: &str) -> Option<image::Handle> {
        // `LruCache::get` promotes the entry to MRU and returns a
        // reference. We clone immediately so the lock can be released
        // and the caller doesn't need to hold the registry lock for
        // the lifetime of the handle.
        self.entries_lock().get(name).map(|e| e.handle.clone())
    }

    /// Return the names of all registered image handles.
    pub fn handle_names(&self) -> Vec<String> {
        self.entries_lock()
            .iter()
            .map(|(name, _)| name.clone())
            .collect()
    }

    /// Number of images currently in the registry.
    pub fn len(&self) -> usize {
        self.entries_lock().len()
    }

    /// True when the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.entries_lock().is_empty()
    }

    /// Remove all registered image handles.
    pub fn clear(&mut self) {
        self.entries_lock().clear();
        self.total_bytes = 0;
    }

    /// Dispatch an image operation by name.
    ///
    /// Supported ops:
    /// - `"create_image"` / `"update_image"` - create or replace an image
    ///   from raw RGBA `pixels` or encoded `data` (PNG, JPEG, etc.).
    /// - `"delete_image"` - remove the named image.
    ///
    /// # Errors
    ///
    /// Returns a reason string when `op` is not one of the supported
    /// values, when a create/update op lacks both `data` and `pixels`,
    /// or when the underlying
    /// [`create_from_bytes`](Self::create_from_bytes) or
    /// [`create_from_rgba`](Self::create_from_rgba) call rejects the
    /// image.
    pub fn apply_op(
        &mut self,
        op: &str,
        handle: &str,
        data: Option<Vec<u8>>,
        pixels: Option<Vec<u8>>,
        width: Option<u32>,
        height: Option<u32>,
    ) -> Result<(), String> {
        match op {
            "create_image" | "update_image" => {
                if let Some(pixel_bytes) = pixels {
                    let w = width.unwrap_or(0);
                    let h = height.unwrap_or(0);
                    self.create_from_rgba(handle, w, h, pixel_bytes)
                } else if let Some(image_bytes) = data {
                    self.create_from_bytes(handle, image_bytes)
                } else {
                    Err(format!("image_op {op}: missing data or pixels field"))
                }
            }
            "delete_image" => {
                self.delete(handle);
                Ok(())
            }
            other => Err(format!("unknown image_op: {other}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_png() -> Vec<u8> {
        vec![
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48,
            0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00,
            0x00, 0x1F, 0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x44, 0x41, 0x54, 0x78,
            0x9C, 0x63, 0xF8, 0xCF, 0xC0, 0xF0, 0x1F, 0x00, 0x05, 0x00, 0x01, 0xFF, 0x89, 0x99,
            0x3D, 0x1D, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
        ]
    }

    #[test]
    fn new_registry_is_empty() {
        let reg = ImageRegistry::new();
        assert!(reg.get("nope").is_none());
        assert_eq!(reg.total_bytes, 0);
    }

    #[test]
    fn create_from_bytes_and_get() {
        let mut reg = ImageRegistry::new();
        let bytes = valid_png();
        let byte_count = bytes.len();
        assert!(reg.create_from_bytes("test", bytes).is_ok());
        assert!(reg.get("test").is_some());
        assert_eq!(reg.total_bytes, byte_count);
    }

    #[test]
    fn create_from_rgba_and_get() {
        let mut reg = ImageRegistry::new();
        // 1x1 RGBA pixel
        assert!(
            reg.create_from_rgba("pixel", 1, 1, vec![255, 0, 0, 255])
                .is_ok()
        );
        assert!(reg.get("pixel").is_some());
        assert_eq!(reg.total_bytes, 4);
    }

    #[test]
    fn delete_removes_handle() {
        let mut reg = ImageRegistry::new();
        let _ = reg.create_from_rgba("gone", 1, 1, vec![0, 0, 0, 0]);
        assert_eq!(reg.total_bytes, 4);
        reg.delete("gone");
        assert!(reg.get("gone").is_none());
        assert_eq!(reg.total_bytes, 0);
    }

    #[test]
    fn delete_nonexistent_is_noop() {
        let mut reg = ImageRegistry::new();
        reg.delete("never_existed");
        assert_eq!(reg.total_bytes, 0);
    }

    #[test]
    fn overwrite_replaces_handle() {
        let mut reg = ImageRegistry::new();
        let _ = reg.create_from_rgba("img", 1, 1, vec![0, 0, 0, 0]);
        assert_eq!(reg.total_bytes, 4);
        let _ = reg.create_from_rgba("img", 1, 2, vec![0; 8]);
        assert!(reg.get("img").is_some());
        assert_eq!(reg.total_bytes, 8);
    }

    #[test]
    fn corrupted_png_is_rejected() {
        let mut reg = ImageRegistry::new();
        let result = reg.create_from_bytes("bad", vec![0x89, 0x50, 0x4e, 0x47]);
        assert!(result.is_err());
        assert!(reg.get("bad").is_none());
        assert_eq!(reg.total_bytes, 0);
    }

    #[test]
    fn invalid_update_does_not_replace_existing_handle() {
        let mut reg = ImageRegistry::new();
        let bytes = valid_png();
        let byte_count = bytes.len();
        assert!(reg.create_from_bytes("img", bytes).is_ok());

        let result = reg.create_from_bytes("img", vec![0x89, 0x50, 0x4e, 0x47]);

        assert!(result.is_err());
        assert!(reg.get("img").is_some());
        assert_eq!(reg.total_bytes, byte_count);
    }

    #[test]
    fn rgba_size_mismatch_rejected() {
        let mut reg = ImageRegistry::new();
        // 2x2 RGBA should be 16 bytes, providing only 4
        let result = reg.create_from_rgba("bad", 2, 2, vec![255, 0, 0, 255]);
        assert!(result.is_err());
        assert!(reg.get("bad").is_none());
    }

    #[test]
    fn rgba_dimension_too_large_rejected() {
        let mut reg = ImageRegistry::new();
        let result = reg.create_from_rgba("huge", 16385, 1, vec![0; 16385 * 4]);
        assert!(result.is_err());
        assert!(reg.get("huge").is_none());
    }

    #[test]
    fn rgba_valid_dimensions_accepted() {
        let mut reg = ImageRegistry::new();
        // 2x2 RGBA = 16 bytes
        assert!(reg.create_from_rgba("ok", 2, 2, vec![0; 16]).is_ok());
        assert!(reg.get("ok").is_some());
    }

    #[test]
    fn sniff_png() {
        assert_eq!(
            sniff_image_format(&[0x89, 0x50, 0x4E, 0x47, 0x0D]),
            Some("PNG")
        );
    }

    #[test]
    fn sniff_jpeg() {
        assert_eq!(sniff_image_format(&[0xFF, 0xD8, 0xFF, 0xE0]), Some("JPEG"));
    }

    #[test]
    fn sniff_gif() {
        assert_eq!(sniff_image_format(b"GIF89a"), Some("GIF"));
    }

    #[test]
    fn sniff_webp() {
        let mut data = vec![0u8; 12];
        data[..4].copy_from_slice(b"RIFF");
        data[8..12].copy_from_slice(b"WEBP");
        assert_eq!(sniff_image_format(&data), Some("WebP"));
    }

    #[test]
    fn sniff_bmp() {
        assert_eq!(sniff_image_format(b"BM\x00\x00"), Some("BMP"));
    }

    #[test]
    fn sniff_unknown() {
        assert_eq!(sniff_image_format(&[0x00, 0x01, 0x02, 0x03]), None);
    }

    #[test]
    fn sniff_too_short() {
        assert_eq!(sniff_image_format(&[0x89, 0x50]), None);
    }

    // apply_op tests

    #[test]
    fn apply_op_create_from_pixels() {
        let mut reg = ImageRegistry::new();
        assert!(
            reg.apply_op(
                "create_image",
                "img",
                None,
                Some(vec![0; 4]),
                Some(1),
                Some(1)
            )
            .is_ok()
        );
        assert!(reg.get("img").is_some());
    }

    #[test]
    fn apply_op_create_from_data() {
        let mut reg = ImageRegistry::new();
        assert!(
            reg.apply_op("create_image", "img", Some(valid_png()), None, None, None)
                .is_ok()
        );
        assert!(reg.get("img").is_some());
    }

    #[test]
    fn apply_op_update_replaces() {
        let mut reg = ImageRegistry::new();
        let _ = reg.apply_op(
            "create_image",
            "img",
            None,
            Some(vec![0; 4]),
            Some(1),
            Some(1),
        );
        let _ = reg.apply_op(
            "update_image",
            "img",
            None,
            Some(vec![0; 8]),
            Some(1),
            Some(2),
        );
        assert!(reg.get("img").is_some());
    }

    #[test]
    fn apply_op_delete() {
        let mut reg = ImageRegistry::new();
        let _ = reg.apply_op(
            "create_image",
            "img",
            None,
            Some(vec![0; 4]),
            Some(1),
            Some(1),
        );
        assert!(
            reg.apply_op("delete_image", "img", None, None, None, None)
                .is_ok()
        );
        assert!(reg.get("img").is_none());
    }

    #[test]
    fn apply_op_missing_data_and_pixels() {
        let mut reg = ImageRegistry::new();
        assert!(
            reg.apply_op("create_image", "img", None, None, None, None)
                .is_err()
        );
    }

    #[test]
    fn apply_op_unknown_op() {
        let mut reg = ImageRegistry::new();
        assert!(
            reg.apply_op("rotate_image", "img", None, None, None, None)
                .is_err()
        );
    }

    #[test]
    fn delete_then_recreate_same_handle() {
        let mut reg = ImageRegistry::new();
        assert!(reg.create_from_bytes("reused", valid_png()).is_ok());
        assert!(reg.get("reused").is_some());

        reg.delete("reused");
        assert!(reg.get("reused").is_none());

        assert!(
            reg.create_from_rgba("reused", 1, 1, vec![255, 0, 0, 255])
                .is_ok()
        );
        assert!(reg.get("reused").is_some());
    }

    // Aggregate limit tests

    #[test]
    fn count_limit_evicts_lru() {
        let mut reg = ImageRegistry::new();
        for i in 0..MAX_IMAGES {
            let name = format!("img_{i}");
            assert!(
                reg.create_from_rgba(&name, 1, 1, vec![0; 4]).is_ok(),
                "image {i} should succeed"
            );
        }
        assert_eq!(reg.len(), MAX_IMAGES);

        assert!(reg.create_from_rgba("one_more", 1, 1, vec![0; 4]).is_ok());

        assert_eq!(reg.len(), MAX_IMAGES);
        assert!(reg.get("one_more").is_some());
        assert!(reg.get("img_0").is_none());
    }

    #[test]
    fn update_existing_does_not_count_as_new() {
        let mut reg = ImageRegistry::new();
        for i in 0..MAX_IMAGES {
            let name = format!("img_{i}");
            let _ = reg.create_from_rgba(&name, 1, 1, vec![0; 4]);
        }
        assert!(reg.create_from_rgba("img_0", 1, 2, vec![0; 8]).is_ok());
        assert_eq!(reg.len(), MAX_IMAGES);
        assert!(reg.get("img_0").is_some());
    }

    #[test]
    fn total_byte_limit_evicts_lru_until_image_fits() {
        let mut reg = ImageRegistry::with_total_byte_limit(12);
        let _ = reg.create_from_rgba("a", 1, 1, vec![0; 4]);
        let _ = reg.create_from_rgba("b", 1, 1, vec![0; 4]);
        let _ = reg.create_from_rgba("c", 1, 1, vec![0; 4]);

        assert!(reg.get("a").is_some());
        assert!(reg.create_from_rgba("d", 1, 2, vec![0; 8]).is_ok());

        assert!(reg.get("a").is_some());
        assert!(reg.get("b").is_none());
        assert!(reg.get("c").is_none());
        assert!(reg.get("d").is_some());
        assert_eq!(reg.total_bytes, 12);
    }

    #[test]
    fn single_image_over_total_byte_limit_is_rejected() {
        let mut reg = ImageRegistry::with_total_byte_limit(7);
        let result = reg.create_from_rgba("too_big", 1, 2, vec![0; 8]);
        assert!(result.is_err());
        assert!(reg.get("too_big").is_none());
        assert_eq!(reg.total_bytes, 0);
    }

    #[test]
    fn get_updates_recency() {
        let mut reg = ImageRegistry::new();
        for i in 0..MAX_IMAGES {
            let name = format!("img_{i}");
            let _ = reg.create_from_rgba(&name, 1, 1, vec![0; 4]);
        }

        assert!(reg.get("img_0").is_some());
        assert!(reg.create_from_rgba("new", 1, 1, vec![0; 4]).is_ok());

        assert!(reg.get("img_0").is_some());
        assert!(reg.get("img_1").is_none());
        assert!(reg.get("new").is_some());
    }

    #[test]
    fn total_bytes_tracking_across_operations() {
        let mut reg = ImageRegistry::new();
        let _ = reg.create_from_rgba("a", 1, 1, vec![0, 0, 0, 0]);
        assert_eq!(reg.total_bytes, 4);
        let _ = reg.create_from_rgba("b", 1, 1, vec![0, 0, 0, 0]);
        assert_eq!(reg.total_bytes, 8);
        reg.delete("a");
        assert_eq!(reg.total_bytes, 4);
        reg.clear();
        assert_eq!(reg.total_bytes, 0);
    }
}
