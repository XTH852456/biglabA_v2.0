# 调度算法实验套件：运行逻辑与输出含义逐步分析

本文档配合源码 [tg-rcore-scheduler-workspace/tg-rcore-tutorial-scheduler-lab/src/main.rs](tg-rcore-scheduler-workspace/tg-rcore-tutorial-scheduler-lab/src/main.rs) 使用，解释“程序每一步做了什么、会输出什么、输出代表什么”。

## 1. 启动方式

在目录 [tg-rcore-scheduler-workspace/tg-rcore-tutorial-scheduler-lab](tg-rcore-scheduler-workspace/tg-rcore-tutorial-scheduler-lab) 执行：

```powershell
cargo run --release
```

也可以在工作区根目录执行：

```powershell
.\tg-rcore-scheduler-workspace\run_experiments.ps1
```

## 2. 主流程（对应 `main()`）

### Step A：构造 workload 集合

- 代码位置：`workload_suite()`
- 内容：构造 3 类负载
  - `CPU-bound`
  - `IO-bound`
  - `Mixed-interactive`

含义：
- CPU-bound 主要考察吞吐与公平性。
- IO-bound 主要考察阻塞/唤醒后的调度响应。
- Mixed-interactive 主要考察交互延迟与饥饿控制。

### Step B：构造 scheduler 集合

- 代码位置：`scheduler_suite()`
- 内容：构造 5 种策略
  - FCFS
  - SJF
  - RR
  - MLFQ
  - CFS-like

含义：
- 形成固定对照组，便于横向比较。

### Step C：笛卡尔积运行实验

- 代码位置：`for workload in &workloads { for scheduler in scheduler_suite() { ... } }`
- 行为：每个 workload 都运行全部 5 种策略。

含义：
- 总实验数 = 3 × 5 = 15。

### Step D：打印表格与导出 CSV

- 代码位置：`print_table()`、`export_metrics_csv()`、`export_context_switch_csv()`

终端输出两部分：
1. 实验对比表
2. 导出文件提示

## 3. 单个实验如何推进（对应 `run_experiment()`）

每个 tick（离散时间单位）都执行同样的调度循环：

### Step 1：处理唤醒

- 条件：`Blocked && unblock_at == now`
- 动作：任务变为 `Ready`，调用 `on_wakeup()`

含义：
- 模拟 I/O 完成后重新参与竞争 CPU。

### Step 2：处理新到达

- 条件：`New && arrival <= now`
- 动作：任务变为 `Ready`，调用 `enqueue()`

含义：
- 模拟任务进入系统。

### Step 3：CPU 空闲时选下一个任务

- 条件：`running.is_none()`
- 动作：调用 `pick_next()`，并通过 `switch_context()` 记录一次切换

含义：
- 不同策略核心差异就在 `pick_next()`。

### Step 4：统计等待时间与饥饿

- 对所有 `Ready` 任务：`waiting_time += 1`
- 若等待超过阈值（`starvation_threshold`）且未记过，则 `starvation_count += 1`

含义：
- `starvation_count` 用于衡量策略是否会“长期饿死”任务。

### Step 5：推进运行任务 1 tick

对 `running` 任务执行：
- `remaining_cpu -= 1`
- `run_since_io += 1`
- 调用 `on_tick()` 判断是否抢占

并根据结果进入分支：
- 完成：`Running -> Done`
- 触发 I/O：`Running -> Blocked`
- 被抢占：`Running -> Ready`

含义：
- `on_tick()` 是 RR/MLFQ/CFS-like 等策略体现“时间推进决策”的关键接口。

### Step 6：判定实验结束

- 条件：所有任务都 `Done`
- 结束后汇总指标

## 4. 指标公式与含义

### 平均等待时间 `avg_waiting`

- 含义：任务在就绪队列排队的平均时长。
- 越低通常越好（响应更快）。

### 平均周转时间 `avg_turnaround`

- 定义：完成时间 - 到达时间。
- 含义：任务从进入系统到完成的总耗时。

### 吞吐量 `throughput`

- 定义：`任务总数 / makespan`
- 含义：单位时间完成任务数，越高代表整体处理能力越强。

### P95 / P99 交互延迟

- 样本来源：交互任务从 `Ready` 到 `Running` 的延迟。
- 含义：越低表示交互任务更“跟手”。

### 饥饿次数 `starvation_count`

- 定义：等待超过阈值的事件次数。
- 含义：越低越好，0 表示未观测到显著饥饿。

### 上下文切换次数 `context_switches`

- 含义：调度开销的一个近似指标。
- 次数太高通常意味着切换开销更重。

## 5. 终端输出字段逐列解释

表头：

- `workload`：负载名称
- `scheduler`：调度算法名称
- `avg_wait`：平均等待时间
- `avg_turn`：平均周转时间
- `throughput`：吞吐量
- `p95_lat`：交互延迟 95 分位
- `p99_lat`：交互延迟 99 分位
- `starve`：饥饿次数
- `switches`：上下文切换次数
- `makespan`：该实验总耗时（tick）

## 6. CSV 输出说明

### `experiment_metrics.csv`

每行是一个 `(workload, scheduler)` 的聚合结果，可直接用于表格或画图。

### `context_switches.csv`

每行是一条上下文切换事件，字段：
- `timestamp`
- `from`
- `to`
- `ready_len`
- `run_fragment`

它适合做时序复盘，例如：
- 某段时间是否发生频繁抢占
- Ready 队列长度是否持续偏大
- 某策略是否导致碎片化运行片段

## 7. 一次典型输出如何解读

例如 Mixed-interactive 场景中：
- 若某策略 `p95/p99` 很低，说明交互任务响应性好。
- 若 `starve` 为 0，说明没有明显饥饿。
- 若 `switches` 明显偏高，需要结合吞吐判断是否值得。

常见取舍：
- FCFS：切换少，但交互延迟可能高。
- RR：公平性好，但切换开销通常更高。
- MLFQ：兼顾交互响应与吞吐，参数敏感。
- CFS-like：更重公平，延迟和吞吐受权重与粒度影响。

## 8. 交付建议（写实验报告时可直接引用）

建议至少给出：
1. 三个 workload 的 5 算法对照表（可贴终端或 CSV）。
2. 对 Mixed-interactive 的重点讨论（P95/P99 与 starve）。
3. 一段基于 `context_switches.csv` 的时间序列观察结论。
4. 你认为最适合课程场景的策略及理由（目标导向：吞吐/响应/公平）。
