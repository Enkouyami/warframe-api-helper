(*
    Build script for warframe-api-helper
    Compiles Soup library and warframe-api-helper executable
    Uses C++17 standard
*)

open System
open System.IO
open System.Diagnostics

#load "/run/media/obsidian_jackal/External_Organ/Tech_and_Software/Programs & Scripts/Migration_Tool/Linux/utils.fsx"
open Utils

// Parse command line arguments
let mutable verbose = false
let mutable platform = "linux"
let mutable cleanOnly = false
let mutable useSun = false

// Get arguments - when running: dotnet fsi build.fsx --verbose
// GetCommandLineArgs() returns the full command line
let allArgs = Environment.GetCommandLineArgs()

// Find the script name and process everything after it
let scriptIndex = allArgs |> Array.tryFindIndex (fun arg -> arg.EndsWith(".fsx"))
let args = 
    match scriptIndex with
    | Some idx when idx < allArgs.Length - 1 ->
        allArgs.[idx + 1..] // Everything after the script name
    | _ ->
        allArgs |> Array.skip 1 // Fallback: skip first arg

// Parse arguments
for arg in args do
    let argLower = arg.ToLower()
    match argLower with
    | "-v" | "--verbose" -> verbose <- true
    | "-c" | "--clean" -> cleanOnly <- true
    | "--sun" -> useSun <- true
    | "linux" | "windows" | "macos" -> platform <- argLower
    | _ -> () // Ignore other arguments

printfn "=== Build warframe-api-helper ==="
printfn ""

// Get script directory
let scriptDir = __SOURCE_DIRECTORY__
let soupDir = Path.Combine(scriptDir, "Soup", "soup")
let binDir = Path.Combine(scriptDir, "Soup", "bin", "int")
let outputBinDir = Path.Combine(scriptDir, "bin")

// Sun build system path - use ~/.local/bin/sun on Linux/macOS, or "sun" if in PATH
let sunPath = 
    if platform = "linux" || platform = "macos" then
        let homeDir = Environment.GetFolderPath(Environment.SpecialFolder.UserProfile)
        let localBinSun = Path.Combine(homeDir, ".local", "bin", "sun")
        if File.Exists(localBinSun) then
            localBinSun
        elif commandExists "sun" then
            "sun" // In PATH
        else
            failwith "Sun build system not found. Please create symlink: ln -s /path/to/suncli ~/.local/bin/sun"
    else
        // Windows or other platforms - check PATH only
        if commandExists "sun" then
            "sun"
        else
            failwith "Sun build system not found in PATH"

// Check for Sun build system first (before defaulting to clang)
let checkForSunBuildSystem () =
    if platform = "linux" || platform = "macos" then
        let homeDir = Environment.GetFolderPath(Environment.SpecialFolder.UserProfile)
        let localBinSun = Path.Combine(homeDir, ".local", "bin", "sun")
        if File.Exists(localBinSun) then
            Some localBinSun
        elif commandExists "sun" then
            Some "sun"
        else
            None
    else
        if commandExists "sun" then
            Some "sun"
        else
            None

// Ensure directories exist
ensureDirectory binDir |> ignore
ensureDirectory outputBinDir |> ignore

// Function to run command with optional verbose streaming
let runCompileCommand (command: string) (args: string) (workingDir: string option) =
    if verbose then
        // Stream output in real-time
        printfn "[VERBOSE] Running: %s %s" command args
        stdout.Flush()
        let argList = 
            args.Split(' ') 
            |> Array.filter (fun s -> s.Length > 0 && s <> "\"\"" && s <> "\"") 
            |> Array.map (fun s -> s.Trim('"'))
            |> Array.toList
        let exitCode = runCommandStream command argList workingDir
        if exitCode <> 0 then
            printfn "[VERBOSE] Command failed with exit code %d" exitCode
        exitCode
    else
        // Capture output and only show errors/warnings
        let exitCode, output, error = runCommand command args workingDir
        if exitCode <> 0 || not (String.IsNullOrWhiteSpace error) then
            if not (String.IsNullOrWhiteSpace output) then printfn "%s" output
            if not (String.IsNullOrWhiteSpace error) then eprintfn "%s" error
        exitCode

