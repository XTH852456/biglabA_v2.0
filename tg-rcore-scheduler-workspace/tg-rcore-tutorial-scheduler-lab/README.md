# 实验 4：调度算法实验套件

本实验把“单一 scheduler”改造成“可插拔 scheduler 策略插件”，并提供统一的数据采集与量化比较。

## 1. 目标

- 支持 FCFS / SJF / RR / MLFQ / CFS-like（简化）。
- 统一调度器接口：
  - `enqueue(task)`
  - `pick_next()`
  - `on_tick()`
  - `on_block()`
  - `on_wakeup()`
- 统一数据采集：每次上下文切换记录
  - 时间戳
  - 就绪队列长度
  - 运行片段长度
- 输出可比较指标：
  - 平均等待时间
  - 平均周转时间
  - 吞吐量
  - P95/P99 交互延迟
  - 饥饿发生次数

## 2. 目录

```text
tg-rcore-scheduler-workspace/
├── run_experiments.ps1
└── tg-rcore-tutorial-scheduler-lab/
    ├── Cargo.toml
    ├── README.md
    └── src/main.rs
```

## 3. 运行方式

在工作区根目录执行：

```powershell
.\tg-rcore-scheduler-workspace\run_experiments.ps1
```

或在 crate 目录执行：

```powershell
cargo run --release
```

## 4. 实验 workload

内置三组 userland 仿真负载：

- `CPU-bound`：长计算任务，几乎不阻塞
- `IO-bound`：频繁 CPU burst + I/O 阻塞
- `Mixed-interactive`：CPU + I/O + 交互型混合

## 5. 输出结果

程序会输出终端对比表，并生成两个 CSV：

- `experiment_metrics.csv`：算法指标对比
- `context_switches.csv`：上下文切换日志明细

其中 `context_switches.csv` 字段：

- `workload`
- `scheduler`
- `timestamp`
- `from`
- `to`
- `ready_len`
- `run_fragment`

## 6. 验收标准（DoD）

- [ ] 五种算法均可运行
- [ ] 三类 workload 都有输出
- [ ] 能看到平均等待/周转/吞吐/P95/P99/饥饿次数
- [ ] 能导出 `context_switches.csv` 进行复盘
- [ ] 能用脚本一键复现实验

## 7. 可扩展建议

1. 给 SJF 增加 burst 预测（指数平滑）形成 SRTF-like。
2. 给 CFS-like 引入可配置权重和最小调度粒度。
3. 增加多核仿真（每核一个 runqueue）对比全局队列。
4. 导出 markdown 报告，自动附上每组 workload 最优策略。
