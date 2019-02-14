use std::fs::File;
use std::io::prelude::*;

use std::cell::RefCell;
use std::io::SeekFrom;
use std::path::Path;

use kfs_libfs as libfs;
use libfs::fat;
use libfs::fat::detail;
use libfs::fat::detail::block::*;
use libfs::fat::detail::cluster::Cluster;
use libfs::*;

#[macro_use]
extern crate log;

extern crate env_logger;

#[derive(Debug)]
struct LinuxBlockDevice {
    file: RefCell<File>,
}

impl LinuxBlockDevice {
    fn new<P>(device_name: P) -> Result<LinuxBlockDevice>
    where
        P: AsRef<Path>,
    {
        Ok(LinuxBlockDevice {
            file: RefCell::new(File::open(device_name).unwrap()),
        })
    }
}

impl BlockDevice for LinuxBlockDevice {
    fn read(&self, blocks: &mut [Block], index: BlockIndex) -> Result<()> {
        /*trace!(
            "Reading block index 0x{:x} (0x{:x})",
            index.0,
            index.into_offset()
        );*/
        self.file
            .borrow_mut()
            .seek(SeekFrom::Start(index.into_offset()))
            .unwrap();
        for block in blocks.iter_mut() {
            self.file
                .borrow_mut()
                .read_exact(&mut block.contents)
                .unwrap();
        }
        Ok(())
    }

    fn write(&self, blocks: &[Block], index: BlockIndex) -> Result<()> {
        self.file
            .borrow_mut()
            .seek(SeekFrom::Start(index.into_offset()))
            .unwrap();
        for block in blocks.iter() {
            self.file.borrow_mut().write_all(&block.contents).unwrap();
        }
        Ok(())
    }

    fn count(&self) -> Result<BlockCount> {
        let num_blocks = self.file.borrow().metadata().unwrap().len() / (Block::LEN as u64);
        Ok(BlockCount(num_blocks as u32))
    }
}

// TODO: redo that after the open_dir is done
/*fn print_dir<T>(directory: Directory<T>, level: u32)
where
    T: BlockDevice,
{
    let iterator = directory.iter();
    let fs = iterator.raw_iter.cluster_iter.fs;
    for dir_entry in iterator {
        if dir_entry.file_name == "." || dir_entry.file_name == ".." {
            continue;
        }

        for i in 0..level {
            print!("    ");
        }
        println!(
            "- \"{}\" (Cluster: 0x{:x})",
            dir_entry.file_name, dir_entry.start_cluster.0
        );
        if dir_entry.attribute.is_directory() {
            let dir = Directory::from_entry(fs, dir_entry);
            print_dir(dir, level + 1);
        }
    }
}*/

fn main() -> Result<()> {
    env_logger::init();

    let system_device = LinuxBlockDevice::new("BIS-PARTITION-SYSTEM1.bin")?;
    let filesystem = fat::detail::get_raw_partition(system_device).unwrap();

    let mut root_dir = filesystem.open_directory("/save", DirFilterFlags::ALL).unwrap();

    let mut entries: [DirectoryEntry; 1] = [DirectoryEntry {
        path: [0x0; DirectoryEntry::PATH_LEN],
        entry_type: DirectoryEntryType::Directory,
        file_size: 0,
    }; 1];

    while root_dir.read(&mut entries).unwrap() != 0 {
        for entry in entries.iter() {
            let path = String::from_utf8_lossy(&entry.path);
            println!(
                "- \"{}\" (type: {:?}, file_size: {})",
                path, entry.entry_type, entry.file_size
            );
        }
    }

    Ok(())
}
