use lru::LruCache;
use spin::Mutex;

/// Represent a block operation error.
#[derive(Debug)]
pub enum BlockError {
    /// Read error
    ReadError,

    /// Write error
    WriteError,

    /// Unknown error
    Unknown,
}

/// Represent a block operation result.
pub type BlockResult<T> = core::result::Result<T, BlockError>;

/// Represent a certain amount of data from a block device.
#[derive(Clone)]
pub struct Block {
    /// The actual storage of the block.
    pub contents: [u8; Block::LEN],
}

#[derive(Debug, Copy, Clone, Hash, PartialOrd, PartialEq, Ord, Eq)]
/// Represent the position of a block on a block device.
pub struct BlockIndex(pub u64);

#[derive(Debug, Copy, Clone)]
/// Represent the count of blocks that a block device hold.
pub struct BlockCount(pub u64);

impl BlockCount {
    /// Get the block count as a raw bytes count.
    pub fn into_bytes_count(self) -> u64 {
        self.0 * Block::LEN_U64
    }
}

impl Block {
    /// The size of a block in bytes.
    pub const LEN: usize = 512;

    /// The size of a block in bytes as a 64 bits unsigned value.
    pub const LEN_U64: u64 = Self::LEN as u64;

    /// Create a new block instance.
    pub fn new() -> Block {
        Block::default()
    }

    /// Return the content of the block.
    pub fn as_contents(&self) -> [u8; Block::LEN] {
        self.contents
    }
}

impl Default for Block {
    fn default() -> Self {
        Block {
            contents: [0u8; Self::LEN],
        }
    }
}

impl core::ops::Deref for Block {
    type Target = [u8; Block::LEN];
    fn deref(&self) -> &Self::Target {
        &self.contents
    }
}

impl core::ops::DerefMut for Block {
    fn deref_mut(&mut self) -> &mut [u8; Block::LEN] {
        &mut self.contents
    }
}

impl BlockIndex {
    /// Convert the block index into an offset in bytes.
    pub fn into_offset(self) -> u64 {
        u64::from(self.0) * (Block::LEN as u64)
    }
}

impl BlockCount {
    /// Convert the block count into a size in bytes.
    pub fn into_size(self) -> u64 {
        u64::from(self.0) * (Block::LEN as u64)
    }
}

/// Represent a device holding blocks.
pub trait BlockDevice: Sized {
    /// Read blocks from the block device starting at the given ``index``.
    fn raw_read(&self, blocks: &mut [Block], index: BlockIndex) -> BlockResult<()>;

    /// Write blocks to the block device starting at the given ``index``.
    fn raw_write(&self, blocks: &[Block], index: BlockIndex) -> BlockResult<()>;

    /// Read blocks from the block device starting at the given ``partition_start + index``.
    fn read(
        &self,
        blocks: &mut [Block],
        partition_start: BlockIndex,
        index: BlockIndex,
    ) -> BlockResult<()> {
        self.raw_read(blocks, BlockIndex(partition_start.0 + index.0))
    }

    /// Write blocks to the block device starting at the given ``partition_start + index``.
    fn write(
        &self,
        blocks: &[Block],
        partition_start: BlockIndex,
        index: BlockIndex,
    ) -> BlockResult<()> {
        self.raw_write(blocks, BlockIndex(partition_start.0 + index.0))
    }

    /// Return the amount of blocks hold by the block device.
    fn count(&self) -> BlockResult<BlockCount>;
}

/// A BlockDevice that reduces device accesses by keeping the most recently used blocks in a cache.
///
/// It will keep track of which blocks are dirty, and will only write those ones to device when
/// flushing, or when they are evicted from the cache.
///
/// When a CachedBlockDevice is dropped, it flushes its cache.
pub struct CachedBlockDevice<B: BlockDevice> {
    /// The inner block device.
    block_device: B,

    /// The LRU cache.
    lru_cache: Mutex<LruCache<BlockIndex, CachedBlock>>,
}

/// Represent a cached block in the LRU cache.
struct CachedBlock {
    /// Bool indicating whether this block should be written to device when flushing.
    dirty: bool,
    /// The data of this block.
    data: Block,
}

impl<B: BlockDevice> CachedBlockDevice<B> {
    /// Creates a new CachedBlockDevice that wraps `device`, and can hold at most `cap` blocks in cache.
    pub fn new(device: B, cap: usize) -> CachedBlockDevice<B> {
        CachedBlockDevice {
            block_device: device,
            lru_cache: Mutex::new(LruCache::new(cap)),
        }
    }

