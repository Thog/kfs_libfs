/// Represent a storage device operation error.
#[derive(Debug)]
pub enum StorageDeviceError {
    /// Read error
    ReadError,

    /// Write error
    WriteError,

    /// Unknown error
    Unknown,
}

/// Represent a storage device operation result.
pub type StorageDeviceResult<T> = core::result::Result<T, StorageDeviceError>;

/// Represent a device managing storage.
// we don't need is_empty, this would be stupid.
#[allow(clippy::len_without_is_empty)]
pub trait StorageDevice {
    /// Read the data at the given offset in the storage device into a given buffer.
    fn read(&self, offset: u64, buf: &mut [u8]) -> StorageDeviceResult<()>;

    /// Write the data at the given offset into the storage device.
    fn write(&self, offset: u64, buf: &[u8]) -> StorageDeviceResult<()>;

    /// Return the total size of the storage device.
    fn len(&self) -> StorageDeviceResult<u64>;
}
