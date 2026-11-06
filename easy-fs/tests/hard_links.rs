/* Exercise inode sharing, unlink-open lifetime, and bitmap reuse on a memory disk. */
use easy_fs::{BlockDevice, EasyFileSystem, BLOCK_SZ};
use std::sync::{Arc, Mutex};

struct MemoryDisk(Mutex<Vec<[u8; BLOCK_SZ]>>);

impl BlockDevice for MemoryDisk {
    fn read_block(&self, block_id: usize, buf: &mut [u8]) {
        buf.copy_from_slice(&self.0.lock().unwrap()[block_id]);
    }

    fn write_block(&self, block_id: usize, buf: &[u8]) {
        self.0.lock().unwrap()[block_id].copy_from_slice(buf);
    }
}

#[test]
fn hard_links_preserve_open_files_and_reclaim_storage() {
    let disk = Arc::new(MemoryDisk(Mutex::new(vec![[0; BLOCK_SZ]; 2048])));
    let fs = EasyFileSystem::create(disk.clone(), 2048, 1);
    let root = EasyFileSystem::root_inode(&fs);
    let original = root.create("original").unwrap();
    let inode_id = original.stat().inode_id;
    let data = vec![0xAB; 200 * BLOCK_SZ];
    assert_eq!(original.write_at(0, &data), data.len());
    assert!(root.link("original", "alias"));
    let alias = root.find("alias").unwrap();
    assert!(Arc::ptr_eq(&original, &alias));
    assert_eq!(alias.stat().inode_id, inode_id);
    assert_eq!(alias.stat().nlink, 2);
    assert!(!root.link("original", "original"));
    assert!(!root.link("original", "alias"));
    assert!(!root.link("missing", "other"));
    assert!(!root.link("original", ""));
    assert!(root.create(&"x".repeat(28)).is_none());

    assert!(root.unlink("original"));
    assert!(!root.unlink("original"));
    assert_eq!(alias.stat().nlink, 1);
    assert!(root.unlink("alias"));
    assert_eq!(alias.stat().nlink, 0);
    assert!(root.find("original").is_none());
    assert!(root.find("alias").is_none());
    let mut read_back = vec![0; data.len()];
    assert_eq!(alias.read_at(0, &mut read_back), data.len());
    assert_eq!(read_back, data);

    let replacement = root.create("replacement").unwrap();
    assert_ne!(replacement.stat().inode_id, inode_id);
    drop(original);
    assert_eq!(alias.write_at(0, b"still open"), 10);
    drop(alias);
    let recycled = root.create("recycled").unwrap();
    assert_eq!(recycled.stat().inode_id, inode_id);
    assert_eq!(recycled.read_at(0, &mut read_back), 0);
    assert!(root.unlink("replacement"));
    assert!(root.unlink("recycled"));
    drop(replacement);
    drop(recycled);

    // Exceed both the inode pool and data pool over time without growing the directory.
    for _ in 0..4200 {
        let inode = root.create("cycle").unwrap();
        assert_eq!(inode.write_at(0, b"x"), 1);
        assert!(root.unlink("cycle"));
        drop(inode);
    }
    for _ in 0..8 {
        let inode = root.create("indirect").unwrap();
        assert_eq!(inode.write_at(0, &data), data.len());
        assert!(root.unlink("indirect"));
        drop(inode);
    }

    let persistent = root.create("persistent").unwrap();
    persistent.write_at(0, b"on disk");
    assert!(root.link("persistent", "persistent_alias"));
    drop(persistent);
    let reopened_fs = EasyFileSystem::open(disk);
    let reopened_root = EasyFileSystem::root_inode(&reopened_fs);
    let reopened = reopened_root.find("persistent_alias").unwrap();
    assert_eq!(reopened.stat().nlink, 2);
    assert_eq!(reopened.read_at(0, &mut read_back[..7]), 7);
    assert_eq!(&read_back[..7], b"on disk");
}
