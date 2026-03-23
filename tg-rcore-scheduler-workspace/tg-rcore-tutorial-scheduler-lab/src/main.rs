use std::collections::VecDeque;
use std::fs;

// 任务类别：用于构造不同 workload，并在 CFS-like 中映射到不同权重。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TaskClass {
    CpuBound,
    IoBound,
    Interactive,
}

// 任务状态机：模拟一个简化内核调度生命周期。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TaskState {
    New,
    Ready,
    Running,
    Blocked,
    Done,
}

// 静态任务描述（实验输入）。
// 说明：一个 TaskSpec 对应“用户态的一个任务画像”。
#[derive(Clone, Debug)]
struct TaskSpec {
    class: TaskClass,
    arrival: u64,
    total_cpu: u64,
    io_every: Option<u64>,
    io_block: u64,
}

// 运行时任务状态（实验内部状态）。
// 说明：相比 TaskSpec，RuntimeTask 额外维护调度过程中演化的数据。
#[derive(Clone, Debug)]
struct RuntimeTask {
    class: TaskClass,
    arrival: u64,
    remaining_cpu: u64,
    io_every: Option<u64>,
    io_block: u64,
    run_since_io: u64,
    state: TaskState,
    unblock_at: Option<u64>,
    ready_since: Option<u64>,
    waiting_time: u64,
    completion_time: Option<u64>,
    vruntime: f64,
    mlfq_level: usize,
    starvation_marked: bool,
}

impl RuntimeTask {
    // 将静态任务描述转换为可调度的运行时任务。
    fn from_spec(spec: &TaskSpec) -> Self {
        Self {
            class: spec.class,
            arrival: spec.arrival,
            remaining_cpu: spec.total_cpu,
            io_every: spec.io_every,
            io_block: spec.io_block,
            run_since_io: 0,
            state: TaskState::New,
            unblock_at: None,
            ready_since: None,
            waiting_time: 0,
            completion_time: None,
            vruntime: 0.0,
            mlfq_level: 0,
            starvation_marked: false,
        }
    }
}

// on_tick 的返回动作：继续运行或触发抢占。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TickAction {
    Continue,
    Preempt,
}

// 调度策略统一接口：
// - enqueue：任务进入就绪队列
// - pick_next：选择下一运行任务
// - on_tick：每个时钟 tick 的调度更新
// - on_block：任务因 I/O 阻塞时的通知
// - on_wakeup：任务被唤醒时的通知
// - ready_len：用于观测“当前调度策略看到的就绪长度”
trait Scheduler {
    fn name(&self) -> &'static str;
    fn enqueue(&mut self, task_id: usize, tasks: &mut [RuntimeTask], now: u64);
    fn pick_next(&mut self, tasks: &mut [RuntimeTask], now: u64) -> Option<usize>;
    fn on_tick(&mut self, running: usize, tasks: &mut [RuntimeTask], now: u64) -> TickAction;
    fn on_block(&mut self, task_id: usize, tasks: &mut [RuntimeTask], now: u64);
    fn on_wakeup(&mut self, task_id: usize, tasks: &mut [RuntimeTask], now: u64);
    fn ready_len(&self) -> usize;
}

// FCFS（先来先服务）：非抢占、按到达顺序出队。
struct FcfsScheduler {
    queue: VecDeque<usize>,
}

impl FcfsScheduler {
    fn new() -> Self {
        Self {
            queue: VecDeque::new(),
        }
    }
}

impl Scheduler for FcfsScheduler {
    fn name(&self) -> &'static str {
        "FCFS"
    }

    fn enqueue(&mut self, task_id: usize, _tasks: &mut [RuntimeTask], _now: u64) {
        // FCFS 规则：新就绪任务总是排在队尾。
        self.queue.push_back(task_id);
    }

    fn pick_next(&mut self, _tasks: &mut [RuntimeTask], _now: u64) -> Option<usize> {
        // FCFS 规则：CPU 空闲时取队首任务执行（先到先服务）。
        self.queue.pop_front()
    }

    fn on_tick(&mut self, _running: usize, _tasks: &mut [RuntimeTask], _now: u64) -> TickAction {
        // FCFS 非抢占：tick 到来不主动让出 CPU。
        TickAction::Continue
    }

    fn on_block(&mut self, _task_id: usize, _tasks: &mut [RuntimeTask], _now: u64) {}

    fn on_wakeup(&mut self, task_id: usize, tasks: &mut [RuntimeTask], now: u64) {
        self.enqueue(task_id, tasks, now);
    }

    fn ready_len(&self) -> usize {
        self.queue.len()
    }
}

