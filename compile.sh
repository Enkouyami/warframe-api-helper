#!/bin/bash
set -e

#################################
# COMPILATION FLAGS EXPLANATION #
#################################
#
# This script compiles warframe-api-helper for Linux, Windows, and macOS.
# Below are explanations of all compilation flags used:
#
# COMMON FLAGS (used for all platforms):
#   -std=c++17          : C++17 language standard (required for Soup library)
#   -fno-rtti           : Disable Runtime Type Information (reduces binary size)
#   -O3                 : Maximum optimization level (best performance)
#   -ffunction-sections : Place each function in its own section (enables dead code elimination)
#   -fdata-sections     : Place each data item in its own section (enables dead code elimination)
#   -DSOUP_STANDALONE   : Preprocessor macro telling Soup it's used standalone
#   -c                  : Compile only (don't link yet)
#   -ISoup/soup         : Add Soup/soup to include path
#
# LINUX/macOS SPECIFIC FLAGS:
#   -fPIE               : Generate Position-Independent Code (enables ASLR for security)
#   -pie                : Create Position-Independent Executable
#   -fuse-ld=lld        : Use LLVM linker (faster than GNU ld)
#   -Wl,--gc-sections   : Remove unused sections (dead code elimination)
#   -Wl,--icf=safe      : Identical Code Folding (merge identical functions safely)
#   -lstdc++            : Link C++ standard library
#   -pthread            : Link POSIX threads library
#   -lm                 : Link math library
#   -ldl                : Link dynamic linking library
#   -lresolv            : Link DNS resolver library
#   -lstdc++fs          : Link C++17 filesystem library
#
# WINDOWS SPECIFIC FLAGS:
#   -static-libgcc      : Statically link GCC runtime
#   -static-libstdc++   : Statically link C++ standard library
#   -lws2_32            : Link Windows Sockets library
#   -liphlpapi          : Link IP Helper API library
#   -Wl,--gc-sections   : Remove unused sections (GNU ld compatible)
#                         Note: --icf=safe is LLVM-specific, not available with mingw's GNU ld
#
# macOS SPECIFIC FLAGS:
#   -lc++               : Link C++ standard library (macOS uses libc++)
#   -pthread            : Link POSIX threads library
#   -Wl,-dead_strip     : Remove dead code (macOS linker flag)
#
# STRIPPING:
#   strip               : Remove debugging symbols (saves ~19% binary size)
#                         Use --strip flag to enable
#
################################################################################

# Detect number of CPU cores for parallel compilation
if command -v nproc >/dev/null 2>&1; then
	NUM_CORES=$(nproc)
elif [ -f /proc/cpuinfo ]; then
	NUM_CORES=$(grep -c processor /proc/cpuinfo)
else
	NUM_CORES=8  # Fallback
fi

