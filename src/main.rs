use clap::Parser;
use notify_rust::Notification;
use std::error::Error;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// The program to execute
    #[arg()]
    program: String,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    let mut command = Command::new(&args.program);
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());

    let mut child = command.spawn()?;

    // Create a thread to read stderr in real-time
    let stderr = child.stderr.take().expect("Failed to capture stderr");
    let stderr_lines = Arc::new(Mutex::new(Vec::new()));
    let stderr_lines_clone = Arc::clone(&stderr_lines);

    let stderr_thread = thread::spawn(move || {
        let reader = BufReader::new(stderr);
        for line in reader.lines() {
            if let Ok(line) = line {
                // Print to stderr immediately
                eprintln!("{}", line);

                // Store the line for later use
                let mut lines = stderr_lines_clone.lock().unwrap();
                lines.push(line);
            }
        }
    });

    // Wait for the command to complete
    let output = child.wait()?;

    // Wait for stderr processing to complete
    stderr_thread.join().unwrap();

    if !output.success() {
        // Get the collected stderr lines
        let stderr_lines = stderr_lines.lock().unwrap();

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
    } else {
        // For stdout, we still use the original approach since we don't need real-time output
        let stdout = child.stdout.take().expect("Failed to capture stdout");
        let reader = BufReader::new(stdout);
        let stdout_content: String = reader
            .lines()
            .filter_map(Result::ok)
            .collect::<Vec<String>>()
            .join("\n");

        if !stdout_content.trim().is_empty() {
            Notification::new()
                .summary("Program Output")
                .body(&stdout_content)
                .show()?;
        }
    }

    Ok(())
}
