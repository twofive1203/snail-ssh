use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// 传输取消时返回给前端的哨兵字符串：前端据此显示"已取消"而非"失败"。
pub const TRANSFER_CANCELLED: &str = "TRANSFER_CANCELLED";

/// 暂停轮询粒度：足够小以保证响应，又不会空转烧 CPU
const PAUSE_POLL_INTERVAL: Duration = Duration::from_millis(120);

/// 单个传输任务的控制信号。传输线程是同步的（spawn_blocking），
/// 因此暂停用"阻塞轮询"实现，取消用协作式检查点实现。
#[derive(Default)]
pub struct TransferSignal {
    cancelled: AtomicBool,
    paused: AtomicBool,
    /// 最近一次已知的已传输字节数（供 UI 查询/调试，续传起点以文件元数据为准）
    transferred: AtomicU64,
}

impl TransferSignal {
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }

    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::Relaxed)
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
        // 取消必须同时解除暂停，否则暂停中的线程会永远 sleep 在轮询里
        self.paused.store(false, Ordering::Relaxed);
    }

    pub fn pause(&self) {
        if !self.is_cancelled() {
            self.paused.store(true, Ordering::Relaxed);
        }
    }

    pub fn resume(&self) {
        self.paused.store(false, Ordering::Relaxed);
    }

    pub fn set_transferred(&self, n: u64) {
        self.transferred.store(n, Ordering::Relaxed);
    }

    /// 供单测与后续 UI 查询使用（当前续传起点以文件元数据为准，运行时代码未读该值）
    #[allow(dead_code)]
    pub fn transferred(&self) -> u64 {
        self.transferred.load(Ordering::Relaxed)
    }

    /// 暂停时阻塞轮询直到恢复；返回 false 表示期间已被取消
    pub fn wait_if_paused(&self) -> bool {
        while self.is_paused() {
            if self.is_cancelled() {
                return false;
            }
            std::thread::sleep(PAUSE_POLL_INTERVAL);
        }
        !self.is_cancelled()
    }
}

struct TransferEntry {
    task_id: String,
    signal: Arc<TransferSignal>,
    session_id: String,
}

/// 注册表键：会话 + 任务。
///
/// 不能只用 task_id：批量拖拽会在同一毫秒内建出多个任务，一旦 id 撞车，
/// 两台主机的上传就会共用同一个控制信号和同一个 `sftp-progress-<taskId>` 事件名，
/// 表现为「另一台机器瞬间传完、内容却不对」。加上会话前缀后，跨会话永不互相干扰。
fn registry_key(session_id: &str, task_id: &str) -> String {
    format!("{}\u{1}{}", session_id, task_id)
}

/// 任务控制注册表：按「会话 + task_id」索引控制信号，并记录所属会话以便会话关闭时统一取消。
#[derive(Default, Clone)]
pub struct TransferRegistry {
    inner: Arc<Mutex<HashMap<String, TransferEntry>>>,
}

impl TransferRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// 注册任务；同一「会话 + task_id」已存在（重试场景）时复用同一信号并清空暂停/进度状态。
    pub fn register(&self, task_id: &str, session_id: &str) -> Arc<TransferSignal> {
        let key = registry_key(session_id, task_id);
        let mut map = self.inner.lock();
        if let Some(entry) = map.get_mut(&key) {
            entry.signal.resume();
            entry.signal.set_transferred(0);
            return Arc::clone(&entry.signal);
        }
        let signal = Arc::new(TransferSignal::default());
        map.insert(
            key,
            TransferEntry {
                task_id: task_id.to_string(),
                signal: Arc::clone(&signal),
                session_id: session_id.to_string(),
            },
        );
        signal
    }

    /// 按 task_id 取控制信号（存在多个同名任务时返回任意一个）。
    /// 生产路径的 pause/cancel 走 `control`（它需要覆盖所有同名任务），
    /// 这里保留给单测与后续调试查询。
    #[allow(dead_code)]
    pub fn get(&self, task_id: &str) -> Option<Arc<TransferSignal>> {
        self.inner
            .lock()
            .values()
            .find(|entry| entry.task_id == task_id)
            .map(|entry| Arc::clone(&entry.signal))
    }

    /// 传输结束（成功/失败/取消）后清理，避免 Map 随任务数增长
    pub fn finish(&self, task_id: &str, session_id: &str) {
        self.inner.lock().remove(&registry_key(session_id, task_id));
    }

    /// 会话关闭时取消其名下所有传输，唤醒暂停中的线程避免永久 sleep
    pub fn cancel_by_session(&self, session_id: &str) {
        let signals: Vec<Arc<TransferSignal>> = self
            .inner
            .lock()
            .values()
            .filter(|entry| entry.session_id == session_id)
            .map(|entry| Arc::clone(&entry.signal))
            .collect();
        for signal in signals {
            signal.cancel();
        }
    }

    /// 对已注册任务执行 pause / resume / cancel；任务不存在时报明确错误，不静默成功。
    /// 控制命令只带 task_id（前端暂停/取消按钮没有会话上下文），因此作用于所有同名任务。
    pub fn control(&self, task_id: &str, action: &str) -> Result<(), String> {
        if !matches!(action, "pause" | "resume" | "cancel") {
            return Err(format!("不支持的传输控制动作: {}", action));
        }
        let map = self.inner.lock();
        let mut matched = 0usize;
        for entry in map.values().filter(|entry| entry.task_id == task_id) {
            matched += 1;
            match action {
                "pause" => entry.signal.pause(),
                "resume" => entry.signal.resume(),
                "cancel" => entry.signal.cancel(),
                _ => {}
            }
        }
        if matched == 0 {
            return Err(format!("传输任务不存在或已结束: {}", task_id));
        }
        Ok(())
    }
}

