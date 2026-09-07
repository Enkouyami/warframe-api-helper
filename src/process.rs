//! Locate the Warframe process (`Warframe.x64.exe`; Linux may truncate the name).

use anyhow::Result;
#[cfg(windows)]
use anyhow::Context;
use crate::output::{Colors, OutputFormatter};

pub struct ProcessFinder;

impl ProcessFinder {
    /// Resolve PID for Warframe on the current platform (or bail if not found).
    pub fn find_warframe_process(platform_name: &str) -> Result<u32> {
        let formatter = OutputFormatter::new();
        
        if platform_name == "Windows" {
            Self::find_windows_process(&formatter)
        } else {
            Self::find_unix_process(&formatter)
        }
    }
    
    #[cfg(windows)]
    fn find_windows_process(formatter: &OutputFormatter) -> Result<u32> {
        use std::process::Command;
        
        let output = Command::new("tasklist")
            .args(&["/FI", "IMAGENAME eq Warframe.x64.exe", "/FO", "CSV", "/NH"])
            .output()
            .context("Failed to run tasklist")?;
        
        if !output.status.success() {
            anyhow::bail!("Failed to find Warframe process");
        }
        
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            if line.contains("Warframe.x64.exe") {
                let parts: Vec<&str> = line.split("\",\"").collect();
                if parts.len() >= 2 {
                    if let Ok(pid) = parts[1].trim_matches('"').parse::<u32>() {
                        formatter.println_colored(
                            Colors::GREEN,
                            &format!("Found Warframe process: Warframe.x64.exe (PID: {})", pid)
                        );
                        return Ok(pid);
                    }
                }
            }
        }
        
        anyhow::bail!("Process not found: Warframe.x64.exe");
    }
    
    #[cfg(not(windows))]
    fn find_windows_process(_formatter: &OutputFormatter) -> Result<u32> {
        anyhow::bail!("Windows process finding not available on this platform");
    }
    
    #[cfg(unix)]
    fn find_unix_process(formatter: &OutputFormatter) -> Result<u32> {
        use std::process::Command;
        
        // Try full name first, then truncated (Linux truncates to 16 chars)
        let process_names = ["Warframe.x64.exe", "Warframe.x64.ex"];
        
        for name in &process_names {
            if let Ok(output) = Command::new("pgrep")
                .args(&["-f", name])
                .output()
            {
                if output.status.success() {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    if let Some(first_line) = stdout.lines().next() {
                        if let Ok(pid) = first_line.split_whitespace().next()
                            .and_then(|s| s.parse::<u32>().ok())
                            .ok_or_else(|| anyhow::anyhow!("Failed to parse PID"))
                        {
                            formatter.println_colored(
                                Colors::GREEN,
                                &format!("Found Warframe process: {} (PID: {})", name, pid)
                            );
                            return Ok(pid);
                        }
                    }
                }
            }
        }
        
        // Fallback: try ps command
        if let Ok(output) = Command::new("ps").arg("aux").output() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                if line.contains("Warframe.x64.exe") || line.contains("Warframe.x64.ex") {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 2 {
                        if let Ok(pid) = parts[1].parse::<u32>() {
                            formatter.println_colored(
                                Colors::GREEN,
                                &format!("Found Warframe process (PID: {})", pid)
                            );
                            return Ok(pid);
                        }
                    }
                }
            }
        }
        
        anyhow::bail!("Process not found: Warframe.x64.exe");
    }
    
    #[cfg(not(unix))]
    fn find_unix_process(_formatter: &OutputFormatter) -> Result<u32> {
        anyhow::bail!("Unix process finding not available on this platform");
    }
}