// SJF（短作业优先，非抢占版本）：每次从就绪集合选 remaining_cpu 最小任务。
struct SjfScheduler {
    ready: Vec<usize>,
}

impl SjfScheduler {
    fn new() -> Self {
        Self { ready: Vec::new() }
    }
}

impl Scheduler for SjfScheduler {
    fn name(&self) -> &'static str {
        "SJF"
    }

    fn enqueue(&mut self, task_id: usize, _tasks: &mut [RuntimeTask], _now: u64) {
        // SJF 先放入就绪集合；真正排序/选择在 pick_next 时完成。
        self.ready.push(task_id);
    }

    fn pick_next(&mut self, tasks: &mut [RuntimeTask], _now: u64) -> Option<usize> {
        if self.ready.is_empty() {
            return None;
        }
        // 线性扫描就绪集合，找到 remaining_cpu 最小的任务。
        let mut best_idx = 0usize;
        let mut best_remain = tasks[self.ready[0]].remaining_cpu;
        for (i, &tid) in self.ready.iter().enumerate().skip(1) {
            let remain = tasks[tid].remaining_cpu;
            if remain < best_remain {
                best_remain = remain;
                best_idx = i;
            }
        }
        // 用 swap_remove O(1) 删除被选中的任务。
        Some(self.ready.swap_remove(best_idx))
    }

    fn on_tick(&mut self, _running: usize, _tasks: &mut [RuntimeTask], _now: u64) -> TickAction {
        TickAction::Continue
    }

    fn on_block(&mut self, _task_id: usize, _tasks: &mut [RuntimeTask], _now: u64) {}

    fn on_wakeup(&mut self, task_id: usize, tasks: &mut [RuntimeTask], now: u64) {
        self.enqueue(task_id, tasks, now);
    }

    fn ready_len(&self) -> usize {
        self.ready.len()
    }
}

// RR（时间片轮转）：达到 quantum 后触发抢占。
struct RrScheduler {
    queue: VecDeque<usize>,
    quantum: u64,
    running: Option<usize>,
    used_slice: u64,
}

impl RrScheduler {
    fn new(quantum: u64) -> Self {
        Self {
            queue: VecDeque::new(),
            quantum,
            running: None,
            used_slice: 0,
        }
    }
}

impl Scheduler for RrScheduler {
    fn name(&self) -> &'static str {
        "RR"
    }

    fn enqueue(&mut self, task_id: usize, _tasks: &mut [RuntimeTask], _now: u64) {
        // RR 规则：回到队尾，等待下一个轮次。
        self.queue.push_back(task_id);
    }

    fn pick_next(&mut self, _tasks: &mut [RuntimeTask], _now: u64) -> Option<usize> {
        // 取队首任务进入运行态，并重置已使用时间片计数。
        let next = self.queue.pop_front();
        self.running = next;
        self.used_slice = 0;
        next
    }

    fn on_tick(&mut self, running: usize, _tasks: &mut [RuntimeTask], _now: u64) -> TickAction {
        // 防御性同步：若 running 不一致，说明发生过切换，重置片计数。
        if self.running != Some(running) {
            self.running = Some(running);
            self.used_slice = 0;
        }
        // 每经过一个 tick，时间片消耗 +1。
        self.used_slice += 1;
        if self.used_slice >= self.quantum {
            // 时间片耗尽：请求抢占，让任务回就绪队列。
            self.running = None;
            self.used_slice = 0;
            TickAction::Preempt
        } else {
            TickAction::Continue
        }
    }

    fn on_block(&mut self, task_id: usize, _tasks: &mut [RuntimeTask], _now: u64) {
        if self.running == Some(task_id) {
            self.running = None;
            self.used_slice = 0;
        }
    }

    fn on_wakeup(&mut self, task_id: usize, tasks: &mut [RuntimeTask], now: u64) {
        self.enqueue(task_id, tasks, now);
    }

    fn ready_len(&self) -> usize {
        self.queue.len()
    }
}

// MLFQ（多级反馈队列）：
// - 多级队列 + 不同时间片
// - 用完时间片会降级
// - 周期性 boost 把等待太久的任务提升，缓解饥饿
struct MlfqScheduler {
    queues: Vec<VecDeque<usize>>,
    quantums: Vec<u64>,
    boost_interval: u64,
    next_boost_at: u64,
    running: Option<usize>,
    used_slice: u64,
}

