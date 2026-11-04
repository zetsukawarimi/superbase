pub mod record;
pub mod segment;

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use parking_lot::RwLock;

use crate::error::{Result, SuperbaseError};
pub use record::{OpType, WalRecord};
pub use segment::{RecordPointer, Segment};

pub const DEFAULT_MAX_SEGMENT_SIZE: u64 = 64 * 1024 * 1024; // 64 MB

pub struct WalEngine {
    dir: PathBuf,
    active_segment_id: AtomicU32,
    segments: Arc<RwLock<HashMap<u32, Arc<Segment>>>>,
    next_seq: AtomicU64,
    max_segment_size: u64,
    sync_immediate: bool,
}

impl WalEngine {
    pub fn open<P: AsRef<Path>>(dir: P, max_segment_size: Option<u64>, sync_immediate: bool) -> Result<Self> {
        let dir = dir.as_ref().to_path_buf();
        fs::create_dir_all(&dir)?;

        let mut segment_ids = Vec::new();
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            if let Some(filename) = path.file_name().and_then(|f| f.to_str()) {
                if filename.starts_with("segment_") && filename.ends_with(".wal") {
                    let num_str = &filename[8..filename.len() - 4];
                    if let Ok(id) = num_str.parse::<u32>() {
                        segment_ids.push(id);
                    }
                }
            }
        }
        segment_ids.sort_unstable();

        let mut segments_map = HashMap::new();
        let mut active_id = 0u32;

        if segment_ids.is_empty() {
            let seg = Arc::new(Segment::open(&dir, 0)?);
            segments_map.insert(0, seg);
            active_id = 0;
        } else {
            for &id in &segment_ids {
                let seg = Arc::new(Segment::open(&dir, id)?);
                segments_map.insert(id, seg);
                if id > active_id {
                    active_id = id;
                }
            }
        }

        let max_seg_size = max_segment_size.unwrap_or(DEFAULT_MAX_SEGMENT_SIZE);

        Ok(Self {
            dir,
            active_segment_id: AtomicU32::new(active_id),
            segments: Arc::new(RwLock::new(segments_map)),
            next_seq: AtomicU64::new(1),
            max_segment_size: max_seg_size,
            sync_immediate,
        })
    }

    pub fn set_seq(&self, seq: u64) {
        self.next_seq.store(seq, Ordering::SeqCst);
    }

    pub fn get_next_seq(&self) -> u64 {
        self.next_seq.fetch_add(1, Ordering::SeqCst)
    }

    fn get_active_segment(&self) -> Result<Arc<Segment>> {
        let active_id = self.active_segment_id.load(Ordering::SeqCst);
        let segments = self.segments.read();
        segments.get(&active_id).cloned().ok_or_else(|| {
            SuperbaseError::Other(format!("Active segment {} not found", active_id))
        })
    }

    fn maybe_rotate_segment(&self) -> Result<Arc<Segment>> {
        let mut active = self.get_active_segment()?;
        if active.size() >= self.max_segment_size {
            let mut segments = self.segments.write();
            let new_id = self.active_segment_id.fetch_add(1, Ordering::SeqCst) + 1;
            let new_seg = Arc::new(Segment::open(&self.dir, new_id)?);
            segments.insert(new_id, new_seg.clone());
            active = new_seg;
        }
        Ok(active)
    }

    pub fn write_put(&self, key: String, value: Vec<u8>) -> Result<RecordPointer> {
        let seq = self.get_next_seq();
        let record = WalRecord::new_put(seq, key, value);
        let segment = self.maybe_rotate_segment()?;
        segment.append(&record, self.sync_immediate)
    }

    pub fn write_delete(&self, key: String) -> Result<RecordPointer> {
        let seq = self.get_next_seq();
        let record = WalRecord::new_delete(seq, key);
        let segment = self.maybe_rotate_segment()?;
        segment.append(&record, self.sync_immediate)
    }

    pub fn read_record(&self, ptr: &RecordPointer) -> Result<WalRecord> {
        let segment = {
            let segments = self.segments.read();
            segments.get(&ptr.segment_id).cloned().ok_or_else(|| {
                SuperbaseError::Other(format!("Segment {} not found", ptr.segment_id))
            })?
        };
        segment.read_record_at(ptr.offset)
    }

    pub fn list_segments(&self) -> Vec<u32> {
        let segments = self.segments.read();
        let mut ids: Vec<u32> = segments.keys().copied().collect();
        ids.sort_unstable();
        ids
    }

    pub fn get_segment(&self, id: u32) -> Option<Arc<Segment>> {
        self.segments.read().get(&id).cloned()
    }

    pub fn flush(&self) -> Result<()> {
        let seg = self.get_active_segment()?;
        seg.flush()
    }

    /// Compaction: Given a map of active valid keys to their RecordPointer,
    /// rewrite them into a fresh compacted segment and safely delete obsolete segments.
    pub fn compact(&self, live_pointers: &[(String, RecordPointer)]) -> Result<HashMap<String, RecordPointer>> {
        let mut new_pointers = HashMap::new();
        if live_pointers.is_empty() {
            return Ok(new_pointers);
        }

        let new_compaction_id = {
            let mut segments = self.segments.write();
            let new_id = self.active_segment_id.fetch_add(1, Ordering::SeqCst) + 1;
            let new_seg = Arc::new(Segment::open(&self.dir, new_id)?);
            segments.insert(new_id, new_seg);
            new_id
        };

        let target_seg = self.get_segment(new_compaction_id).unwrap();

        for (key, ptr) in live_pointers {
            let record = self.read_record(ptr)?;
            if record.op == OpType::Put {
                let new_ptr = target_seg.append(&record, true)?;
                new_pointers.insert(key.clone(), new_ptr);
            }
        }

        // Clean up old segments except the newly created one
        let mut segments = self.segments.write();
        let old_ids: Vec<u32> = segments.keys().filter(|&&id| id < new_compaction_id).copied().collect();

        for old_id in old_ids {
            if let Some(old_seg) = segments.remove(&old_id) {
                drop(old_seg);
                let old_path = self.dir.join(format!("segment_{:06}.wal", old_id));
                let _ = fs::remove_file(old_path);
            }
        }

        Ok(new_pointers)
    }
}
