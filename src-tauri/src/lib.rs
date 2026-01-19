pub fn run(name: &str) -> String {
    if name.is_empty() {
        String::from("Hello, stranger! You've been greeted from Rust!")
    } else {
        format!("Hello, {}! You've been greeted from Rust!", name)
    }
}

#[cfg(test)]
mod tests {
    use super::run;

    #[test]
    fn test_run_command() {
        let result = run("TestUser");
        assert_eq!(result, "Hello, TestUser! You've been greeted from Rust!");
    }

    #[test]
    fn test_run_command_empty() {
        let result = run("");
        assert_eq!(result, "Hello, stranger! You've been greeted from Rust!");
    }
}
