//! Colored / verbose terminal output (`NO_COLOR` respected).

use crate::types::Match;

/// ANSI color codes for terminal output
pub struct Colors;

impl Colors {
    /// Reset color
    pub const RESET: &'static str = "\x1b[0m";

    /// Text colors
    pub const GREEN: &'static str = "\x1b[32m"; // Success statements
    pub const CYAN: &'static str = "\x1b[36m"; // Paths, files, URLs
    pub const YELLOW: &'static str = "\x1b[33m"; // Action statements
    pub const MAGENTA: &'static str = "\x1b[35m"; // Magenta
    pub const VIOLET: &'static str = "\x1b[38;5;99m"; // Violet for process names (256-color mode #99)

    /// Platform-specific colors (256-color mode)
    pub const PLATFORM_WINDOWS: &'static str = "\x1b[31m"; // Red
    pub const PLATFORM_MACOS: &'static str = "\x1b[38;5;202m"; // Peach (256-color mode #202)
    pub const PLATFORM_LINUX: &'static str = "\x1b[38;5;27m"; // Blue (256-color mode #27)

    /// Check if colors should be enabled
    pub fn should_enable() -> bool {
        // Check if stdout is a TTY and not NO_COLOR is set
        atty::is(atty::Stream::Stdout) && std::env::var("NO_COLOR").is_err()
    }

    /// Get platform-specific color
    pub fn get_platform_color(platform_name: &str) -> &'static str {
        match platform_name {
            "Windows" => Self::PLATFORM_WINDOWS,
            "Darwin" => Self::PLATFORM_MACOS,
            "Linux" => Self::PLATFORM_LINUX,
            _ => Self::VIOLET,
        }
    }
}

/// Handles all colored output and formatting
pub struct OutputFormatter {
    pub colors_enabled: bool,
    /// When false, verbose-only details (raw match address/region/string) are suppressed.
    pub verbose: bool,
}

impl OutputFormatter {
    pub fn new() -> Self {
        Self {
            colors_enabled: Colors::should_enable(),
            verbose: false,
        }
    }

    /// Construct a formatter with an explicit verbosity setting.
    pub fn with_verbose(verbose: bool) -> Self {
        Self {
            colors_enabled: Colors::should_enable(),
            verbose,
        }
    }

    /// Format context bytes as readable text, using ⁇ for non-printable characters
    pub fn format_context_text(context_bytes: &[u8]) -> String {
        context_bytes
            .iter()
            .map(|&byte_value| {
                if 32 <= byte_value && byte_value <= 126 {
                    char::from(byte_value)
                } else {
                    '⁇'
                }
            })
            .collect()
    }

    /// Print platform information with colored output
    pub fn print_platform_info(&self, platform_name: &str) {
        if self.colors_enabled {
            let platform_color = Colors::get_platform_color(platform_name);
            println!(
                "{}Platform:{} {}{}{}",
                Colors::MAGENTA,
                Colors::RESET,
                platform_color,
                platform_name,
                Colors::RESET
            );
        } else {
            println!("Platform: {}", platform_name);
        }
        println!("{}", "=".repeat(65));
    }

    /// Print a match with formatted output.
    ///
    /// The raw match details (address/region/string/context) are only shown in
    /// verbose mode; otherwise this is a no-op.
    pub fn print_match(&self, match_data: &Match) {
        if !self.verbose {
            return;
        }
        if self.colors_enabled {
            println!("  {}*** ACCOUNT ID FOUND ***{}", Colors::GREEN, Colors::RESET);

            if let Some(address) = match_data.address {
                println!("  {}Address:{} 0x{:x}", Colors::CYAN, Colors::RESET, address);
            }
            if let Some(ref region) = match_data.region {
                println!("  {}Region:{} {}", Colors::CYAN, Colors::RESET, region);
            }

            println!("  {}String:{} {}", Colors::CYAN, Colors::RESET, match_data.string);
            println!(
                "  {}Context:{} {}",
                Colors::CYAN,
                Colors::RESET,
                Self::format_context_text(&match_data.context)
            );
        } else {
            println!("  *** ACCOUNT ID FOUND ***");
            if let Some(address) = match_data.address {
                println!("  Address: 0x{:x}", address);
            }
            if let Some(ref region) = match_data.region {
                println!("  Region: {}", region);
            }
            println!("  String: {}", match_data.string);
            println!(
                "  Context: {}",
                Self::format_context_text(&match_data.context)
            );
        }
        println!();
    }

    /// Print colored message with newline
    pub fn println_colored(&self, color: &str, message: &str) {
        if self.colors_enabled {
            println!("{}{}{}", color, message, Colors::RESET);
        } else {
            println!("{}", message);
        }
    }
}

impl Default for OutputFormatter {
    fn default() -> Self {
        Self::new()
    }
}
