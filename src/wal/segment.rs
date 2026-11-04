use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use parking_lot::Mutex;
use std::sync::Arc;

use crate::error::{Result, SuperbaseError};
use crate::wal::record::{WalRecord, HEADER_SIZE};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecordPointer {
    pub segment_id: u32,
    pub offset: u64,
    pub length: u32,
    pub timestamp: u64,
}

pub struct Segment {
    pub id: u32,
    pub path: PathBuf,
    writer: Arc<Mutex<BufWriter<File>>>,
    read_file: Arc<Mutex<File>>,
    current_size: Arc<Mutex<u64>>,
}

impl Segment {
    pub fn open(dir: &Path, id: u32) -> Result<Self> {
        let filename = format!("segment_{:06}.wal", id);
        let path = dir.join(filename);

        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)?;

        let size = file.metadata()?.len();

        let read_file = OpenOptions::new()
            .read(true)
            .open(&path)?;

        Ok(Self {
            id,
            path,
            writer: Arc::new(Mutex::new(BufWriter::new(file))),
            read_file: Arc::new(Mutex::new(read_file)),
            current_size: Arc::new(Mutex::new(size)),
        })
    }

    pub fn append(&self, record: &WalRecord, sync_immediate: bool) -> Result<RecordPointer> {
        let bytes = record.encode();
        let len = bytes.len() as u32;

        let mut writer = self.writer.lock();
        let mut size_guard = self.current_size.lock();
        let offset = *size_guard;

        writer.seek(SeekFrom::Start(offset))?;
        writer.write_all(&bytes)?;

        if sync_immediate {
            writer.flush()?;
            writer.get_ref().sync_data()?;
        } else {
            writer.flush()?;
        }

        *size_guard += len as u64;

        Ok(RecordPointer {
            segment_id: self.id,
            offset,
            length: len,
            timestamp: record.timestamp,
        })
    }

    pub fn read_record_at(&self, offset: u64) -> Result<WalRecord> {
        let mut file = self.read_file.lock();
        file.seek(SeekFrom::Start(offset))?;

        match WalRecord::decode(&mut *file, offset)? {
            Some(rec) => Ok(rec),
            None => Err(SuperbaseError::Other(format!(
                "Unexpected EOF reading record at segment {} offset {}",
                self.id, offset
            ))),
        }
    }

    pub fn size(&self) -> u64 {
        *self.current_size.lock()
    }

    pub fn flush(&self) -> Result<()> {
        let mut writer = self.writer.lock();
        writer.flush()?;
        writer.get_ref().sync_data()?;
        Ok(())
    }

    /// Scan entire segment and recover records with their pointers
    pub fn scan_all(&self) -> Result<Vec<(RecordPointer, WalRecord)>> {
        let mut file = OpenOptions::new().read(true).open(&self.path)?;
        let file_len = file.metadata()?.len();
        let mut current_offset = 0u64;
        let mut results = Vec::new();

        while current_offset < file_len {
            file.seek(SeekFrom::Start(current_offset))?;
            match WalRecord::decode(&mut file, current_offset) {
                Ok(Some(record)) => {
                    let record_len = HEADER_SIZE + record.key.len() + record.value.len();
                    let ptr = RecordPointer {
                        segment_id: self.id,
                        offset: current_offset,
                        length: record_len as u32,
                        timestamp: record.timestamp,
                    };
                    results.push((ptr, record));
                    current_offset += record_len as u64;
                }
                Ok(None) => break,
                Err(SuperbaseError::ChecksumMismatch(off)) => {
                    tracing::warn!("Corruption detected at offset {}; truncating replay", off);
                    break;
                }
                Err(e) => return Err(e),
            }
        }

        // Update current size
        *self.current_size.lock() = current_offset;

        Ok(results)
    }
}
