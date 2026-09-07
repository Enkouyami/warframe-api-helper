//! Scan Warframe process memory for `?accountId=` / `&nonce=` matches.

use anyhow::Result;
use crate::types::Match;
use crate::memory::MemoryScanner;
use crate::pattern::PatternMatcher;
use crate::output::OutputFormatter;
use crate::process::ProcessFinder;

pub struct SearchWorkflow;

impl SearchWorkflow {
    /// Auto-detect or use `--pid`, then scan readable memory regions.
    pub fn execute(
        formatter: &OutputFormatter,
        platform_name: &str,
        pid: Option<u32>,
        all_matches: bool,
    ) -> Result<Vec<Match>> {
        let scanner = MemoryScanner::new(PatternMatcher::PATTERN);
        let stop_at_first = !all_matches;
        let mut found_matches: Vec<Match> = Vec::new();

        let process_pid = if let Some(provided_pid) = pid {
            provided_pid
        } else {
            formatter.println_colored(
                crate::output::Colors::YELLOW,
                "Auto-detecting Warframe process..."
            );
            ProcessFinder::find_warframe_process(platform_name)?
        };

        let process_matches = Self::search_process_memory(
            formatter,
            &scanner,
            process_pid,
            platform_name,
            stop_at_first,
        )?;
        found_matches.extend(process_matches);

        Ok(found_matches)
    }

    fn search_process_memory(
        formatter: &OutputFormatter,
        scanner: &MemoryScanner,
        pid: u32,
        platform_name: &str,
        stop_at_first: bool,
    ) -> Result<Vec<Match>> {
        formatter.println_colored(
            crate::output::Colors::YELLOW,
            "=== Running Process Memory (by region) ==="
        );

        formatter.println_colored(
            crate::output::Colors::YELLOW,
            &format!("Searching for '?accountId=' and '&nonce=' in Warframe process (PID {})...", pid)
        );

        let matches = if platform_name == "Windows" {
            scanner.search_windows_memory_regions(pid, stop_at_first)?
        } else {
            scanner.search_memory_regions(pid as i32, stop_at_first)?
        };

        if matches.is_empty() {
            formatter.println_colored(
                crate::output::Colors::YELLOW,
                "No matches found in running process memory"
            );
        } else {
            formatter.println_colored(
                crate::output::Colors::GREEN,
                &format!("Found {} occurrence(s):", matches.len())
            );
            for match_data in &matches {
                formatter.print_match(match_data);
            }
        }

        Ok(matches)
    }
}
