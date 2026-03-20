# tg-rcore-tutorial-storage

`tg-rcore-tutorial-storage` 是一个 `no_std` 的最小 VirtIO-BLK 组件 crate，面向 QEMU `virt` 平台，提供：

- `init() -> Result<u64, StorageError>`：初始化块设备并返回扇区总数。
- `read_sector(sector, &mut [u8; 512])`：读取一个 512B 扇区。
- `write_sector(sector, &[u8; 512])`：写入一个 512B 扇区。

该组件由 `tg-rcore-tutorial-ch1-storage` 调用完成完整实验。