impl MlfqScheduler {
    fn new() -> Self {
        Self {
            queues: vec![VecDeque::new(), VecDeque::new(), VecDeque::new(), VecDeque::new()],
            quantums: vec![2, 4, 8, 16],
            boost_interval: 60,
            next_boost_at: 60,
            running: None,
            used_slice: 0,
        }
    }

    fn push_level(&mut self, task_id: usize, level: usize) {
        // level 做边界保护，防止越界写入队列。
        let level = level.min(self.queues.len() - 1);
        self.queues[level].push_back(task_id);
    }

    fn boost_all(&mut self, tasks: &mut [RuntimeTask]) {
        // 先把所有层级任务抽平，避免在遍历中借用冲突。
        let mut all_tasks = Vec::new();
        for q in &mut self.queues {
            while let Some(tid) = q.pop_front() {
                all_tasks.push(tid);
            }
        }
        // 全部任务提升到最高优先级（0 级）。
        for tid in all_tasks {
            tasks[tid].mlfq_level = 0;
            self.queues[0].push_back(tid);
        }
    }
}

impl Scheduler for MlfqScheduler {
    fn name(&self) -> &'static str {
        "MLFQ"
    }

    fn enqueue(&mut self, task_id: usize, tasks: &mut [RuntimeTask], _now: u64) {
        // MLFQ 入队：按任务当前 level 放入对应就绪队列。
        let level = tasks[task_id].mlfq_level;
        self.push_level(task_id, level);
    }

    fn pick_next(&mut self, tasks: &mut [RuntimeTask], _now: u64) -> Option<usize> {
        // 从高优先级到低优先级依次找第一个可运行任务。
        for (level, q) in self.queues.iter_mut().enumerate() {
            if let Some(tid) = q.pop_front() {
                // 被选中后记录其当前层级并重置片计数。
                tasks[tid].mlfq_level = level;
                self.running = Some(tid);
                self.used_slice = 0;
                return Some(tid);
            }
        }
        self.running = None;
        None
    }

    fn on_tick(&mut self, running: usize, tasks: &mut [RuntimeTask], now: u64) -> TickAction {
        // 到达全局 boost 时刻：提升所有等待任务优先级，缓解饥饿。
        if now >= self.next_boost_at {
            self.boost_all(tasks);
            self.next_boost_at = now + self.boost_interval;
        }

        // 防御性同步当前运行任务。
        if self.running != Some(running) {
            self.running = Some(running);
            self.used_slice = 0;
        }

        // 片计数 +1，并读取当前层级片长。
        self.used_slice += 1;
        let level = tasks[running].mlfq_level;
        let quantum = self.quantums[level];

        if self.used_slice >= quantum {
            // 片耗尽：任务降一级（除最低级外），然后请求抢占。
            self.used_slice = 0;
            self.running = None;
            if level + 1 < self.queues.len() {
                tasks[running].mlfq_level = level + 1;
            }
            TickAction::Preempt
        } else {
            TickAction::Continue
        }
    }

    fn on_block(&mut self, task_id: usize, _tasks: &mut [RuntimeTask], _now: u64) {
        if self.running == Some(task_id) {
            self.running = None;
            self.used_slice = 0;
        }
    }

    fn on_wakeup(&mut self, task_id: usize, tasks: &mut [RuntimeTask], now: u64) {
        // I/O 返回的任务通常更“交互友好”：唤醒时给予一级提升。
        let level = tasks[task_id].mlfq_level;
        if level > 0 {
            tasks[task_id].mlfq_level = level - 1;
        }
        self.enqueue(task_id, tasks, now);
    }

    fn ready_len(&self) -> usize {
        self.queues.iter().map(VecDeque::len).sum()
    }
}

// CFS-like（简化版）：
// - 维护 vruntime
// - 每次选择 vruntime 最小的任务
// - 若当前任务明显“跑多了”，触发抢占
struct CfsLikeScheduler {
    ready: Vec<usize>,
    running: Option<usize>,
    used_slice: u64,
    min_granularity: u64,
    target_slice: u64,
}

impl CfsLikeScheduler {
    fn new() -> Self {
        Self {
            ready: Vec::new(),
            running: None,
            used_slice: 0,
            min_granularity: 2,
            target_slice: 6,
        }
    }