# Function to clean build artifacts
clean_build() {
	local platform="$1"
	local keep_binary="${2:-false}"
	local binary_name="warframe-api-helper"
	
	# Determine binary name based on platform
	case "$platform" in
		windows|Windows|WIN32|win32)
			binary_name="warframe-api-helper.exe"
			;;
		*)
			binary_name="warframe-api-helper"
			;;
	esac
	
	echo "=== Cleaning build artifacts for $platform ==="
	cd "$(dirname "$0")"
	rm -rf Soup/bin/int/*.o bin/*.o 2>/dev/null || true
	if [ "$keep_binary" != "true" ]; then
		rm -f "bin/$binary_name" 2>/dev/null || true
	fi
	echo "Clean complete!"
}

# Parse arguments
PLATFORM=""
STRIP_FLAG=""
CLEAN_ONLY=false
CLEAN_AFTER=false

for arg in "$@"; do
	case "$arg" in
		--clean|-c)
			CLEAN_ONLY=true
			;;
		--clean-after|-ca)
			CLEAN_AFTER=true
			;;
		--strip|-s)
			STRIP_FLAG="--strip"
			;;
		linux|Linux|LINUX|windows|Windows|WIN32|win32|macos|MacOS|macOS|darwin)
			if [ -z "$PLATFORM" ]; then
				PLATFORM="$arg"
			fi
			;;
		*)
			echo "Warning: Unknown argument '$arg'"
			;;
	esac
done

# Default to Linux if no platform specified
if [ -z "$PLATFORM" ]; then
	PLATFORM="linux"
fi

# Handle clean-only mode
if [ "$CLEAN_ONLY" = true ]; then
	clean_build "$PLATFORM"
	exit 0
fi

echo "=== Compiling warframe-api-helper for $PLATFORM ==="
cd "$(dirname "$0")"

# Platform-specific settings
case "$PLATFORM" in
	linux|Linux|LINUX)
		CXX="clang++"
		STRIP_CMD="strip"
		BINARY_NAME="warframe-api-helper"
		PIE_FLAGS="-fPIE -pie"
		LINKER_FLAGS="-fuse-ld=lld -Wl,--gc-sections,--icf=safe"
		LIBS="-lstdc++ -pthread -lm -ldl -lresolv -lstdc++fs"
		;;
	windows|Windows|WIN32|win32)
		CXX="x86_64-w64-mingw32-g++"
		STRIP_CMD="x86_64-w64-mingw32-strip"
		BINARY_NAME="warframe-api-helper.exe"
		PIE_FLAGS=""
		# GNU ld (mingw) doesn't support --icf=safe (LLVM-specific), only --gc-sections
		LINKER_FLAGS="-Wl,--gc-sections"
		# Windows libraries: ws2_32 (sockets/IP functions including WSAPoll, inet_pton, inet_ntop)
		# iphlpapi (IP helper), hid (HID devices)
		# Note: Put system libraries after static libs, and ws2_32 needs to be linked explicitly
		LIBS="-static-libgcc -static-libstdc++ -lws2_32 -liphlpapi -lhid"
		;;
	macos|MacOS|macOS|darwin)
		# Try to find macOS cross-compiler
		if command -v x86_64-apple-darwin-clang++ >/dev/null 2>&1; then
			CXX="x86_64-apple-darwin-clang++"
			STRIP_CMD="x86_64-apple-darwin-strip"
		elif command -v o64-clang++ >/dev/null 2>&1; then
			CXX="o64-clang++"
			STRIP_CMD="o64-strip"
		else
			echo "Error: macOS cross-compiler not found. Install osxcross or similar."
			exit 1
		fi
		BINARY_NAME="warframe-api-helper"
		PIE_FLAGS="-fPIE -pie"
		LINKER_FLAGS="-Wl,-dead_strip"
		LIBS="-lc++ -pthread"
		;;
	*)
		if [ "$CLEAN_ONLY" != true ]; then
			echo "Error: Unknown platform '$PLATFORM'"
			echo "Usage: $0 [linux|windows|macos] [--strip] [--clean|-c] [--clean-after|-ca]"
			exit 1
		fi
		;;
esac

# Check if compiler exists
if ! command -v "$CXX" >/dev/null 2>&1; then
	echo "Error: Compiler '$CXX' not found"
	exit 1
fi

# Clean previous builds for this platform
rm -rf Soup/bin/int/*.o bin/*.o "bin/$BINARY_NAME" 2>/dev/null || true
mkdir -p Soup/bin/int bin

# Common compilation flags
# For Windows: _WIN32_WINNT=0x0601 enables WSAPoll, inet_pton, inet_ntop (Windows 7+)
if [ "$PLATFORM" = "windows" ] || [ "$PLATFORM" = "Windows" ] || [ "$PLATFORM" = "WIN32" ] || [ "$PLATFORM" = "win32" ]; then
	COMMON_FLAGS="-std=c++17 -fno-rtti -O3 -ffunction-sections -fdata-sections -DSOUP_STANDALONE -D_WIN32_WINNT=0x0601"
else
	COMMON_FLAGS="-std=c++17 -fno-rtti -O3 -ffunction-sections -fdata-sections -DSOUP_STANDALONE"
fi

# Compile Soup library
echo "Compiling Soup library for $PLATFORM (using $NUM_CORES parallel jobs)..."
if [ "$PLATFORM" = "windows" ] || [ "$PLATFORM" = "Windows" ] || [ "$PLATFORM" = "WIN32" ] || [ "$PLATFORM" = "win32" ]; then
	# Windows: no PIE
	$CXX $COMMON_FLAGS -c Soup/soup/soup.cpp -o Soup/bin/int/soup.o
	
	find Soup/soup -name "*.cpp" ! -name "soup.cpp" -print0 | xargs -0 -P"$NUM_CORES" -I {} sh -c 'f="{}"; b=$(basename "$f" .cpp); '"$CXX $COMMON_FLAGS -c \"\$f\" -o \"Soup/bin/int/\$b.o\" 2>&1" | grep -v "^$" || true
else
	# Linux/macOS: with PIE
	$CXX $COMMON_FLAGS -fPIE -c Soup/soup/soup.cpp -o Soup/bin/int/soup.o
	
	find Soup/soup -name "*.cpp" ! -name "soup.cpp" -print0 | xargs -0 -P"$NUM_CORES" -I {} sh -c 'f="{}"; b=$(basename "$f" .cpp); '"$CXX $COMMON_FLAGS -fPIE -c \"\$f\" -o \"Soup/bin/int/\$b.o\" 2>&1" | grep -v "^$" || true
fi

echo "Compiled $(ls Soup/bin/int/*.o 2>/dev/null | wc -l) Soup object files"

# Compile main.cpp
echo "Compiling main.cpp..."
if [ "$PLATFORM" = "windows" ] || [ "$PLATFORM" = "Windows" ] || [ "$PLATFORM" = "WIN32" ] || [ "$PLATFORM" = "win32" ]; then
	$CXX $COMMON_FLAGS -c main.cpp -o bin/main.o -ISoup/soup
else
	$CXX $COMMON_FLAGS -fPIE -c main.cpp -o bin/main.o -ISoup/soup
fi

# Link
echo "Linking binary..."
if [ "$PLATFORM" = "windows" ] || [ "$PLATFORM" = "Windows" ] || [ "$PLATFORM" = "WIN32" ] || [ "$PLATFORM" = "win32" ]; then
	$CXX $COMMON_FLAGS $LINKER_FLAGS $LIBS bin/main.o Soup/bin/int/*.o -o "bin/$BINARY_NAME"
else
	$CXX $COMMON_FLAGS $PIE_FLAGS $LINKER_FLAGS $LIBS bin/main.o Soup/bin/int/*.o -o "bin/$BINARY_NAME"
fi

# Strip debugging symbols if requested
if [ -n "$STRIP_FLAG" ]; then
	if command -v "$STRIP_CMD" >/dev/null 2>&1; then
		echo ""
		echo "Stripping debugging symbols..."
		$STRIP_CMD "bin/$BINARY_NAME"
		echo "Debugging symbols removed."
	else
		echo ""
		echo "Warning: Strip command '$STRIP_CMD' not found. Skipping strip."
	fi
fi

echo ""
echo "warframe-api-helper binary created:"
ls -lh "bin/$BINARY_NAME"

echo ""
echo "Binary type:"
file "bin/$BINARY_NAME"

echo ""
echo "Build complete for $PLATFORM!"
if [ -z "$STRIP_FLAG" ]; then
	echo ""
	echo "Note: Use './compile.sh $PLATFORM --strip' to remove debugging symbols"
fi

# Clean after build if requested (keep final binary, only clean object files)
if [ "$CLEAN_AFTER" = true ]; then
	echo ""
	clean_build "$PLATFORM" "true"
fi
