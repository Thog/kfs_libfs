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
use libfs::fat::detail::directory::Directory;
use libfs::fat::detail::table::FatClusterIter;

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

fn print_dir<T>(directory: Directory<T>, level: u32)
where
    T: BlockDevice,
{
    for dir_entry in directory.iter() {
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
            let dir = Directory::from_entry(directory.fs, dir_entry);
            print_dir(dir, level + 1);
        }
    }
}

fn main() -> Result<()> {
    env_logger::init();

    let system_device = LinuxBlockDevice::new("BIS-PARTITION-SYSTEM1.bin")?;
    let filesystem = fat::detail::get_raw_partition(system_device).unwrap();

    let root_dir = filesystem.get_root_directory();
    println!("{:?}", root_dir.dir_info);

    print_dir(root_dir, 0);

    let root_dir = filesystem.get_root_directory();

    Ok(())
}
