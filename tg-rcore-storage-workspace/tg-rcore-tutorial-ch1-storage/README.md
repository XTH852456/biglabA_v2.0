# tg-rcore-tutorial-ch1-storage 实验指导

本实验在 `tg-rcore-tutorial-ch1` 的最小内核基础上扩展出一个可读写磁盘的内核 crate：`tg-rcore-tutorial-ch1-storage`，并通过组件 crate `tg-rcore-tutorial-storage` 封装了最小 VirtIO-BLK 驱动。

## 1. 实验目标

- 理解裸机内核如何通过 MMIO 与 VirtIO 块设备交互。
- 完成一个最小可用的“扇区写入 + 扇区读取 + 数据校验”闭环。
- 形成可复现的运行流程：`cargo run --release` 一键编译并在 QEMU 中验证。

## 2. 目录结构

```text
tg-rcore-storage-workspace/
├── tg-rcore-tutorial-storage/              # 组件 crate：最小 VirtIO-BLK 驱动
│   ├── Cargo.toml
│   └── src/lib.rs
└── tg-rcore-tutorial-ch1-storage/          # 内核 crate：启动 + 调用 storage 驱动做读写验证
    ├── .cargo/config.toml                  # 交叉编译目标 + QEMU runner（含 virtio-blk 挂盘）
    ├── build.rs                            # 生成链接脚本 + 自动创建 disk.img
    ├── Cargo.toml
    ├── rust-toolchain.toml
    ├── src/main.rs
    └── README.md
```

## 3. 环境准备

### 3.1 Rust 目标

```powershell
rustup target add riscv64gc-unknown-none-elf
```

### 3.2 QEMU

要求安装 `qemu-system-riscv64`。

验证命令：

```powershell
qemu-system-riscv64 --version
```

## 4. 运行步骤

在 `tg-rcore-tutorial-ch1-storage` 目录执行：

```powershell
cargo run --release
```

说明：

- `.cargo/config.toml` 已固定 `riscv64gc-unknown-none-elf`。
- `runner` 已固定 QEMU 参数，并挂载 `disk.img` 为 virtio-blk 设备。
- `build.rs` 会在首次构建时自动创建 `disk.img`（8 MiB）。

## 5. 预期现象

终端可看到类似输出（示例）：

```text
========================================
TG-RCORE CH1 STORAGE DEMO (S-MODE)
========================================
virtio-blk init: ok
capacity: 16384 sectors (8192 KiB)
before first16: 00 00 00 ...
write sector 1: ok
after  first16: 54 47 2d ...
verify: pass
restore sector 1: ok
storage read/write demo finished.
```

若最后为 `verify: pass` 且 QEMU 正常退出，说明最小读写链路打通。

## 6. 核心原理说明

### 6.1 `tg-rcore-tutorial-storage`（组件 crate）

- 设备基址：`0x1000_1000`（QEMU `virt` 平台首个 virtio-mmio 槽位）。
- 初始化流程：
  - 检查 `magic/version/device_id`。
  - 状态机按 `ACKNOWLEDGE -> DRIVER -> FEATURES_OK -> DRIVER_OK` 推进。
  - 建立 queue 0 的描述符表、avail ring、used ring。
- I/O 流程（同步轮询）：
  - 构造 3 段描述符链：请求头、数据区、状态字节。
  - 投递到 avail ring 并 `QUEUE_NOTIFY`。
  - 轮询 used ring，完成后检查状态码。

### 6.2 `tg-rcore-tutorial-ch1-storage`（内核 crate）

- 启动：沿用 ch1 风格 `_start`，手动设栈后跳转 `rust_main`。
- 验证：
  - 读取 sector 1 原内容。
  - 写入测试模式数据。
  - 重新读取并逐字节比较。
  - 成功后恢复原始数据，保持实验可重复。

## 7. DoD（完成判据）

- [ ] 能执行 `cargo run --release` 并看到 `virtio-blk init: ok`
- [ ] 能看到 `verify: pass`
- [ ] 能解释 queue 三段描述符链的作用
- [ ] 能说明为何使用 sector 1 而非覆盖关键启动区域

## 8. 常见问题

### 8.1 QEMU 无输出

优先检查：

- 是否缺少 `-nographic`
- 是否忘记通过 `.cargo/config.toml` 启动
- 是否在正确目录执行 `cargo run --release`

### 8.2 设备初始化失败

优先检查：

- QEMU 是否带了块设备参数：
  - `-drive file=disk.img,if=none,format=raw,id=vd0`
  - `-device virtio-blk-device,drive=vd0`
- 是否误改了 VirtIO MMIO 基址 `0x1000_1000`

### 8.3 verify 失败

优先检查：

- 描述符 flags：读请求的数据描述符必须带 `WRITE` 标志。
- 请求类型：读为 `VIRTIO_BLK_T_IN`，写为 `VIRTIO_BLK_T_OUT`。
- used ring 轮询是否正确更新 `last_used_idx`。

## 9. 建议扩展练习

1. 增加多扇区连续读写接口（例如一次 4 个扇区）。
2. 引入简单文件头结构，在固定扇区存储元数据。
3. 在 trap 中加入超时机制，避免设备异常时无限轮询。
