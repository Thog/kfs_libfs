use std::fs::File;
use std::io::prelude::*;


use std::cell::RefCell;
use std::io::SeekFrom;
use std::path::Path;

use kfs_libfs as libfs;
use libfs::*;
use libfs::fat;
use libfs::fat::block::*;

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
            file: RefCell::new(File::open(device_name).unwrap())
        })
    }
}


impl BlockDevice for LinuxBlockDevice {

    fn read(
        &self,
        blocks: &mut [Block],
        index: BlockIndex
    ) -> Result<()> {
        println!("Reading offset 0x{:x}", index.0);
        self.file
            .borrow_mut()
            .seek(SeekFrom::Start(index.into_offset())).unwrap();
        for block in blocks.iter_mut() {
            self.file.borrow_mut().read_exact(&mut block.contents).unwrap();
        }
        Ok(())
    }

    fn write(&self, blocks: &[Block], index: BlockIndex) -> Result<()> {
        self.file
            .borrow_mut()
            .seek(SeekFrom::Start(index.into_offset())).unwrap();
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

fn main() -> Result<()> {
    let mut system_device = LinuxBlockDevice::new("system.img")?;
    let filesystem = fat::get_partition(system_device, BlockIndex(0)).unwrap();
    Ok(())
}