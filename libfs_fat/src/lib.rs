//! libfs compatibility layer arround libfat.
#![feature(alloc)]
#![no_std]

extern crate alloc;

use alloc::boxed::Box;
use core::iter::Iterator;

use libfs::block::BlockDevice;

use libfs::FileSystemResult;
use libfs::{
    DirFilterFlags, DirectoryEntry, DirectoryEntryType, DirectoryOperations, FileModeFlags,
    FileOperations, FileSystemError, FileSystemOperations, FileTimeStampRaw,
};

use libfat::directory::dir_entry::DirectoryEntry as FatDirectoryEntry;
use libfat::directory::dir_entry_iterator::DirectoryEntryIterator as FatDirectoryEntryIterator;

/// A libfat directory reader implementing ``DirectoryOperations``.
struct DirectoryReader<'a, T> {
    /// The opened directory path. Used to get the complete path of every entries.
    base_path: [u8; DirectoryEntry::PATH_LEN],

    /// The iterator used to iter over libfat's directory entries.
    internal_iter: FatDirectoryEntryIterator<'a, T>,

    /// The filter required by the user.
    filter_fn: &'static dyn Fn(&FileSystemResult<FatDirectoryEntry>) -> bool,

    /// The number of entries in the directory after ``filter_fn``.
    entry_count: u64,
}

/// A libfat file interface implementing ``FileOperations``.
struct FileInterface<'a, T> {
    /// Internal interface to libfat's filesystem.
    fs: &'a libfat::filesystem::FatFileSystem<T>,

    /// The libfat's directory entry of this file.
    file_info: FatDirectoryEntry,

    /// The flags applied to the given file.
    mode: FileModeFlags,
}

/// A wrapper arround libfat ``FatFileSystem`` implementing ``FileSystemOperations``.
pub struct FatFileSystem<T> {
    /// libfat filesystem interface.
    inner: libfat::filesystem::FatFileSystem<T>,
}

/// Predicate helper used to filter directory entries.
struct DirectoryFilterPredicate;

impl DirectoryFilterPredicate {
    /// Accept all entries except "." & "..".
    fn all(entry: &FileSystemResult<FatDirectoryEntry>) -> bool {
        if entry.is_err() {
            return false;
        }

        if let Ok(entry) = entry {
            let name = entry.file_name.as_str();
            name != "." && name != ".."
        } else {
            false
        }
    }

    /// Only accept directory entries.
    fn dirs(entry: &FileSystemResult<FatDirectoryEntry>) -> bool {
        if entry.is_err() {
            return false;
        }

        if let Ok(entry_val) = entry {
            entry_val.attribute.is_directory() && Self::all(entry)
        } else {
            false
        }
    }

    /// Only accept file entries.
    fn files(entry: &FileSystemResult<FatDirectoryEntry>) -> bool {
        if entry.is_err() {
            return false;
        }

        if let Ok(entry_val) = entry {
            !entry_val.attribute.is_directory() && Self::all(entry)
        } else {
            false
        }
    }
}

impl<B> FatFileSystem<B>
where
    B: BlockDevice,
{
    /// Helper used to open a directory using the root directory.
    fn get_dir_from_path(
        &self,
        path: &str,
    ) -> FileSystemResult<libfat::directory::Directory<'_, B>> {
        if path == "/" {
            Ok(self.inner.get_root_directory())
        } else {
            self.inner.get_root_directory().open_dir(path)
        }
    }

    /// Open the given block device as a FAT filesystem.
    pub fn get_raw_partition(block_device: B) -> FileSystemResult<Self> {
        let inner_fs = libfat::get_raw_partition(block_device)?;

        Ok(FatFileSystem { inner: inner_fs })
    }
}

