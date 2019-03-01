use std::fs::File;
use std::fs::OpenOptions;
use std::io::prelude::*;

use std::cell::RefCell;
use std::io::SeekFrom;
use std::path::Path;

use kfs_libfs as libfs;
use libfs::fat;
use libfs::fat::detail::block::*;
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
            file: RefCell::new(
                OpenOptions::new()
                    .read(true)
                    .write(true)
                    .open(device_name)
                    .unwrap(),
            ),
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

fn print_dir<T>(filesystem: &T, path: &str, level: u32)
where
    T: FileSystemOperations,
{
    let mut root_dir = filesystem
        .open_directory(path, DirFilterFlags::ALL)
        .unwrap();

    let mut entries: [DirectoryEntry; 1] = [DirectoryEntry {
        path: [0x0; DirectoryEntry::PATH_LEN],
        entry_type: DirectoryEntryType::Directory,
        file_size: 0,
    }; 1];

    while root_dir.read(&mut entries).unwrap() != 0 {
        for entry in entries.iter() {
            let path = String::from_utf8_lossy(&entry.path);
            let entry_name = path.trim_matches(char::from(0));

            for i in 0..level {
                print!("    ");
            }

            println!(
                "- \"{}\" (type: {:?}, file_size: {})",
                entry_name, entry.entry_type, entry.file_size
            );

            if entry.entry_type == DirectoryEntryType::Directory {
                print_dir(filesystem, entry_name, level + 1);
            }
        }
    }
}

fn dump_to_file<'a>(file: &mut Box<dyn FileOperations + 'a>, path: &str) {
    let mut f = File::create(path).unwrap();

    let mut buffer: [u8; 0x200] = [0x0; 0x200];
    let mut offset = 0;

    loop {
        let read_size = file.read(offset as u64, &mut buffer).unwrap() as usize;
        f.write_all(&buffer[0..read_size]).unwrap();
        if read_size == 0 {
            break;
        }
        offset += read_size;
    }
}

fn main() -> Result<()> {
    env_logger::init();

    let system_device = LinuxBlockDevice::new("system.bin")?;
    let filesystem = fat::detail::get_raw_partition(system_device).unwrap();
    //print_dir(&filesystem, "/", 0);

    //let allocated_cluster = filesystem.alloc_cluster(None).unwrap();
    //println!("Allocated Cluster {}", allocated_cluster.0);

    //filesystem.free_cluster(allocated_cluster, None).unwrap();
    //filesystem.unlink("/saveMeta/0000000000000015").unwrap();

    let mut some_file = filesystem
        .open_file(
            "PRF2SAFE.RCV",
            FileModeFlags::READABLE | FileModeFlags::WRITABLE,
        )
        .unwrap();

    dump_to_file(&mut some_file, "PRF2SAFE_SAVE.RCV");    
    some_file.set_len(20).unwrap();
    let file_len = some_file.get_len().unwrap();
    let data = b"HELLO WORLD";
    some_file.write(file_len - data.len() as u64, data).unwrap();
    dump_to_file(&mut some_file, "PRF2SAFE.RCV");
    Ok(())
}
