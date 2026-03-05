pub fn protocol_stdout_line(method: &str) -> String {
    serde_json::json!({
        "method": method,
        "params": {}
    })
    .to_string()
}

pub fn protocol_stderr_line(message: &str) -> String {
    format!("{message}\n")
}
