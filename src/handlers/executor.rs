use std::process::Command;
use std::string::FromUtf8Error;

pub struct CommandExecutor;

impl CommandExecutor {
    /// 执行 shell 命令并返回输出
    pub fn execute(command: &str) -> Result<String, ExecutionError> {
        let output = Command::new("sh")
            .arg("-c")
            .arg(command)
            .output()
            .map_err(ExecutionError::IoError)?;

        if output.status.success() {
            String::from_utf8(output.stdout).map_err(ExecutionError::InvalidOutput)
        } else {
            let error_msg = String::from_utf8(output.stderr)
                .unwrap_or_else(|_| "Failed to decode stderr".to_string());
            Err(ExecutionError::CommandFailed(error_msg))
        }
    }
}

#[derive(Debug)]
pub enum ExecutionError {
    IoError(std::io::Error),
    CommandFailed(String),
    InvalidOutput(FromUtf8Error),
}

// 从 main.rs 迁移的辅助函数
pub fn execute_command(command: &str) -> Result<String, Box<dyn std::error::Error>> {
    let output = Command::new("sh").arg("-c").arg(command).output()?;

    if output.status.success() {
        Ok(String::from_utf8(output.stdout)?)
    } else {
        let error_msg = String::from_utf8(output.stderr)?;
        Err(format!("Command failed: {}", error_msg).into())
    }
}
