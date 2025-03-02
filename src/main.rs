use clap::Parser;
use notify_rust::Notification;
use std::error::Error;
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::Mutex;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
#[command(trailing_var_arg = true)]
struct Args {
    /// The program to execute
    #[arg()]
    program: String,

    /// Arguments to pass to the program
    #[arg(trailing_var_arg = true)]
    args: Vec<String>,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    let mut command = Command::new(&args.program);
    command.args(&args.args);
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());

    let mut child = command.spawn()?;

    // Capture stderr in real-time
    let stderr = child.stderr.take().expect("Failed to capture stderr");
    let stderr_lines = Arc::new(Mutex::new(Vec::new()));

    // Process stderr asynchronously
    let stderr_lines_clone = Arc::clone(&stderr_lines);
    let stderr_reader = async move {
        let mut reader = BufReader::new(stderr).lines();

        while let Some(line) = reader.next_line().await.expect("Failed to read line") {
            // Print to stderr immediately
            eprintln!("{}", line);

            // Store the line for later use
            let mut lines = stderr_lines_clone.lock().await;
            lines.push(line);
        }
    };

    // Capture stdout
    let stdout = child.stdout.take().expect("Failed to capture stdout");
    let stdout_reader = async {
        let mut reader = BufReader::new(stdout).lines();
        let mut stdout_content = Vec::new();

        while let Some(line) = reader.next_line().await.expect("Failed to read line") {
            stdout_content.push(line);
        }

        stdout_content.join("\n")
    };

    // Run both readers concurrently
    let ((), stdout_content) = tokio::join!(stderr_reader, stdout_reader);

    // Wait for the command to complete
    let output = child.wait().await?;

    if !output.success() {
        // Get the collected stderr lines
        let stderr_lines = stderr_lines.lock().await;

        // Get last 3 lines of stderr for notification
        let last_lines: Vec<_> = stderr_lines.iter().rev().take(3).collect();
        let notification_text = last_lines
            .into_iter()
            .rev()
            .map(|s| s.as_str())
            .collect::<Vec<&str>>()
            .join("\n");

        Notification::new()
            .summary("Program Error")
            .body(&notification_text)
            .show()?;
    } else if !stdout_content.trim().is_empty() {
        Notification::new()
            .summary("Program Output")
            .body(&stdout_content)
            .show()?;
    }

    Ok(())
}