    fn class_weight(class: TaskClass) -> f64 {
        // 权重越小，vruntime 增长越慢，越容易被再次调度。
        match class {
            TaskClass::Interactive => 0.85,
            TaskClass::IoBound => 1.0,
            TaskClass::CpuBound => 1.15,
        }
    }

    fn min_ready_vruntime(&self, tasks: &[RuntimeTask]) -> Option<f64> {
        // 扫描就绪集合，找最小 vruntime 作为公平基线。
        let mut best = None;
        for &tid in &self.ready {
            let v = tasks[tid].vruntime;
            best = match best {
                None => Some(v),
                Some(cur) => Some(cur.min(v)),
            };
        }
        best
    }
}

impl Scheduler for CfsLikeScheduler {
    fn name(&self) -> &'static str {
        "CFS-like"
    }

    fn enqueue(&mut self, task_id: usize, _tasks: &mut [RuntimeTask], _now: u64) {
        // 简化实现：就绪集合用 Vec 保存，挑选时线性找最小 vruntime。
        self.ready.push(task_id);
    }

    fn pick_next(&mut self, tasks: &mut [RuntimeTask], _now: u64) -> Option<usize> {
        if self.ready.is_empty() {
            self.running = None;
            return None;
        }

        // 从 ready 中选择 vruntime 最小者。
        let mut best_idx = 0usize;
        let mut best_vr = tasks[self.ready[0]].vruntime;
        for (i, &tid) in self.ready.iter().enumerate().skip(1) {
            let vr = tasks[tid].vruntime;
            if vr < best_vr {
                best_vr = vr;
                best_idx = i;
            }
        }

        // 选中后移出 ready，并初始化当前运行片统计。
        let tid = self.ready.swap_remove(best_idx);
        self.running = Some(tid);
        self.used_slice = 0;
        Some(tid)
    }

    fn on_tick(&mut self, running: usize, tasks: &mut [RuntimeTask], _now: u64) -> TickAction {
        if self.running != Some(running) {
            self.running = Some(running);
            self.used_slice = 0;
        }

        // 核心：每 tick 增加 vruntime，增量受任务类别权重影响。
        let weight = Self::class_weight(tasks[running].class);
        tasks[running].vruntime += weight;

        // 动态时间片：ready 越多，单任务片越短，但不少于最小粒度。
        self.used_slice += 1;
        let ready_count = self.ready.len().max(1) as u64;
        let dynamic_slice = (self.target_slice / ready_count).max(self.min_granularity);

        // 片还没用满，继续运行当前任务。
        if self.used_slice < dynamic_slice {
            return TickAction::Continue;
        }

        // 用满后比较公平基线：若当前明显“跑超前”，触发抢占。
        if let Some(min_ready) = self.min_ready_vruntime(tasks) {
            if min_ready + 0.5 < tasks[running].vruntime {
                self.used_slice = 0;
                self.running = None;
                return TickAction::Preempt;
            }
        }

        TickAction::Continue
    }

    fn on_block(&mut self, task_id: usize, _tasks: &mut [RuntimeTask], _now: u64) {
        if self.running == Some(task_id) {
            self.running = None;
            self.used_slice = 0;
        }
    }

    fn on_wakeup(&mut self, task_id: usize, tasks: &mut [RuntimeTask], now: u64) {
        self.enqueue(task_id, tasks, now);
    }

    fn ready_len(&self) -> usize {
        self.ready.len()
    }
}

// 一组实验负载（一个 workload 包含多个任务）。
#[derive(Clone)]
struct Workload {
    name: &'static str,
    tasks: Vec<TaskSpec>,
    starvation_threshold: u64,
}

// 一次上下文切换观测事件。
// run_fragment 表示“from 任务这次连续运行了多久”。
#[derive(Clone, Debug)]
struct ContextSwitchEvent {
    ts: u64,
    from: Option<usize>,
    to: Option<usize>,
    ready_len: usize,
    run_fragment: u64,
}

// 一次实验结果指标。
#[derive(Clone, Debug)]
struct ExperimentMetrics {
    avg_waiting: f64,
    avg_turnaround: f64,
    throughput: f64,
    p95_latency: f64,
    p99_latency: f64,
    starvation_count: u64,
    makespan: u64,
    context_switches: usize,
}

// 单个（workload, scheduler）组合的完整结果。
#[derive(Clone)]
struct ExperimentResult {
    scheduler: String,
    workload: String,
    metrics: ExperimentMetrics,
    events: Vec<ContextSwitchEvent>,
}