    /// Writes every dirty cached block to device.
    ///
    /// Note that this will not empty the cache, just perform device writes
    /// and update dirty blocks as now non-dirty.
    ///
    /// This function has no effect on lru order.
    pub fn flush(&self) -> BlockResult<()> {
        for (index, block) in self.lru_cache.lock().iter_mut() {
            if block.dirty {
                self.block_device
                    .raw_write(core::slice::from_ref(&block.data), *index)?;
                block.dirty = false;
            }
        }
        Ok(())
    }
}

impl<B: BlockDevice> Drop for CachedBlockDevice<B> {
    /// Dropping a CachedBlockDevice flushes it.
    ///
    /// If a device write fails, it is silently ignored.
    fn drop(&mut self) {
        let _ = self.flush();
    }
}

impl<B: BlockDevice> BlockDevice for CachedBlockDevice<B> {
    /// Attempts to fill `blocks` with blocks found in the cache, and will fetch them from device if it can't.
    ///
    /// Will update the access time of every block involved.
    fn raw_read(&self, blocks: &mut [Block], index: BlockIndex) -> BlockResult<()> {
        let mut lru = self.lru_cache.lock();
        // check if we can satisfy the request only from what we have in cache
        let mut fully_cached = true;
        if blocks.len() > lru.len() {
            // requested more blocks that cache is holding
            fully_cached = false
        } else {
            // check each block is found in the cache
            for i in 0..blocks.len() {
                if !lru.contains(&BlockIndex(index.0 + i as u64)) {
                    fully_cached = false;
                    break;
                }
            }
        }

        if !fully_cached {
            // we must read from device
            self.block_device.raw_read(blocks, index)?
        }

        // update from/to cache
        for (i, block) in blocks.iter_mut().enumerate() {
            if let Some(cached_block) = lru.get(&BlockIndex(index.0 + i as u64)) {
                // block was found in cache, its access time was updated.
                if fully_cached || cached_block.dirty {
                    // fully_cached: block[i] is uninitialized, copy it from cache.
                    // dirty:        block[i] is initialized from device if !fully_cached,
                    //               but we hold a newer dirty version in cache, overlay it.
                    *block = cached_block.data.clone();
                }
            } else {
                // add the block we just read to the cache.
                // if cache is full, flush its lru entry
                if lru.len() == lru.cap() {
                    let (evicted_index, evicted_block) = lru.pop_lru().unwrap();
                    if evicted_block.dirty {
                        self.block_device
                            .raw_write(core::slice::from_ref(&evicted_block.data), evicted_index)?;
                    }
                }
                let new_cached_block = CachedBlock {
                    dirty: false,
                    data: block.clone(),
                };
                lru.put(BlockIndex(index.0 + i as u64), new_cached_block);
            }
        }
        Ok(())
    }

    /// Adds dirty blocks to the cache.
    ///
    /// If the block was already present in the cache, it will simply be updated.
    ///
    /// When the cache is full, least recently used blocks will be evicted and written to device.
    /// This operation may fail, and this function will return an error when it happens.
    fn raw_write(&self, blocks: &[Block], index: BlockIndex) -> BlockResult<()> {
        let mut lru = self.lru_cache.lock();

        if blocks.len() < lru.cap() {
            for (i, block) in blocks.iter().enumerate() {
                let new_block = CachedBlock {
                    dirty: true,
                    data: block.clone(),
                };
                // add it to the cache
                // if cache is full, flush its lru entry
                if lru.len() == lru.cap() {
                    let (evicted_index, evicted_block) = lru.pop_lru().unwrap();
                    if evicted_block.dirty {
                        self.block_device
                            .raw_write(core::slice::from_ref(&evicted_block.data), evicted_index)?;
                    }
                }
                lru.put(BlockIndex(index.0 + i as u64), new_block);
            }
        } else {
            // we're performing a big write, that will evict all cache blocks.
            // evict it in one go, and repopulate with the first `cap` blocks from `blocks`.
            for (evicted_index, evicted_block) in lru.iter() {
                if evicted_block.dirty
                    // if evicted block is `blocks`, don't bother writing it as we're about to re-write it anyway.
                    && !(index >= *evicted_index && index < BlockIndex(evicted_index.0 + blocks.len() as u64))
                {
                    self.block_device
                        .raw_write(core::slice::from_ref(&evicted_block.data), *evicted_index)?;
                }
            }
            // write in one go
            self.block_device.raw_write(blocks, index)?;
            // add first `cap` blocks to cache
            for (i, block) in blocks.iter().take(lru.cap()).enumerate() {
                lru.put(
                    BlockIndex(index.0 + i as u64),
                    CachedBlock {
                        dirty: false,
                        data: block.clone(),
                    },
                )
            }
        }
        Ok(())
    }