// Clean build artifacts if requested
if cleanOnly then
    printfn "Cleaning build artifacts..."
    try
        match checkForSunBuildSystem () with
        | Some sun ->
            // Use Sun to clean
            printfn "Using Sun build system to clean..."
            let exitCode, _, _ = runCommand sun "" (Some soupDir)
            if exitCode <> 0 then
                printfn "Warning: Sun clean may have failed, continuing with manual clean..."
        | None -> ()
        if Directory.Exists(binDir) then
            Directory.GetFiles(binDir, "*.o") |> Array.iter File.Delete
        if Directory.Exists(outputBinDir) then
            Directory.GetFiles(outputBinDir, "*.o") |> Array.iter File.Delete
        let libPath = Path.Combine(scriptDir, "libsoup.a")
        if File.Exists(libPath) then File.Delete(libPath)
        let sunLibPath = Path.Combine(soupDir, "soup.a")
        if File.Exists(sunLibPath) then File.Delete(sunLibPath)
        printfn "Clean complete!"
    with
    | ex -> printfn "Error: Failed to clean: %s" ex.Message
    exit 0

// Check for Sun build system first - if available, use it
match checkForSunBuildSystem () with
| Some sun when not useSun ->
    // Sun build system found, but --sun flag not explicitly set
    // Still use it as default (checking Sun first)
    useSun <- true
    if verbose then
        printfn "[VERBOSE] Sun build system found, using it by default"
| Some sun ->
    // Sun build system found and --sun flag is set
    useSun <- true
| None ->
    // No Sun build system found, will use clang/gcc
    useSun <- false

