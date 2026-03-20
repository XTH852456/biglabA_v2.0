# QEMU VirtIO-BLK 运行结果总结

## 1. 目标

验证 `tg-rcore-tutorial-ch1-storage` 在 QEMU `virt` 平台上完成最小磁盘读写。

## 2. 关键配置

- 强制 modern virtio-mmio：`-global virtio-mmio.force-legacy=false`
- 挂载块设备：
  - `-drive file=disk.img,if=none,format=raw,id=vd0`
  - `-device virtio-blk-device,drive=vd0`
- 内核侧支持扫描 virtio-mmio 槽位，自动定位 block 设备。

## 3. 运行命令

在 `tg-rcore-tutorial-ch1-storage` 目录执行：

```powershell
cargo run --release
```

## 4. 实际结果

```text
========================================
TG-RCORE CH1 STORAGE DEMO (S-MODE)
========================================
virtio-blk init: ok
capacity: 16384 sectors (8192 KiB)
before first16: 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
write sector 1: ok
after  first16: 54 47 2d 52 43 4f 52 45 20 53 54 4f 52 41 47 45
verify: pass
restore sector 1: ok
storage read/write demo finished.
```

## 5. 结论

`tg-rcore-tutorial-ch1-storage` 已可稳定完成单扇区写入、读回校验和恢复，达到“极简内核可读写磁盘信息”的实验目标。