// 计算百分位（用于交互延迟 P95/P99）。
fn percentile(data: &mut [u64], p: f64) -> f64 {
    if data.is_empty() {
        return 0.0;
    }
    // 先升序排序，再按百分位位置取样。
    data.sort_unstable();
    let idx = ((data.len() as f64 - 1.0) * p).round() as usize;
    data[idx] as f64
}

// 上下文切换统一函数：
// 1) 记录事件
// 2) 更新 running 与 run_start
// 3) 对新运行任务补充“交互延迟统计”等状态更新
fn switch_context(
    running: &mut Option<usize>,
    run_start: &mut u64,
    next: Option<usize>,
    ts: u64,
    tasks: &mut [RuntimeTask],
    ready_len: usize,
    events: &mut Vec<ContextSwitchEvent>,
    interactive_latencies: &mut Vec<u64>,
) {
    if *running == next {
        // from==to 时不算一次切换，直接返回。
        return;
    }

    let run_fragment = if running.is_some() {
        ts.saturating_sub(*run_start)
    } else {
        0
    };

    events.push(ContextSwitchEvent {
        ts,
        from: *running,
        to: next,
        ready_len,
        run_fragment,
    });

    // 切换当前运行任务。
    *running = next;
    *run_start = ts;

    if let Some(tid) = *running {
        // 交互任务延迟定义：从进入 READY 到真正被调度运行的时间。
        if tasks[tid].class == TaskClass::Interactive {
            if let Some(ready_since) = tasks[tid].ready_since {
                interactive_latencies.push(ts.saturating_sub(ready_since));
            }
        }
        tasks[tid].ready_since = None;
        tasks[tid].starvation_marked = false;
        tasks[tid].state = TaskState::Running;
    }
}

