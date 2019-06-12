//! Interface to manipulate filesystem
#![no_std]

extern crate alloc;

#[macro_use]
extern crate bitflags;

use alloc::boxed::Box;

/// Represent a filesystem error.
#[derive(Debug)]
pub enum FileSystemError {
    /// Unknown error.
    Unknown,

    /// The given resource couldn't be found.
    NotFound,

    /// There isn't enough space for a resource to be stored.
    NoSpaceLeft,

    /// The access to a given resource has been denied.
    AccessDenied,

    /// A writing operation failed on the attached storage device.
    WriteFailed,

    /// A read operation failed on the attached storage device.
    ReadFailed,

    /// The given partition cannot be found.
    PartitionNotFound,

    /// The given resource cannot be represented as a file.
    NotAFile,

    /// The given resource cannot be represented as a directory.
    NotADirectory,

    /// A resource at the given path already exist.
    FileExists,

    /// The given path is too long to be resolved.
    PathTooLong,

    /// The partition wasn't used as it's invalid.
    InvalidPartition,
}

/// Represent the type of a given resource when walking a directory.
#[derive(Debug, PartialEq)]
pub enum DirectoryEntryType {
    /// The entry is a file.
    File,
    /// The entry is a directory.
    Directory,
}

/// Represent an entry inside a directory.
pub struct DirectoryEntry {
    /// The path of the resource.
    pub path: [u8; Self::PATH_LEN],

    /// The type of the resource.
    pub entry_type: DirectoryEntryType,

    /// The size of the file. (0 if it's a directory)
    pub file_size: u64,
}

impl DirectoryEntry {
    /// Represent the max path size (in bytes) supported.
    pub const PATH_LEN: usize = 0x301;
}

bitflags! {
    /// Flags indicating the way a file should be open.
    pub struct FileModeFlags: u32 {
        // The file should be readable.
        const READABLE = 0b0000_0001;

        // The file should be writable.
        const WRITABLE = 0b0000_0010;

        // The file should be appendable.
        const APPENDABLE = 0b0000_0100;
    }
}

bitflags! {
    /// Flags indicating the filters when walking a directory.
    pub struct DirFilterFlags: u32 {
        /// Accept directories.
        const DIRECTORY = 0b0000_0001;

        /// Accept files.
        const FILE = 0b0000_0010;

        /// Do not filter anything.
        const ALL = Self::DIRECTORY.bits | Self::FILE.bits;
    }
}

/// Represent the attached timestamps on a given resource.
#[derive(Debug)]
pub struct FileTimeStampRaw {
    /// The resource creation UNIX timestamp.
    pub creation_timestamp: u64,

    /// The resource last modification UNIX timestamp.
    pub modified_timestamp: u64,

    /// The resource last access UNIX timestamp.
    pub accessed_timestamp: u64,

    /// false if one of the given timestamp couldn't have been retrieved.
    pub is_valid: bool,
}

/// Represent a filesystem result.
pub type FileSystemResult<T> = core::result::Result<T, FileSystemError>;

/// Represent the operation on a file.
pub trait FileOperations {
    /// Read the content of a file at a given ``offset`` in ``buf``.
    fn read(&mut self, offset: u64, buf: &mut [u8]) -> FileSystemResult<u64>;

    /// Write the content given ``buf`` at the given ``offset`` in the file.
    /// If the file is too small to hold the data and the appendable flag is set, it will resize the file and append the data.
    /// If the file is too small to hold the data and the appendable flag isn't set, this will return a FileSystemError::NoSpaceLeft.
    fn write(&mut self, offset: u64, buf: &[u8]) -> FileSystemResult<()>;

    /// Flush any data not written on the filesystem.
    fn flush(&mut self) -> FileSystemResult<()>;

    /// Resize the file with the given ``size``.
    /// If the file isn't open with the appendable flag, it will not be extendable and will return a FileSystemError::NoSpaceLeft.
    fn set_len(&mut self, size: u64) -> FileSystemResult<()>;

    /// Return the current file size.
    fn get_len(&mut self) -> FileSystemResult<u64>;
}

/// Represent the operation on a directory.
pub trait DirectoryOperations {
    /// Read the next directory entries and return the number of entries read.
    fn read(&mut self, buf: &mut [DirectoryEntry]) -> FileSystemResult<u64>;

    /// Return the count of entries in the directory.
    fn entry_count(&self) -> FileSystemResult<u64>;
}

/// Represent the operation on a filesystem.
pub trait FileSystemOperations {
    /// Create a file with a given ``size`` at the specified ``path``.
    fn create_file(&self, path: &str, size: u64) -> FileSystemResult<()>;

    /// Create a directory at the specified ``path``.
    fn create_directory(&self, path: &str) -> FileSystemResult<()>;

    /// Rename a file at ``old_path`` into ``new_path``.
    fn rename_file(&self, old_path: &str, new_path: &str) -> FileSystemResult<()>;

    /// Rename a directory at ``old_path`` into ``new_path``
    fn rename_directory(&self, old_path: &str, new_path: &str) -> FileSystemResult<()>;

    /// Delete a file at the specified ``path``.
    fn delete_file(&self, path: &str) -> FileSystemResult<()>;

    /// Delete a directory at the specified ``path``.
    fn delete_directory(&self, path: &str) -> FileSystemResult<()>;

    /// Open a file at the specified ``path`` with the given ``mode`` flags.
    fn open_file<'a>(
        &'a self,
        path: &str,
        mode: FileModeFlags,
    ) -> FileSystemResult<Box<dyn FileOperations + 'a>>;

    /// Open a directory at the specified ``path`` with the given ``mode`` flags.
    fn open_directory<'a>(
        &'a self,
        path: &str,
        filter: DirFilterFlags,
    ) -> FileSystemResult<Box<dyn DirectoryOperations + 'a>>;

    /// Return the attached timestamps on a resource at the given ``path``.
    fn get_file_timestamp_raw(&self, path: &str) -> FileSystemResult<FileTimeStampRaw>;
}
