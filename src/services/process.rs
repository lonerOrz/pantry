use std::io;
use std::time::Duration;

#[derive(Debug)]
pub struct CommandOutput {
    pub success: bool,
    pub stdout: Vec<u8>,
}

pub trait CommandExecutor: Send + Sync {
    fn execute(&self, program: &str, args: &[&str]) -> io::Result<CommandOutput>;
    fn execute_with_timeout(
        &self,
        program: &str,
        args: &[&str],
        timeout_secs: u64,
    ) -> io::Result<CommandOutput>;
}

pub struct ShellExec;

impl CommandExecutor for ShellExec {
    fn execute(&self, program: &str, args: &[&str]) -> io::Result<CommandOutput> {
        let output = std::process::Command::new(program).args(args).output()?;
        Ok(CommandOutput {
            success: output.status.success(),
            stdout: output.stdout,
        })
    }

    fn execute_with_timeout(
        &self,
        program: &str,
        args: &[&str],
        timeout_secs: u64,
    ) -> io::Result<CommandOutput> {
        let child = std::process::Command::new(program)
            .args(args)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()?;

        let pid = child.id();
        let (tx, rx) = std::sync::mpsc::channel();

        std::thread::spawn(move || {
            let result = child.wait_with_output();
            let _ = tx.send(result);
        });

        match rx.recv_timeout(Duration::from_secs(timeout_secs)) {
            Ok(result) => {
                let output = result?;
                Ok(CommandOutput {
                    success: output.status.success(),
                    stdout: output.stdout,
                })
            }
            Err(_) => {
                let _ = std::process::Command::new("kill")
                    .arg("-9")
                    .arg(pid.to_string())
                    .output();
                Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    format!("Command timed out after {} seconds", timeout_secs),
                ))
            }
        }
    }
}

#[cfg(test)]
pub struct MockExec {
    responses: std::sync::Mutex<Vec<io::Result<CommandOutput>>>,
}

#[cfg(test)]
impl MockExec {
    pub fn new() -> Self {
        Self {
            responses: std::sync::Mutex::new(Vec::new()),
        }
    }

    pub fn push_ok(self, success: bool, stdout: Vec<u8>) -> Self {
        self.responses
            .lock()
            .unwrap()
            .push(Ok(CommandOutput { success, stdout }));
        self
    }

    pub fn push_err(self, err: io::Error) -> Self {
        self.responses.lock().unwrap().push(Err(err));
        self
    }
}

#[cfg(test)]
impl CommandExecutor for MockExec {
    fn execute(&self, _program: &str, _args: &[&str]) -> io::Result<CommandOutput> {
        self.responses
            .lock()
            .unwrap()
            .pop()
            .unwrap_or(Ok(CommandOutput {
                success: true,
                stdout: Vec::new(),
            }))
    }

    fn execute_with_timeout(
        &self,
        program: &str,
        args: &[&str],
        _timeout_secs: u64,
    ) -> io::Result<CommandOutput> {
        self.execute(program, args)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_exec_returns_ok() {
        let mock = MockExec::new().push_ok(true, b"hello\n".to_vec());
        let result = mock.execute("sh", &["-c", "echo hello"]).unwrap();
        assert!(result.success);
        assert_eq!(result.stdout, b"hello\n");
    }

    #[test]
    fn mock_exec_returns_err() {
        let mock = MockExec::new().push_err(io::Error::other("fail"));
        let result = mock.execute("sh", &["-c", "false"]);
        assert!(result.is_err());
    }

    #[test]
    fn mock_exec_lifo_order() {
        let mock = MockExec::new()
            .push_ok(true, b"first".to_vec())
            .push_ok(true, b"second".to_vec());
        let r1 = mock.execute("sh", &["-c", "a"]).unwrap();
        let r2 = mock.execute("sh", &["-c", "b"]).unwrap();
        assert_eq!(r1.stdout, b"second");
        assert_eq!(r2.stdout, b"first");
    }
}
