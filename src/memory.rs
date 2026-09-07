//! Process memory scan for `?accountId=` (Windows VirtualQuery / Linux `/proc`).

use anyhow::{Context, Result};
use crate::types::Match;
use crate::pattern::PatternMatcher;

const CONTEXT_BYTES_BEFORE: usize = 50;
const CONTEXT_BYTES_AFTER: usize = 100;
const MAX_REGION_SIZE: usize = 100 * 1024 * 1024; // 100MB

/// Handles process memory scanning
pub struct MemoryScanner {
    pattern: &'static [u8],
}

impl MemoryScanner {
    pub fn new(pattern: &'static [u8]) -> Self {
        Self { pattern }
    }

    /// Extract context bytes around a position
    fn extract_context(&self, data: &[u8], pos: usize) -> Vec<u8> {
        let start = pos.saturating_sub(CONTEXT_BYTES_BEFORE);
        let end = (pos + self.pattern.len() + CONTEXT_BYTES_AFTER).min(data.len());
        data[start..end].to_vec()
    }

    /// Process a pattern match and return match data if valid
    fn process_match(
        &self,
        data: &[u8],
        pos: usize,
        base_address: usize,
        region_info: &str,
    ) -> Option<Match> {
        let context = self.extract_context(data, pos);
        let full_string = PatternMatcher::extract_string_from_data(data, self.pattern, pos);

        // Only include matches that have both ?accountId= and &nonce=
        if !PatternMatcher::has_both_patterns(&full_string) {
            return None;
        }

        Some(Match::from_process_memory(
            full_string,
            base_address + pos,
            region_info.to_string(),
            context,
        ))
    }

    /// Common pattern matching logic for searching in data
    fn search_in_data(
        &self,
        data: &[u8],
        base_address: usize,
        region_info: &str,
        stop_at_first_complete: bool,
    ) -> Vec<Match> {
        let mut matches = Vec::new();
        let mut offset = 0;

        loop {
            if let Some(pos) = data[offset..]
                .windows(self.pattern.len())
                .position(|window| window == self.pattern)
                .map(|found| found + offset)
            {
                if let Some(match_data) = self.process_match(data, pos, base_address, region_info) {
                    matches.push(match_data);

                    if stop_at_first_complete {
                        let (account_id, nonce) =
                            PatternMatcher::extract_account_id_and_nonce(&matches.last().unwrap().string);
                        if account_id.is_some() && nonce.is_some() {
                            return matches;
                        }
                    }
                }
                offset = pos + 1;
            } else {
                break;
            }
        }

        matches
    }

    /// Search memory regions from /proc/[pid]/maps (Linux)
    #[cfg(unix)]
    pub fn search_memory_regions(&self, pid: i32, stop_at_first_complete: bool) -> Result<Vec<Match>> {
        use procfs::process::Process;
        use std::fs::File;
        use std::io::{Seek, SeekFrom, Read};

        let process = Process::new(pid)
            .context("Failed to open process")?;

        let maps = process.maps()
            .context("Failed to read process maps")?;

        let mem_path = format!("/proc/{}/mem", pid);
        let mut mem_file = File::open(&mem_path)
            .context("Failed to open process memory file")?;

        let mut all_matches = Vec::new();

        for map in maps {
            if !map.perms.contains(procfs::process::MMPermissions::READ) {
                continue;
            }

            let size = map.address.1 - map.address.0;
            if size == 0 || size > MAX_REGION_SIZE as u64 {
                continue;
            }

            if mem_file.seek(SeekFrom::Start(map.address.0)).is_err() {
                continue;
            }

            let mut buffer = vec![0u8; size as usize];
            if mem_file.read_exact(&mut buffer).is_err() {
                continue;
            }

            let region_info = format!("{:x}-{:x}", map.address.0, map.address.1);
            let region_matches = self.search_in_data(
                &buffer,
                map.address.0 as usize,
                &region_info,
                stop_at_first_complete,
            );

            all_matches.extend(region_matches);

            if stop_at_first_complete && !all_matches.is_empty() {
                let last_match = &all_matches[all_matches.len() - 1];
                let (account_id, nonce) = PatternMatcher::extract_account_id_and_nonce(&last_match.string);
                if account_id.is_some() && nonce.is_some() {
                    return Ok(all_matches);
                }
            }
        }

        Ok(all_matches)
    }