// 运行一个实验组合（一个 workload + 一种 scheduler）。
// 这是整个仿真内核：按 tick 推进状态机，收集指标与上下文切换日志。
fn run_experiment(workload: &Workload, mut scheduler: Box<dyn Scheduler>) -> ExperimentResult {
    // 初始化任务运行态数组。
    let mut tasks: Vec<RuntimeTask> = workload.tasks.iter().map(RuntimeTask::from_spec).collect();
    // 当前运行任务（None 表示 CPU 空闲）。
    let mut running: Option<usize> = None;
    // 当前运行片段起点，用于计算 run_fragment。
    let mut run_start: u64 = 0;
    // 全局仿真时钟（tick）。
    let mut now: u64 = 0;
    // 饥饿事件计数。
    let mut starvation_count: u64 = 0;
    // 仅收集交互任务 Ready->Running 延迟样本。
    let mut interactive_latencies: Vec<u64> = Vec::new();
    // 上下文切换事件日志。
    let mut events: Vec<ContextSwitchEvent> = Vec::new();

    let max_ticks = 500_000u64;

    // 离散时间仿真主循环。
    while now < max_ticks {
        // Step 1: 处理 I/O 唤醒（Blocked -> Ready）。
        let mut wake_list = Vec::new();
        for tid in 0..tasks.len() {
            if tasks[tid].state == TaskState::Blocked && tasks[tid].unblock_at == Some(now) {
                wake_list.push(tid);
            }
        }
            // 将本 tick 被唤醒任务批量入队，避免遍历时可变借用冲突。
        for tid in wake_list {
            tasks[tid].unblock_at = None;
            tasks[tid].state = TaskState::Ready;
            tasks[tid].ready_since = Some(now);
            scheduler.on_wakeup(tid, &mut tasks, now);
        }

        // Step 2: 处理到达（New -> Ready）。
        let mut arrival_list = Vec::new();
        for tid in 0..tasks.len() {
            if tasks[tid].state == TaskState::New && tasks[tid].arrival <= now {
                arrival_list.push(tid);
            }
        }
            // 将本 tick 新到达任务批量入队。
        for tid in arrival_list {
            tasks[tid].state = TaskState::Ready;
            tasks[tid].ready_since = Some(now);
            scheduler.enqueue(tid, &mut tasks, now);
        }

        // Step 3: CPU 空闲时，挑选下一任务并记录一次上下文切换。
        if running.is_none() {
            if let Some(next) = scheduler.pick_next(&mut tasks, now) {
                // 记录切换时就绪队列长度（便于后续分析拥塞）。
                let ready_len = scheduler.ready_len();
                switch_context(
                    &mut running,
                    &mut run_start,
                    Some(next),
                    now,
                    &mut tasks,
                    ready_len,
                    &mut events,
                    &mut interactive_latencies,
                );
            }
        }

        // Step 4: 统计 READY 态等待时间，并检测饥饿。
        for task in &mut tasks {
            if task.state == TaskState::Ready {
                // Ready 态每过 1 tick，累计等待 +1。
                task.waiting_time += 1;
                if let Some(ready_since) = task.ready_since {
                    let wait_span = now.saturating_sub(ready_since);
                    // 超阈值且尚未标记时，记录一次饥饿事件。
                    if wait_span > workload.starvation_threshold && !task.starvation_marked {
                        starvation_count += 1;
                        task.starvation_marked = true;
                    }
                }
            }
        }

        // Step 5: 推进当前运行任务 1 tick，并根据事件触发状态变迁。
        if let Some(tid) = running {
            // 消耗 1 tick CPU 时间。
            if tasks[tid].remaining_cpu > 0 {
                tasks[tid].remaining_cpu -= 1;
            }
            // 统计距离上一次 I/O 的 CPU burst 长度。
            tasks[tid].run_since_io += 1;

            // 让策略处理 tick：可能返回 Continue，也可能要求抢占。
            let action = scheduler.on_tick(tid, &mut tasks, now);
            // 状态变迁统一发生在“下一个时刻边界”。
            let ts_next = now + 1;

            // 5.1 任务执行完成：Running -> Done。
            if tasks[tid].remaining_cpu == 0 {
                tasks[tid].state = TaskState::Done;
                tasks[tid].completion_time = Some(ts_next);
                // 任务完成时，把 CPU 置空并记录一次切换。
                let ready_len = scheduler.ready_len();
                switch_context(
                    &mut running,
                    &mut run_start,
                    None,
                    ts_next,
                    &mut tasks,
                    ready_len,
                    &mut events,
                    &mut interactive_latencies,
                );
            } else if let Some(io_every) = tasks[tid].io_every {
                // 5.2 到达 I/O burst：Running -> Blocked。
                if io_every > 0 && tasks[tid].run_since_io >= io_every {
                    // 当前 burst 达到 I/O 触发阈值：进入阻塞。
                    tasks[tid].run_since_io = 0;
                    tasks[tid].state = TaskState::Blocked;
                    // 记录预计唤醒时刻。
                    tasks[tid].unblock_at = Some(ts_next + tasks[tid].io_block);
                    scheduler.on_block(tid, &mut tasks, ts_next);
                    let ready_len = scheduler.ready_len();
                    switch_context(
                        &mut running,
                        &mut run_start,
                        None,
                        ts_next,
                        &mut tasks,
                        ready_len,
                        &mut events,
                        &mut interactive_latencies,
                    );
                } else if action == TickAction::Preempt {
                    // 5.3 被策略抢占：Running -> Ready。
                    tasks[tid].state = TaskState::Ready;
                    // 抢占返回 Ready 时刷新 ready_since，方便统计延迟/饥饿。
                    tasks[tid].ready_since = Some(ts_next);
                    scheduler.enqueue(tid, &mut tasks, ts_next);
                    let ready_len = scheduler.ready_len();
                    switch_context(
                        &mut running,
                        &mut run_start,
                        None,
                        ts_next,
                        &mut tasks,
                        ready_len,
                        &mut events,
                        &mut interactive_latencies,
                    );
                }
            } else if action == TickAction::Preempt {
                // 5.4 纯 CPU 任务被抢占：Running -> Ready。
                tasks[tid].state = TaskState::Ready;
                tasks[tid].ready_since = Some(ts_next);
                scheduler.enqueue(tid, &mut tasks, ts_next);
                let ready_len = scheduler.ready_len();
                switch_context(
                    &mut running,
                    &mut run_start,
                    None,
                    ts_next,
                    &mut tasks,
                    ready_len,
                    &mut events,
                    &mut interactive_latencies,
                );
            }
        }

        // Step 6: 全部任务结束则退出仿真。
        if tasks.iter().all(|t| t.state == TaskState::Done) {
            now += 1;
            break;
        }

        now += 1;
    }

    // 汇总核心指标。
    // completed 做 max(1) 防止除零（理论上不会为 0，这里做健壮性保护）。
    let completed = tasks.iter().filter(|t| t.completion_time.is_some()).count().max(1) as f64;
    // 总等待 = 所有任务在 Ready 状态累计时间之和。
    let total_wait: u64 = tasks.iter().map(|t| t.waiting_time).sum();
    // 总周转 = 每个任务(完成时刻 - 到达时刻)之和。
    let total_turnaround: u64 = tasks
        .iter()
        .map(|t| t.completion_time.unwrap_or(now).saturating_sub(t.arrival))
        .sum();

    // 克隆样本再计算百分位，避免打乱原始样本顺序。
    let mut pbuf = interactive_latencies.clone();
    let p95 = percentile(&mut pbuf, 0.95);
    let p99 = percentile(&mut pbuf, 0.99);

    let metrics = ExperimentMetrics {
        avg_waiting: total_wait as f64 / completed,
        avg_turnaround: total_turnaround as f64 / completed,
        // 吞吐量 = 完成任务数 / 总耗时。
        throughput: (tasks.len() as f64) / (now.max(1) as f64),
        p95_latency: p95,
        p99_latency: p99,
        starvation_count,
        makespan: now,
        context_switches: events.len(),
    };

    // 返回“可打印 + 可导出”的统一结构。
    ExperimentResult {
        scheduler: scheduler.name().to_string(),
        workload: workload.name.to_string(),
        metrics,
        events,
    }
}