// If using Sun, build with Sun and then link the main executable
if useSun then
    let sunPath = 
        match checkForSunBuildSystem () with
        | Some path -> path
        | None -> failwith "Sun build system not found but useSun is true"
    
    printfn "Using Sun build system to compile Soup library..."
    if verbose then
        printfn ""
        printfn "[VERBOSE] Running: %s" sunPath
    
    let exitCode, output, error = runCommand sunPath "" (Some soupDir)
    
    if exitCode <> 0 then
        printfn "Error: Sun build failed"
        if not (String.IsNullOrWhiteSpace output) then printfn "%s" output
        if not (String.IsNullOrWhiteSpace error) then eprintfn "%s" error
        exit exitCode
    
    // Check if soup.a was created
    let sunLibPath = Path.Combine(soupDir, "soup.a")
    if not (File.Exists(sunLibPath)) then
        printfn "Error: Sun did not create soup.a"
        exit 1
    
    // Copy soup.a to libsoup.a in project root for consistency
    let libPath = Path.Combine(scriptDir, "libsoup.a")
    File.Copy(sunLibPath, libPath, true)
    let fileInfo = FileInfo(libPath)
    printfn "Soup library built with Sun: %s (%.2f KB)" libPath (float fileInfo.Length / 1024.0)
    
    // Now compile and link the main executable using the regular method
    printfn ""
    printfn "Compiling warframe-api-helper executable..."
    
    // Find main source
    let findMainSource () =
        let possiblePaths = [
            Path.Combine(scriptDir, "main.cpp")
            Path.Combine(scriptDir, "src", "main.cpp")
        ]
        let found = possiblePaths |> List.tryFind File.Exists
        match found with
        | Some path -> Some path
        | None ->
            try
                let allCppFiles = Directory.GetFiles(scriptDir, "*.cpp", SearchOption.TopDirectoryOnly)
                let filtered = 
                    allCppFiles 
                    |> Array.filter (fun f -> 
                        not (f.Contains("Soup")) && 
                        not (f.Contains("build")) &&
                        not (f.Contains("bin")))
                filtered |> Array.tryHead
            with
            | _ -> None
    
    match findMainSource () with
    | Some mainSource ->
        // Get compiler settings for linking
        let cxx, pieFlags, linkerFlags, libs, binaryName =
            match platform with
            | "linux" ->
                ("clang++", "-fPIE", "-fuse-ld=lld -Wl,--gc-sections,--icf=safe", 
                 "-lstdc++ -pthread -lm -ldl -lresolv -lstdc++fs", "warframe-api-helper")
            | "windows" ->
                // Try clang-based MinGW first, then native Windows clang, then GCC-based MinGW
                let windowsCompiler = 
                    if commandExists "x86_64-w64-mingw32-clang++" then
                        "x86_64-w64-mingw32-clang++"
                    elif commandExists "clang++" then
                        "clang++"  // Native Windows clang
                    elif commandExists "clang-cl" then
                        "clang-cl"  // MSVC-compatible clang frontend
                    elif commandExists "x86_64-w64-mingw32-g++" then
                        "x86_64-w64-mingw32-g++"  // GCC-based MinGW (fallback)
                    else
                        failwith "No Windows compiler found. Please install clang or MinGW-w64"
                (windowsCompiler, "", "-Wl,--gc-sections",
                 "-static-libgcc -static-libstdc++ -liphlpapi -ldnsapi -lbcrypt -lsetupapi -lgdi32 -lhid -lws2_32 -lwsock32",
                 "warframe-api-helper.exe")
            | "macos" ->
                let cxx = 
                    if commandExists "x86_64-apple-darwin-clang++" then "x86_64-apple-darwin-clang++"
                    elif commandExists "o64-clang++" then "o64-clang++"
                    else failwith "macOS cross-compiler not found"
                (cxx, "-fPIE", "-Wl,-dead_strip", "-lc++ -pthread", "warframe-api-helper")
            | _ -> failwith (sprintf "Unknown platform: %s" platform)
        
        if not (commandExists cxx) then
            printfn "Error: Compiler '%s' not found for linking" cxx
            exit 1
        
        let commonFlags = 
            if platform = "windows" then
                "-std=c++17 -fno-rtti -O3 -ffunction-sections -fdata-sections -DSOUP_STANDALONE -D_WIN32_WINNT=0x0601"
            else
                "-std=c++17 -fno-rtti -O3 -ffunction-sections -fdata-sections -DSOUP_STANDALONE"
        
        // Compile main source
        let mainObj = Path.Combine(outputBinDir, "main.o")
        let compileArgs = 
            if platform = "windows" then
                sprintf "%s -c \"%s\" -o \"%s\" -I%s" commonFlags mainSource mainObj soupDir
            else
                sprintf "%s %s -c \"%s\" -o \"%s\" -I%s" commonFlags pieFlags mainSource mainObj soupDir
        
        let compileExitCode = runCompileCommand cxx compileArgs (Some scriptDir)
        
        if compileExitCode <> 0 then
            printfn "Error: Failed to compile main.cpp"
            exit compileExitCode
        
        // Link executable
        printfn "Linking binary..."
        let binaryPath = Path.Combine(outputBinDir, binaryName)
        let linkArgs = 
            if platform = "windows" then
                sprintf "%s %s \"%s\" libsoup.a %s -o \"%s\"" commonFlags linkerFlags mainObj libs binaryPath
            else
                sprintf "%s %s %s \"%s\" libsoup.a %s -o \"%s\"" commonFlags pieFlags linkerFlags mainObj libs binaryPath
        
        let linkExitCode = runCompileCommand cxx linkArgs (Some scriptDir)
        
        if linkExitCode <> 0 then
            printfn "Error: Failed to link binary"
            exit linkExitCode
        
        if File.Exists(binaryPath) then
            let fileInfo = FileInfo(binaryPath)
            printfn "warframe-api-helper binary created: %s (%.2f KB)" binaryPath (float fileInfo.Length / 1024.0)
            printfn ""
            printfn "Binary type:"
            let exitCode, output, _ = runCommand "file" (sprintf "\"%s\"" binaryPath) None
            if exitCode = 0 then printfn "%s" output
        else
            printfn "Error: Binary was not created"
            exit 1
    | None ->
        printfn "Warning: No main source file found - only Soup library was built with Sun"
    
    printfn ""
    printfn "Build complete for %s using Sun!" platform
    exit 0