    fn count(&self) -> BlockResult<BlockCount> {
        self.block_device.count()
    }
}

use crate::storage::StorageDevice;
use crate::storage::StorageDeviceError;
use crate::storage::StorageDeviceResult;

impl From<BlockError> for StorageDeviceError {
    fn from(error: BlockError) -> Self {
        match error {
            BlockError::ReadError => StorageDeviceError::ReadError,
            BlockError::WriteError => StorageDeviceError::WriteError,
            BlockError::Unknown => StorageDeviceError::Unknown,
        }
    }
}

/// Implementation of storage device for block device
pub struct StorageBlockDevice<B: BlockDevice> {
    /// The inner block device.
    block_device: B,
}

impl<B: BlockDevice> StorageBlockDevice<B> {
    /// Create a new storage block device
    pub fn new(block_device: B) -> Self {
        StorageBlockDevice {
            block_device
        }
    }
}

impl<B: BlockDevice> StorageDevice for StorageBlockDevice<B> {
    fn read(&self, offset: u64, buf: &mut [u8]) -> StorageDeviceResult<()> {
        let mut read_size = 0u64;
        let mut blocks = [Block::new()];

        while read_size < buf.len() as u64 {
            // Compute the next offset of the data to read.
            let current_offset = offset + read_size;

            // Extract the block index containing the data.
            let current_block_index = BlockIndex(current_offset / Block::LEN_U64);

            // Extract the offset inside the block containing the data.
            let current_block_offset = current_offset % Block::LEN_U64;

            // Read the block.
            self.block_device
                .raw_read(&mut blocks, BlockIndex(current_block_index.0))?;

            // Slice on the part of the buffer we need.
            let buf_slice = &mut buf[read_size as usize..];

            // Limit copy to the size of a block or lower
            let buf_limit = if buf_slice.len() + current_block_offset as usize >= Block::LEN {
                Block::LEN - current_block_offset as usize
            } else {
                buf_slice.len()
            };

            // Copy the data into the buffer.
            for (index, buf_entry) in buf_slice.iter_mut().take(buf_limit).enumerate() {
                *buf_entry = blocks[0][current_block_offset as usize + index];
            }

            // Increment with what we read.
            read_size += buf_limit as u64;
        }

        Ok(())
    }

    fn write(&self, offset: u64, buf: &[u8]) -> StorageDeviceResult<()> {
        let mut write_size = 0u64;
        let mut blocks = [Block::new()];

        while write_size < buf.len() as u64 {
            // Compute the next offset of the data to write.
            let current_offset = offset + write_size;

            // Extract the block index containing the data.
            let current_block_index = BlockIndex(current_offset / Block::LEN_U64);

            // Extract the offset inside the block containing the data.
            let current_block_offset = current_offset % Block::LEN_U64;

            // Read the block.
            self.block_device
                .raw_read(&mut blocks, BlockIndex(current_block_index.0))?;

            // Slice on the part of the buffer we need.
            let buf_slice = &buf[write_size as usize..];

            // Limit copy to the size of a block or lower
            let buf_limit = if buf_slice.len() + current_block_offset as usize >= Block::LEN {
                Block::LEN - current_block_offset as usize
            } else {
                buf_slice.len()
            };

            let block_slice = &mut blocks[0][current_block_offset as usize..];

            // Copy the data from the buffer.
            for (index, buf_entry) in block_slice.iter_mut().take(buf_limit).enumerate() {
                *buf_entry = buf_slice[index];
            }

            self.block_device
                .raw_write(&blocks, BlockIndex(current_block_index.0))?;

            // Increment with what we wrote.
            write_size += buf_limit as u64;
        }

        Ok(())
    }

    fn len(&self) -> StorageDeviceResult<u64> {
        Ok(self.block_device.count()?.into_bytes_count())
    }
}