// 预置 workload 套件（CPU / IO / 混合交互）。
fn workload_suite() -> Vec<Workload> {
    vec![
        Workload {
            name: "CPU-bound",
            // CPU-heavy 场景：阈值相对宽松，重点看吞吐与等待。
            starvation_threshold: 120,
            tasks: vec![
                TaskSpec { class: TaskClass::CpuBound, arrival: 0, total_cpu: 140, io_every: None, io_block: 0 },
                TaskSpec { class: TaskClass::CpuBound, arrival: 3, total_cpu: 110, io_every: None, io_block: 0 },
                TaskSpec { class: TaskClass::CpuBound, arrival: 6, total_cpu: 90, io_every: None, io_block: 0 },
                TaskSpec { class: TaskClass::CpuBound, arrival: 10, total_cpu: 100, io_every: None, io_block: 0 },
            ],
        },
        Workload {
            name: "IO-bound",
            // I/O 密集场景：频繁阻塞/唤醒，重点看唤醒后响应。
            starvation_threshold: 100,
            tasks: vec![
                TaskSpec { class: TaskClass::IoBound, arrival: 0, total_cpu: 90, io_every: Some(4), io_block: 6 },
                TaskSpec { class: TaskClass::IoBound, arrival: 1, total_cpu: 80, io_every: Some(5), io_block: 5 },
                TaskSpec { class: TaskClass::IoBound, arrival: 2, total_cpu: 70, io_every: Some(4), io_block: 7 },
                TaskSpec { class: TaskClass::IoBound, arrival: 5, total_cpu: 75, io_every: Some(3), io_block: 6 },
                TaskSpec { class: TaskClass::IoBound, arrival: 7, total_cpu: 60, io_every: Some(4), io_block: 8 },
            ],
        },
        Workload {
            name: "Mixed-interactive",
            // 混合交互场景：重点关注 P95/P99 和饥饿次数。
            starvation_threshold: 80,
            tasks: vec![
                TaskSpec { class: TaskClass::CpuBound, arrival: 0, total_cpu: 120, io_every: None, io_block: 0 },
                TaskSpec { class: TaskClass::CpuBound, arrival: 8, total_cpu: 95, io_every: None, io_block: 0 },
                TaskSpec { class: TaskClass::IoBound, arrival: 2, total_cpu: 80, io_every: Some(4), io_block: 6 },
                TaskSpec { class: TaskClass::IoBound, arrival: 3, total_cpu: 75, io_every: Some(5), io_block: 5 },
                TaskSpec { class: TaskClass::Interactive, arrival: 0, total_cpu: 65, io_every: Some(2), io_block: 4 },
                TaskSpec { class: TaskClass::Interactive, arrival: 1, total_cpu: 60, io_every: Some(2), io_block: 4 },
                TaskSpec { class: TaskClass::Interactive, arrival: 5, total_cpu: 55, io_every: Some(3), io_block: 3 },
                TaskSpec { class: TaskClass::Interactive, arrival: 9, total_cpu: 50, io_every: Some(2), io_block: 5 },
            ],
        },
    ]
}