// Platform-specific settings - find best available compiler
let cxx, pieFlags, linkerFlags, libs, binaryName =
    match platform with
    | "linux" ->
        ("clang++", "-fPIE", "-fuse-ld=lld -Wl,--gc-sections,--icf=safe", 
         "-lstdc++ -pthread -lm -ldl -lresolv -lstdc++fs", "warframe-api-helper")
    | "windows" ->
        // Try clang-based MinGW first, then native Windows clang, then GCC-based MinGW
        let windowsCompiler = 
            if commandExists "x86_64-w64-mingw32-clang++" then
                "x86_64-w64-mingw32-clang++"
            elif commandExists "clang++" then
                "clang++"  // Native Windows clang
            elif commandExists "clang-cl" then
                "clang-cl"  // MSVC-compatible clang frontend
            elif commandExists "x86_64-w64-mingw32-g++" then
                "x86_64-w64-mingw32-g++"  // GCC-based MinGW (fallback)
            else
                failwith "No Windows compiler found. Please install clang or MinGW-w64"
        (windowsCompiler, "", "-Wl,--gc-sections",
         "-static-libgcc -static-libstdc++ -liphlpapi -ldnsapi -lbcrypt -lsetupapi -lgdi32 -lhid -lws2_32 -lwsock32",
         "warframe-api-helper.exe")
    | "macos" ->
        let cxx = 
            if commandExists "x86_64-apple-darwin-clang++" then "x86_64-apple-darwin-clang++"
            elif commandExists "o64-clang++" then "o64-clang++"
            else failwith "macOS cross-compiler not found"
        (cxx, "-fPIE", "-Wl,-dead_strip", "-lc++ -pthread", "warframe-api-helper")
    | _ -> failwith (sprintf "Unknown platform: %s" platform)

// Check if compiler exists
if not (commandExists cxx) then
    printfn "Error: Compiler '%s' not found" cxx
    exit 1

// Common compilation flags - C++17
let commonFlags = 
    if platform = "windows" then
        "-std=c++17 -fno-rtti -O3 -ffunction-sections -fdata-sections -DSOUP_STANDALONE -D_WIN32_WINNT=0x0601"
    else
        "-std=c++17 -fno-rtti -O3 -ffunction-sections -fdata-sections -DSOUP_STANDALONE"

let numCores = getCpuCoreCount()

// Compile Soup library
printfn "Compiling Soup library for %s (using %d parallel jobs)..." platform numCores
if verbose then
    printfn ""
    printfn "Verbose mode: showing all compilation commands and output"
printfn ""

let soupCppFiles = 
    if Directory.Exists(soupDir) then
        Directory.GetFiles(soupDir, "*.cpp", SearchOption.TopDirectoryOnly)
    else
        [||]

if soupCppFiles.Length = 0 then
    printfn "Error: No .cpp files found in %s" soupDir
    exit 1

// Class-based compilation tracking for organization
type CompileTask = {
    File: string
    Output: string
    BaseName: string
}

let compileTasks = 
    soupCppFiles
    |> Array.map (fun file ->
        let baseName = Path.GetFileNameWithoutExtension(file)
        let outputFile = Path.Combine(binDir, baseName + ".o")
        { File = file; Output = outputFile; BaseName = baseName }
    )

let compileSoupFile (task: CompileTask) =
    let includeFlag = sprintf "-I%s" soupDir
    let compileArgs = 
        if platform = "windows" then
            sprintf "%s -c \"%s\" -o \"%s\" %s" commonFlags task.File task.Output includeFlag
        else
            sprintf "%s %s -c \"%s\" -o \"%s\" %s" commonFlags pieFlags task.File task.Output includeFlag
    
    let exitCode = runCompileCommand cxx compileArgs (Some scriptDir)
    if exitCode <> 0 then
        printfn "Error: Failed to compile %s" (Path.GetFileName(task.File))
        exit exitCode

// Compile all Soup files
let mutable compiledCount = 0
for task in compileTasks do
    if verbose then
        printfn ""
        printfn "Compiling %s..." (Path.GetFileName(task.File))
    compileSoupFile task
    compiledCount <- compiledCount + 1