/// 断点续传起点：仅当目标已存在且比源短时才续传（返回目标长度），
/// 否则从头开始——目标更长时续传会留下脏尾巴，等长时说明已完成/需重传。
pub fn resume_offset(existing_len: Option<u64>, source_len: u64) -> u64 {
    match existing_len {
        Some(len) if len < source_len => len,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_register_get_finish_lifecycle() {
        let registry = TransferRegistry::new();
        assert!(registry.get("t1").is_none());

        let signal = registry.register("t1", "sess-a");
        assert!(registry.get("t1").is_some());
        // 未暂停且未取消
        assert!(!signal.is_paused());
        assert!(!signal.is_cancelled());

        registry.finish("t1", "sess-a");
        assert!(registry.get("t1").is_none());
    }

    #[test]
    fn same_task_id_in_different_sessions_is_isolated() {
        let registry = TransferRegistry::new();
        let a = registry.register("t1", "sess-a");
        let b = registry.register("t1", "sess-b");
        // 跨会话撞 id 时不能复用同一个信号（否则进度事件互相串台）
        assert!(!Arc::ptr_eq(&a, &b));

        a.pause();
        assert!(a.is_paused());
        assert!(!b.is_paused());

        // 一个会话结束只清理自己那条记录
        registry.finish("t1", "sess-a");
        assert!(!Arc::ptr_eq(&registry.get("t1").unwrap(), &a));
        registry.finish("t1", "sess-b");
        assert!(registry.get("t1").is_none());
    }

    #[test]
    fn registry_register_reuses_signal_and_clears_state() {
        let registry = TransferRegistry::new();
        let first = registry.register("t1", "sess-a");
        first.pause();
        first.set_transferred(1234);

        let second = registry.register("t1", "sess-a");
        assert!(Arc::ptr_eq(&first, &second));
        // 重试复用同一信号，但暂停/进度被清空
        assert!(!second.is_paused());
        assert_eq!(second.transferred(), 0);
    }

    #[test]
    fn registry_cancel_by_session_only_touches_that_session() {
        let registry = TransferRegistry::new();
        let a = registry.register("t1", "sess-a");
        let b = registry.register("t2", "sess-b");
        registry.cancel_by_session("sess-a");
        assert!(a.is_cancelled());
        assert!(!b.is_cancelled());
    }

    #[test]
    fn registry_control_actions_and_unknown_task() {
        let registry = TransferRegistry::new();
        let signal = registry.register("t1", "sess-a");

        registry.control("t1", "pause").unwrap();
        assert!(signal.is_paused());
        registry.control("t1", "resume").unwrap();
        assert!(!signal.is_paused());
        registry.control("t1", "cancel").unwrap();
        assert!(signal.is_cancelled());

        assert!(registry.control("missing", "pause").is_err());
        assert!(registry.control("t1", "explode").is_err());
    }

    #[test]
    fn control_actions_reach_every_session_with_that_task_id() {
        let registry = TransferRegistry::new();
        let a = registry.register("t1", "sess-a");
        let b = registry.register("t1", "sess-b");
        assert!(registry.control("t1", "cancel").is_ok());
        assert!(a.is_cancelled());
        assert!(b.is_cancelled());
    }

    #[test]
    fn cancel_releases_paused_wait() {
        let signal = TransferSignal::default();
        signal.pause();
        assert!(signal.is_paused());
        // 取消后 wait_if_paused 应立即返回 false，而不是等满轮询
        signal.cancel();
        assert!(!signal.wait_if_paused());
    }

    #[test]
    fn resume_offset_only_resumes_shorter_targets() {
        assert_eq!(resume_offset(None, 1000), 0);
        assert_eq!(resume_offset(Some(0), 1000), 0);
        assert_eq!(resume_offset(Some(400), 1000), 400);
        assert_eq!(resume_offset(Some(1000), 1000), 0);
        assert_eq!(resume_offset(Some(2000), 1000), 0);
    }
}
