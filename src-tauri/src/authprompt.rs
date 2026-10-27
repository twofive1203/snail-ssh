//! keyboard-interactive / OTP 的提问桥。
//!
//! 建连跑在阻塞线程的异步任务里，不能直接弹窗。这里发 Tauri 事件，
//! 等前端把答案从命令通道送回来。取消或超时都要让建连失败，不能空等。

use parking_lot::Mutex;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::LazyLock;
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tokio::sync::oneshot;

static WAITERS: LazyLock<Mutex<HashMap<String, oneshot::Sender<Option<Vec<String>>>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

const PROMPT_TIMEOUT: Duration = Duration::from_secs(180);

#[derive(Clone)]
pub struct AuthPrompter {
    app: AppHandle,
}

impl AuthPrompter {
    pub fn new(app: AppHandle) -> Self {
        Self { app }
    }

    pub async fn ask(
        &self,
        host_label: &str,
        name: &str,
        instructions: &str,
        prompts: &[PromptField],
    ) -> Result<Vec<String>, String> {
        if prompts.is_empty() {
            return Ok(Vec::new());
        }
        let request_id = uuid::Uuid::new_v4().to_string();
        let (tx, rx) = oneshot::channel();
        WAITERS.lock().insert(request_id.clone(), tx);
        let payload = AuthPromptEvent {
            request_id: request_id.clone(),
            host_label: host_label.to_string(),
            name: name.to_string(),
            instructions: instructions.to_string(),
            prompts: prompts.to_vec(),
        };
        if let Err(err) = self.app.emit("ssh-auth-prompt", payload) {
            WAITERS.lock().remove(&request_id);
            return Err(format!("无法向界面发起认证提问: {}", err));
        }
        match tokio::time::timeout(PROMPT_TIMEOUT, rx).await {
            Ok(Ok(Some(answers))) => {
                if answers.len() != prompts.len() {
                    Err("交互式认证答案数量不匹配".into())
                } else {
                    Ok(answers)
                }
            }
            Ok(Ok(None)) => Err("已取消交互式认证".into()),
            Ok(Err(_)) => Err("交互式认证已中断".into()),
            Err(_) => {
                WAITERS.lock().remove(&request_id);
                Err("交互式认证超时（3 分钟内未填写）".into())
            }
        }
    }
}

/// 前端提交答案。`responses == None` 表示用户取消。
pub fn submit(request_id: &str, responses: Option<Vec<String>>) -> Result<(), String> {
    let tx = WAITERS
        .lock()
        .remove(request_id)
        .ok_or_else(|| "认证请求已过期或不存在".to_string())?;
    tx.send(responses)
        .map_err(|_| "认证请求已结束".to_string())
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AuthPromptEvent {
    request_id: String,
    host_label: String,
    name: String,
    instructions: String,
    prompts: Vec<PromptField>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptField {
    pub prompt: String,
    /// 服务端要求不回显（密码、OTP）
    pub secret: bool,
}