printfn "Compiled %d Soup object files" compiledCount

// Create static library
printfn "Creating libsoup.a..."
let objectFiles = Directory.GetFiles(binDir, "*.o")
if objectFiles.Length = 0 then
    printfn "Error: No object files to link"
    exit 1

let arArgs = sprintf "rcs libsoup.a %s" (String.Join(" ", objectFiles |> Array.map (fun f -> sprintf "\"%s\"" f)))
let arExitCode = runCompileCommand "ar" arArgs (Some scriptDir)

if arExitCode <> 0 then
    printfn "Error: Failed to create libsoup.a"
    exit arExitCode

let libPath = Path.Combine(scriptDir, "libsoup.a")
if File.Exists(libPath) then
    let fileInfo = FileInfo(libPath)
    printfn "libsoup.a created: %s (%.2f KB)" libPath (float fileInfo.Length / 1024.0)
else
    printfn "Error: libsoup.a was not created"
    exit 1

// Find and compile main source file
printfn ""
printfn "Looking for warframe-api-helper source files..."

let findMainSource () =
    let possiblePaths = [
        Path.Combine(scriptDir, "main.cpp")
        Path.Combine(scriptDir, "src", "main.cpp")
    ]
    
    let found = possiblePaths |> List.tryFind File.Exists
    match found with
    | Some path -> Some path
    | None ->
        try
            let allCppFiles = Directory.GetFiles(scriptDir, "*.cpp", SearchOption.TopDirectoryOnly)
            let filtered = 
                allCppFiles 
                |> Array.filter (fun f -> 
                    let fileName = Path.GetFileName(f).ToLower()
                    not (f.Contains("Soup")) && 
                    not (f.Contains("build")) &&
                    not (f.Contains("bin")) &&
                    fileName <> "build.fsx")
            filtered |> Array.tryHead
        with
        | _ -> None

match findMainSource () with
| Some mainSource ->
    printfn "Found source file: %s" mainSource
    printfn "Compiling warframe-api-helper..."
    
    let mainObj = Path.Combine(outputBinDir, "main.o")
    let compileArgs = 
        if platform = "windows" then
            sprintf "%s -c \"%s\" -o \"%s\" -I%s" commonFlags mainSource mainObj soupDir
        else
            sprintf "%s %s -c \"%s\" -o \"%s\" -I%s" commonFlags pieFlags mainSource mainObj soupDir
    
    let compileExitCode = runCompileCommand cxx compileArgs (Some scriptDir)
    
    if compileExitCode <> 0 then
        printfn "Error: Failed to compile main.cpp"
        exit compileExitCode
    
    // Link executable
    printfn "Linking binary..."
    let binaryPath = Path.Combine(outputBinDir, binaryName)
    let linkArgs = 
        if platform = "windows" then
            sprintf "%s %s \"%s\" libsoup.a %s -o \"%s\"" commonFlags linkerFlags mainObj libs binaryPath
        else
            sprintf "%s %s %s \"%s\" libsoup.a %s -o \"%s\"" commonFlags pieFlags linkerFlags mainObj libs binaryPath
    
    let linkExitCode = runCompileCommand cxx linkArgs (Some scriptDir)
    
    if linkExitCode <> 0 then
        printfn "Error: Failed to link binary"
        exit linkExitCode
    
    if File.Exists(binaryPath) then
        let fileInfo = FileInfo(binaryPath)
        printfn "warframe-api-helper binary created: %s (%.2f KB)" binaryPath (float fileInfo.Length / 1024.0)
        printfn ""
        printfn "Binary type:"
        let exitCode, output, _ = runCommand "file" (sprintf "\"%s\"" binaryPath) None
        if exitCode = 0 then printfn "%s" output
    else
        printfn "Error: Binary was not created"
        exit 1
| None ->
    printfn "Warning: No main source file found (main.cpp or src/main.cpp)"
    printfn "Only Soup library was built."
    printfn ""
    printfn "Searched in: %s" scriptDir
    printfn "To build the executable, place your main source file (main.cpp) in the project root or src/ directory"

printfn ""
printfn "Build complete for %s!" platform
printfn ""