impl<B> FileSystemOperations for FatFileSystem<B>
where
    B: BlockDevice,
{
    fn create_file(&self, path: &str, size: u64) -> FileSystemResult<()> {
        self.inner.touch(path)?;

        let mut file = self.open_file(path, FileModeFlags::APPENDABLE)?;
        file.set_len(size)
    }

    fn create_directory(&self, path: &str) -> FileSystemResult<()> {
        self.inner.mkdir(path)
    }

    fn rename_file(&self, old_path: &str, new_path: &str) -> FileSystemResult<()> {
        self.inner.rename(old_path, new_path, false)
    }

    fn rename_directory(&self, old_path: &str, new_path: &str) -> FileSystemResult<()> {
        self.inner.rename(old_path, new_path, true)
    }

    fn delete_file(&self, path: &str) -> FileSystemResult<()> {
        self.inner.unlink(path, false)
    }

    fn delete_directory(&self, path: &str) -> FileSystemResult<()> {
        self.inner.unlink(path, true)
    }

    fn open_file<'a>(
        &'a self,
        path: &str,
        mode: FileModeFlags,
    ) -> FileSystemResult<Box<dyn FileOperations + 'a>> {
        // TODO: separate type file operation type with diferent implementation

        let file_entry = self.inner.get_root_directory().open_file(path)?;

        let res = Box::new(FileInterface {
            fs: &self.inner,
            file_info: file_entry,
            mode,
        });

        Ok(res as Box<dyn FileOperations + 'a>)
    }

    fn open_directory<'a>(
        &'a self,
        path: &str,
        filter: DirFilterFlags,
    ) -> FileSystemResult<Box<dyn DirectoryOperations + 'a>> {
        // reject path that are too big (shoudn't never happens but well we don't know)
        if path.len() >= DirectoryEntry::PATH_LEN {
            return Err(FileSystemError::NotFound);
        }

        let filter_fn: &'static dyn Fn(&FileSystemResult<FatDirectoryEntry>) -> bool =
            if (filter & DirFilterFlags::ALL) == DirFilterFlags::ALL {
                &DirectoryFilterPredicate::all
            } else if (filter & DirFilterFlags::DIRECTORY) == DirFilterFlags::DIRECTORY {
                &DirectoryFilterPredicate::dirs
            } else {
                &DirectoryFilterPredicate::files
            };

        let target_dir = self.get_dir_from_path(path)?;
        // find a better way of doing this
        let target_dir_clone = self.get_dir_from_path(path)?;

        let entry_count = target_dir.iter().filter(filter_fn).count() as u64;

        let mut data: [u8; DirectoryEntry::PATH_LEN] = [0x0; DirectoryEntry::PATH_LEN];
        for (index, c) in path
            .as_bytes()
            .iter()
            .enumerate()
            .take(DirectoryEntry::PATH_LEN)
        {
            data[index] = *c;
        }

        // Add '/' if missing at the end
        if let Some('/') = path.chars().last() {
            // Already valid
        } else {
            data[path.as_bytes().len()] = 0x2F;
        }

        let res = Box::new(DirectoryReader {
            base_path: data,
            internal_iter: target_dir_clone.iter(),
            filter_fn,
            entry_count,
        });

        Ok(res as Box<dyn DirectoryOperations + 'a>)
    }

    fn get_file_timestamp_raw(&self, name: &str) -> FileSystemResult<FileTimeStampRaw> {
        let file_entry = self.inner.get_root_directory().open_file(name)?;

        let result = FileTimeStampRaw {
            creation_timestamp: file_entry.creation_timestamp,
            modified_timestamp: file_entry.last_modification_timestamp,
            accessed_timestamp: file_entry.last_access_timestamp,
            is_valid: true,
        };

        Ok(result)
    }
}

impl<'a, T> DirectoryOperations for DirectoryReader<'a, T>
where
    T: BlockDevice,
{
    fn read(&mut self, buf: &mut [DirectoryEntry]) -> FileSystemResult<u64> {
        for (index, entry) in buf.iter_mut().enumerate() {
            let mut raw_dir_entry;
            loop {
                let entry_opt = self.internal_iter.next();

                // Prematury ending
                if entry_opt.is_none() {
                    return Ok(index as u64);
                }

                raw_dir_entry = entry_opt.unwrap();
                let filter_fn = self.filter_fn;

                if filter_fn(&raw_dir_entry) {
                    break;
                }
            }

            *entry = Self::convert_entry(raw_dir_entry?, &self.base_path);
        }

        // everything was read correctly
        Ok(buf.len() as u64)
    }

    fn entry_count(&self) -> FileSystemResult<u64> {
        Ok(self.entry_count)
    }
}

impl<'a, T> FileOperations for FileInterface<'a, T>
where
    T: BlockDevice,
{
    fn read(&mut self, offset: u64, buf: &mut [u8]) -> FileSystemResult<u64> {
        if (self.mode & FileModeFlags::READABLE) != FileModeFlags::READABLE {
            return Err(FileSystemError::AccessDenied);
        }

        self.file_info.read(self.fs, offset, buf)
    }

    fn write(&mut self, offset: u64, buf: &[u8]) -> FileSystemResult<()> {
        if (self.mode & FileModeFlags::WRITABLE) != FileModeFlags::WRITABLE {
            return Err(FileSystemError::AccessDenied);
        }

        self.file_info.write(
            self.fs,
            offset,
            buf,
            (self.mode & FileModeFlags::APPENDABLE) == FileModeFlags::APPENDABLE,
        )
    }

    fn flush(&mut self) -> FileSystemResult<()> {
        // NOP
        Ok(())
    }

    fn set_len(&mut self, size: u64) -> FileSystemResult<()> {
        if (self.mode & FileModeFlags::APPENDABLE) != FileModeFlags::APPENDABLE {
            return Err(FileSystemError::AccessDenied);
        }

        self.file_info.set_len(self.fs, size)
    }

    fn get_len(&mut self) -> FileSystemResult<u64> {
        Ok(u64::from(self.file_info.file_size))
    }
}

impl<'a, T> DirectoryReader<'a, T>
where
    T: BlockDevice,
{
    /// convert libfat's DirectoryEntry to libfs's DirectoryEntry.
    fn convert_entry(
        fat_dir_entry: FatDirectoryEntry,
        base_path: &[u8; DirectoryEntry::PATH_LEN],
    ) -> DirectoryEntry {
        let mut path: [u8; DirectoryEntry::PATH_LEN] = [0x0; DirectoryEntry::PATH_LEN];

        let file_size = fat_dir_entry.file_size;

        let entry_type = if fat_dir_entry.attribute.is_directory() {
            DirectoryEntryType::Directory
        } else {
            DirectoryEntryType::File
        };

        let mut base_index = 0;

        loop {
            let c = base_path[base_index];
            if c == 0x0 {
                break;
            }

            path[base_index] = c;
            base_index += 1;
        }

        for (index, c) in fat_dir_entry
            .file_name
            .as_bytes()
            .iter()
            .enumerate()
            .take(DirectoryEntry::PATH_LEN - base_index)
        {
            path[base_index + index] = *c;
        }

        DirectoryEntry {
            path,
            entry_type,
            file_size: u64::from(file_size),
        }
    }
}
