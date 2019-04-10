use std::fs::File;
use std::fs::OpenOptions;
use std::io::prelude::*;

use std::cell::RefCell;
use std::io::SeekFrom;
use std::path::Path;

use libfs::block::*;
use libfs::*;

#[macro_use]
extern crate log;

extern crate env_logger;

#[derive(Debug)]
struct LinuxBlockDevice {
    file: RefCell<File>,
}

impl LinuxBlockDevice {
    fn new<P>(device_name: P) -> BlockResult<LinuxBlockDevice>
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
    fn raw_read(&self, blocks: &mut [Block], index: BlockIndex) -> BlockResult<()> {
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

    fn raw_write(&self, blocks: &[Block], index: BlockIndex) -> BlockResult<()> {
        self.file
            .borrow_mut()
            .seek(SeekFrom::Start(index.into_offset()))
            .unwrap();
        for block in blocks.iter() {
            self.file.borrow_mut().write_all(&block.contents).unwrap();
        }
        Ok(())
    }

    fn count(&self) -> BlockResult<BlockCount> {
        let num_blocks = self.file.borrow().metadata().unwrap().len() / (Block::LEN_U64);
        Ok(BlockCount(num_blocks))
    }
}

fn print_dir<T>(filesystem: &T, path: &str, level: u32, recursive: bool) -> FileSystemResult<()>
where
    T: FileSystemOperations,
{
    let mut root_dir = filesystem.open_directory(path, DirFilterFlags::ALL)?;

    let mut entries: [DirectoryEntry; 1] = [DirectoryEntry {
        path: [0x0; DirectoryEntry::PATH_LEN],
        entry_type: DirectoryEntryType::Directory,
        file_size: 0,
    }; 1];

    while root_dir.read(&mut entries).unwrap() != 0 {
        for entry in entries.iter() {
            let path = String::from_utf8_lossy(&entry.path);
            let entry_name = path.trim_matches(char::from(0));

            for _ in 0..level {
                print!("    ");
            }

            println!(
                "- \"{}\" (type: {:?}, file_size: {}, timestamp: {:?})",
                entry_name,
                entry.entry_type,
                entry.file_size,
                filesystem.get_file_timestamp_raw(entry_name)
            );

            if entry.entry_type == DirectoryEntryType::Directory && recursive {
                print_dir(filesystem, entry_name, level + 1, recursive)?;
            }
        }
    }

    Ok(())
}

fn dump_to_file<'a>(file: &mut Box<dyn FileOperations + 'a>, path: &str) -> FileSystemResult<()> {
    let mut f = File::create(path).unwrap();

    let file_len = file.get_len()?;

    let mut buffer = Vec::new();
    buffer.resize(file_len as usize, 0u8);

    let read_size = file.read(0, &mut buffer)? as usize;
    f.write_all(&buffer[0..read_size]).unwrap();

    Ok(())
}

fn dump_to_file_buffered<'a>(
    file: &mut Box<dyn FileOperations + 'a>,
    path: &str,
    buffer: &mut [u8],
) -> FileSystemResult<()> {
    let mut f = File::create(path).unwrap();

    let mut offset = 0;

    loop {
        let read_size = file.read(offset as u64, buffer)? as usize;
        f.write_all(&buffer[0..read_size]).unwrap();
        if read_size == 0 {
            break;
        }
        offset += read_size;
    }

    Ok(())
}

fn dump_to_filesystem<'a>(
    file: &mut Box<dyn FileOperations + 'a>,
    path: &str,
) -> FileSystemResult<()> {
    let mut f = OpenOptions::new()
        .read(true)
        .write(false)
        .open(path)
        .unwrap();

    let mut base_buffer = Vec::new();
    f.read_to_end(&mut base_buffer).unwrap();
    file.write(0 as u64, &base_buffer)?;

    Ok(())
}

fn main() -> FileSystemResult<()> {
    env_logger::init();

    let system_device =
        StorageBlockDevice::new(LinuxBlockDevice::new(std::env::args().nth(1).unwrap()).unwrap());
    let filesystem = libfs_fat::FatFileSystem::get_raw_partition(system_device)?;

    print_dir(&filesystem, "/", 0, true)?;

    //filesystem.create_file("/hello_world.txt", 0)?;

    /*let mut file = filesystem.open_file(
        "/hello_world.txt",
        FileModeFlags::READABLE | FileModeFlags::WRITABLE | FileModeFlags::APPENDABLE,
    )?*/
;
    //dump_to_filesystem(&mut file, "PRF2SAFE.RCV")?;
    //dump_to_file(&mut file, "hello_world.txt")?;

    //filesystem.rename_directory("/save", "/this_is_a_long_name")?;

    //print_dir(&filesystem, "/", 0, false)?;
    Ok(())
}
