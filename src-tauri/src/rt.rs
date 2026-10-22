//! 供同步命令使用的 tokio 运行时。
//!
//! Tauri 命令最终都跑在阻塞线程里（`run_blocking` / `run_transfer`），而 russh 是异步的，
//! 因此这里给网络层准备一条独立的多线程运行时：阻塞线程用 `block_on` 驱动单个操作，
//! 长生命周期的任务（shell 输出泵）用 `spawn` 常驻。

use std::future::Future;
use std::sync::OnceLock;
use tokio::runtime::{Builder, Runtime};
use tokio::task::JoinHandle;

/// 网络层固定 2 个 worker：一条连接一个读泵，够用且不抢占界面线程
const WORKER_THREADS: usize = 2;

fn runtime() -> &'static Runtime {
    static RT: OnceLock<Runtime> = OnceLock::new();
    RT.get_or_init(|| {
        Builder::new_multi_thread()
            .worker_threads(WORKER_THREADS)
            .thread_name("snail-ssh-net")
            .enable_all()
            .build()
            .expect("创建 SSH 网络运行时失败")
    })
}

/// 在阻塞线程里同步等待一个异步操作。
///
/// 严禁在运行时异步上下文里调用（例如异步任务内部）：tokio 会直接 panic。
pub fn block_on<F: Future>(future: F) -> F::Output {
    runtime().block_on(future)
}

/// 投递常驻任务（shell 输出泵、SFTP 传输等）到网络运行时
pub fn spawn<F>(future: F) -> JoinHandle<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    runtime().spawn(future)
}
