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

#[derive(Debug, Copy, Clone)]
/// Represent the position of a block on a block device.
pub struct BlockIndex(pub u32);

#[derive(Debug, Copy, Clone)]
/// Represent the count of blocks that a block device hold.
pub struct BlockCount(pub u32);

impl Block {
    /// The size of a block in bytes.
    pub const LEN: usize = 512;

    /// The size of a block in bytes as a 32 bits unsigned value.
    pub const LEN_U32: u32 = Self::LEN as u32;

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