// 预置调度策略套件。
fn scheduler_suite() -> Vec<Box<dyn Scheduler>> {
    // 默认实验参数：
    // - RR quantum=4
    // - MLFQ 使用 4 级队列
    // - CFS-like 使用 min_granularity=2, target_slice=6
    vec![
        Box::new(FcfsScheduler::new()),
        Box::new(SjfScheduler::new()),
        Box::new(RrScheduler::new(4)),
        Box::new(MlfqScheduler::new()),
        Box::new(CfsLikeScheduler::new()),
    ]
}

// 终端对比表：核心展示入口。
fn print_table(results: &[ExperimentResult]) {
    // 统一列宽，便于不同算法输出对齐对比。
    println!(
        "{:<18} {:<14} {:>10} {:>12} {:>12} {:>10} {:>8} {:>8} {:>9} {:>10}",
        "workload",
        "scheduler",
        "avg_wait",
        "avg_turn",
        "throughput",
        "p95_lat",
        "p99_lat",
        "starve",
        "switches",
        "makespan"
    );
    println!("{}", "-".repeat(120));

    // 每条结果一行：一个 workload + 一个 scheduler。
    for r in results {
        println!(
            "{:<18} {:<14} {:>10.2} {:>12.2} {:>12.4} {:>10.2} {:>8.2} {:>8} {:>9} {:>10}",
            r.workload,
            r.scheduler,
            r.metrics.avg_waiting,
            r.metrics.avg_turnaround,
            r.metrics.throughput,
            r.metrics.p95_latency,
            r.metrics.p99_latency,
            r.metrics.starvation_count,
            r.metrics.context_switches,
            r.metrics.makespan,
        );
    }
}

// 导出“指标级”CSV，便于后续画图和汇总。
fn export_metrics_csv(results: &[ExperimentResult]) {
    // 首行写 CSV 表头，后续每行写一条聚合指标记录。
    let mut rows = String::from("workload,scheduler,avg_waiting,avg_turnaround,throughput,p95_latency,p99_latency,starvation_count,context_switches,makespan\n");
    for r in results {
        rows.push_str(&format!(
            "{},{},{:.4},{:.4},{:.6},{:.4},{:.4},{},{},{}\n",
            r.workload,
            r.scheduler,
            r.metrics.avg_waiting,
            r.metrics.avg_turnaround,
            r.metrics.throughput,
            r.metrics.p95_latency,
            r.metrics.p99_latency,
            r.metrics.starvation_count,
            r.metrics.context_switches,
            r.metrics.makespan,
        ));
    }
    fs::write("experiment_metrics.csv", rows).expect("write experiment_metrics.csv");
}

// 导出“上下文切换事件级”CSV，便于做时间序列复盘。
fn export_context_switch_csv(results: &[ExperimentResult]) {
    // 事件级日志：后续可做时间序列可视化。
    let mut rows = String::from("workload,scheduler,timestamp,from,to,ready_len,run_fragment\n");
    for r in results {
        for ev in &r.events {
            // None 用 '-' 表示，便于 CSV 查看。
            let from = ev.from.map_or(String::from("-"), |v| v.to_string());
            let to = ev.to.map_or(String::from("-"), |v| v.to_string());
            rows.push_str(&format!(
                "{},{},{},{},{},{},{}\n",
                r.workload, r.scheduler, ev.ts, from, to, ev.ready_len, ev.run_fragment
            ));
        }
    }
    fs::write("context_switches.csv", rows).expect("write context_switches.csv");
}

// 主函数运行流程：
// 1) 组合 workload 与 scheduler
// 2) 跑仿真
// 3) 打印表格
// 4) 导出 CSV
fn main() {
    // Step 1: 创建实验输入（workload）。
    let workloads = workload_suite();
    // 收集所有实验组合结果。
    let mut results = Vec::new();

    // 笛卡尔积实验：每个 workload 都跑全部策略。
    for workload in &workloads {
        for scheduler in scheduler_suite() {
            results.push(run_experiment(workload, scheduler));
        }
    }

    println!("\n== Scheduling Experiment Suite ==\n");
    // Step 2: 打印终端对比表。
    print_table(&results);

    // Step 3: 导出聚合指标与事件日志。
    export_metrics_csv(&results);
    export_context_switch_csv(&results);

    // 终端提示导出位置。
    println!("\nCSV exported:");
    println!("- experiment_metrics.csv");
    println!("- context_switches.csv");
}
