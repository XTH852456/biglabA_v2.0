# QEMU 串口无输出问题总结

## 1. 现象
运行下面命令后，QEMU 窗口里看不到预期字符（例如 `u` 或欢迎字符串）：

```bash
qemu-system-riscv64 -machine virt -bios none -kernel target/riscv64gc-unknown-none-elf/release/tg-rcore-tutorial-ch1-uart
```

## 2. 根因分析

### 根因 1：串口输出没有重定向到当前终端
- 在 `virt` 机器下，UART 输出默认走串口。
- 命令里缺少 `-nographic` 时，你盯着图形窗口通常看不到串口文本。

### 根因 2：链接脚本没有稳定生效，镜像加载地址不正确
- 裸机程序需要明确把内核放在 QEMU `virt` 可执行的起始区域。
- 若没有通过 `-Tlink.x` 固定链接行为，可能被默认链接地址影响，导致程序无法按预期执行。

### 根因 3：入口函数没有先初始化栈指针
- 在 `no_std + no_main` 裸机环境，进入 Rust 函数前必须先设置 `sp`。
- 如果直接进入高层函数，第一次函数调用或栈访问就可能异常，表现为“无输出”。

## 3. 解决方案

### 方案 1：在子工程添加 `.cargo/config.toml`
- 固定目标平台：`riscv64gc-unknown-none-elf`
- 固定链接参数：`-Clink-arg=-Tlink.x`
- 固定 runner：`qemu-system-riscv64 -machine virt -nographic -bios none -kernel`

对应文件：
- `tg-rcore-tutorial-ch1-uart/.cargo/config.toml`

### 方案 2：修正链接脚本
- 在 `link.x` 中设置：
  - `ENTRY(_start)`
  - `BASE = 0x80000000`
- 让程序从 QEMU `virt` 下可执行位置启动。

对应文件：
- `tg-rcore-tutorial-ch1-uart/link.x`

### 方案 3：修正启动入口
- 将业务逻辑放到 `kernel_main`。
- 在 `_start`（naked 函数）里先分配静态栈并设置 `sp`，再跳转到 `kernel_main`。

对应文件：
- `tg-rcore-tutorial-ch1-uart/src/main.rs`

## 4. 验证结果
在 `tg-rcore-tutorial-ch1-uart` 目录执行：

```bash
cargo run --release
```

终端已出现串口输出：

```text
========================================
HELLO WORLD FROM S-MODE UART
========================================
```

说明程序已经在 QEMU 中正常执行，UART 输出链路也已打通。

## 5. 建议的日常运行方式
后续优先使用：

```bash
cargo run --release
```

原因：
- 避免每次手敲 QEMU 参数遗漏 `-nographic`
- 避免忘记 `-Tlink.x` 导致链接地址问题
- 运行参数统一，实验结果可复现