    /// Search memory regions in a Windows process
    #[cfg(windows)]
    pub fn search_windows_memory_regions(&self, pid: u32, stop_at_first_complete: bool) -> Result<Vec<Match>> {
        use winapi::um::memoryapi::{VirtualQueryEx, ReadProcessMemory};
        use winapi::um::processthreadsapi::OpenProcess;
        use winapi::um::winnt::{MEMORY_BASIC_INFORMATION, PROCESS_VM_READ, PROCESS_QUERY_INFORMATION};
        use winapi::um::handleapi::CloseHandle;
        use std::mem::size_of;

        let handle = unsafe {
            OpenProcess(PROCESS_VM_READ | PROCESS_QUERY_INFORMATION, 0, pid)
        };

        if handle.is_null() {
            anyhow::bail!("Failed to open process");
        }

        let mut all_matches = Vec::new();
        let mut address: usize = 0;

        loop {
            let mut mbi: MEMORY_BASIC_INFORMATION = unsafe { std::mem::zeroed() };
            let size = unsafe {
                VirtualQueryEx(handle, address as *const _, &mut mbi, size_of::<MEMORY_BASIC_INFORMATION>())
            };

            if size == 0 {
                break;
            }

            if mbi.State == winapi::um::winnt::MEM_COMMIT
                && (mbi.Protect == winapi::um::winnt::PAGE_READONLY
                    || mbi.Protect == winapi::um::winnt::PAGE_READWRITE
                    || mbi.Protect == winapi::um::winnt::PAGE_EXECUTE_READ
                    || mbi.Protect == winapi::um::winnt::PAGE_EXECUTE_READWRITE)
                && mbi.RegionSize < MAX_REGION_SIZE
            {
                let region_size = mbi.RegionSize;
                let mut buffer = vec![0u8; region_size];
                let mut bytes_read = 0;

                if unsafe {
                    ReadProcessMemory(
                        handle,
                        mbi.BaseAddress as *const _,
                        buffer.as_mut_ptr() as *mut _,
                        region_size,
                        &mut bytes_read,
                    )
                } != 0
                {
                    buffer.truncate(bytes_read);

                    let region_info = format!("0x{:x}-0x{:x}",
                        mbi.BaseAddress as usize,
                        (mbi.BaseAddress as usize) + mbi.RegionSize);

                    let region_matches = self.search_in_data(
                        &buffer,
                        mbi.BaseAddress as usize,
                        &region_info,
                        stop_at_first_complete,
                    );

                    all_matches.extend(region_matches);

                    if stop_at_first_complete && !all_matches.is_empty() {
                        let last_match = &all_matches[all_matches.len() - 1];
                        let (account_id, nonce) = PatternMatcher::extract_account_id_and_nonce(&last_match.string);
                        if account_id.is_some() && nonce.is_some() {
                            unsafe { CloseHandle(handle) };
                            return Ok(all_matches);
                        }
                    }
                }
            }

            address = (mbi.BaseAddress as usize) + mbi.RegionSize;
        }

        unsafe { CloseHandle(handle) };
        Ok(all_matches)
    }

    #[cfg(not(unix))]
    pub fn search_memory_regions(&self, _pid: i32, _stop_at_first_complete: bool) -> Result<Vec<Match>> {
        anyhow::bail!("Unix memory scanning not available on this platform");
    }

    #[cfg(not(windows))]
    pub fn search_windows_memory_regions(&self, _pid: u32, _stop_at_first_complete: bool) -> Result<Vec<Match>> {
        anyhow::bail!("Windows memory scanning not available on this platform");
    }
}
