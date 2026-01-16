#include <iostream>
#include <unordered_map>
#include <cstdlib>
#include <cstring>
#include <algorithm>
#include <filesystem>
#include <vector>

#include <aes.hpp>
#include <HttpRequest.hpp>
#include <json.hpp>
#include <Pattern.hpp>
#include <Process.hpp>
#include <ProcessHandle.hpp>
#include <string.hpp>

using namespace soup;

static const uint8_t key[16] = { 76, 69, 79, 45, 65, 76, 69, 67, 9, 69, 79, 45, 65, 76, 69, 67 };
static const uint8_t iv[16] = { 49, 50, 70, 71, 66, 51, 54, 45, 76, 69, 51, 45, 113, 61, 57, 0 };

// Helper function to sanitize filename (remove invalid characters)
[[nodiscard]] static std::string sanitizeFilename(const std::string& name)
{
	std::string sanitized = name;
	// Replace invalid filename characters
	for (char& character : sanitized)
	{
		if (character == '/' || character == '\\' || character == ':' || character == '*' || character == '?' || character == '"' || character == '<' || character == '>' || character == '|')
		{
			character = '_';
		}
	}
	// Limit length
	if (sanitized.length() > 50)
	{
		sanitized = sanitized.substr(0, 50);
	}
	return sanitized;
}

// Helper function to get accountId from command line or environment
[[nodiscard]] static std::string getAccountIdFromArgs(int argc, char* argv[])
{
	std::string accountId;
	
	// Check CLI arguments first (takes precedence)
	for (int arg_index = 1; arg_index < argc; ++arg_index)
	{
		std::string arg = argv[arg_index];
		if (arg.find("--account-id=") == 0)
		{
			accountId = arg.substr(13); // Length of "--account-id="
			// Remove any whitespace
			accountId.erase(std::remove_if(accountId.begin(), accountId.end(), ::isspace), accountId.end());
			return accountId;
		}
		if (arg.find("-a=") == 0)
		{
			accountId = arg.substr(3); // Length of "-a="
			accountId.erase(std::remove_if(accountId.begin(), accountId.end(), ::isspace), accountId.end());
			return accountId;
		}
	}
	
	// Check environment variable (if CLI arg not found)
	const char* env_accountId = std::getenv("ACCOUNT_ID");
	if (env_accountId != nullptr)
	{
		accountId = env_accountId;
		// Remove any whitespace
		accountId.erase(std::remove_if(accountId.begin(), accountId.end(), ::isspace), accountId.end());
	}
	
	return accountId;
}

// Helper function to get nonce from command line or environment
[[nodiscard]] static std::string getNonceFromArgs(int argc, char* argv[])
{
	std::string nonce;
	
	// Check CLI arguments first (takes precedence)
	for (int arg_index = 1; arg_index < argc; ++arg_index)
	{
		std::string arg = argv[arg_index];
		if (arg.find("--nonce=") == 0)
		{
			nonce = arg.substr(8); // Length of "--nonce="
			// Remove any whitespace
			nonce.erase(std::remove_if(nonce.begin(), nonce.end(), ::isspace), nonce.end());
			return nonce;
		}
		if (arg.find("-n=") == 0)
		{
			nonce = arg.substr(3); // Length of "-n="
			nonce.erase(std::remove_if(nonce.begin(), nonce.end(), ::isspace), nonce.end());
			return nonce;
		}
	}
	
	// Check environment variable (if CLI arg not found)
	const char* env_nonce = std::getenv("NONCE");
	if (env_nonce != nullptr)
	{
		nonce = env_nonce;
		// Remove any whitespace
		nonce.erase(std::remove_if(nonce.begin(), nonce.end(), ::isspace), nonce.end());
	}
	
	return nonce;
}

// Helper function to get accountId from lastData.dat files (NO EE.log)
[[nodiscard]] static std::string getAccountIdFromLastData()
{
	std::vector<std::string> datFiles;
	
	// Try lastData.dat first
	if (std::filesystem::exists("lastData.dat"))
	{
		datFiles.push_back("lastData.dat");
	}
	
	// Search for lastData_*.dat files
	try
	{
		for (const auto& entry : std::filesystem::directory_iterator("."))
		{
			if (entry.is_regular_file())
			{
				std::string filename = entry.path().filename().string();
				if (filename.find("lastData_") == 0 && filename.find(".dat") == filename.length() - 4)
				{
					datFiles.push_back(filename);
				}
			}
		}
	}
	catch (...)
	{
		// If directory iteration fails, just try the default file
	}
	
	for (const auto& datFile : datFiles)
	{
		std::string encrypted = string::fromFile(datFile);
		if (encrypted.empty())
		{
			continue;
		}
		
		// Decrypt
		std::string decrypted = encrypted;
		aes::cbcDecrypt(
			reinterpret_cast<uint8_t*>(decrypted.data()), decrypted.size(),
			key, 16,
			iv
		);
		
		// Remove PKCS7 padding
		if (!aes::pkcs7Unpad(decrypted))
		{
			continue;
		}
		
		// Parse JSON to extract accountId
		auto json_result = json::decode(decrypted);
		if (json_result && json_result->isObj())
		{
			auto& json_object = json_result->asObj();
			if (auto accountIdNode = json_object.find("accountId"))
			{
				if (accountIdNode->isStr())
				{
					std::string accountId = accountIdNode->asStr().value;
					if (accountId.length() == 24)
					{
						return accountId;
					}
				}
			}
		}
	}
	
	return {};
}

[[nodiscard]] static std::string gruzzleAuthz(const ProcessHandle& mod, bool allMatches = false)
{
	std::cout << "Gruzzling";
	const auto pattern = Pattern("3F 61 63 63 6F 75 6E 74 49 64 3D"); // ?accountId=
	std::vector<std::string> matches;
	for (const auto& ai : mod.getAllocations())
	{
		if (auto res = mod.externalScan(ai.range, pattern))
		{
			res = res.add(11); // Skip "?accountId="

			char accountId[24];
			mod.externalRead(res, accountId, 24);
			res = res.add(24);

			// Verify we have "&nonce=" next
			char noncePrefix[7];
			mod.externalRead(res, noncePrefix, 7);
			if (std::memcmp(noncePrefix, "&nonce=", 7) != 0)
			{
				// This match doesn't have &nonce=, skip it (enhanced algorithm requirement)
				continue;
			}
			res = res.add(7); // Skip "&nonce="

			// Verify accountId is valid (24 hex characters)
			bool validAccountId = true;
			for (int index = 0; index < 24; ++index)
			{
				if (!string::isHexDigitChar(accountId[index]))
				{
					validAccountId = false;
					break;
				}
			}
			
			if (!validAccountId)
			{
				continue;
			}

			std::string authz = "?accountId=" + std::string(accountId, 24) + "&nonce=";
			char c;
			do
			{
				c = mod.externalRead<char>(res);
				res = res.add(1);
			} while (string::isNumberChar(c) && (authz.push_back(c), true));
			
			// Check for sessionId after nonce (continue reading from current position)
			// Look for "&sessionId=" pattern by reading characters directly
			// Limit search to 200 bytes after nonce to avoid scanning too far
			auto sessionCheckPos = res;
			std::string sessionIdCheck;
			for (int i = 0; i < 200 && sessionCheckPos < ai.range.end(); ++i)
			{
				c = mod.externalRead<char>(sessionCheckPos);
				sessionCheckPos = sessionCheckPos.add(1);
				sessionIdCheck.push_back(c);
				
				// Check if we found "&sessionId="
				if (sessionIdCheck.length() >= 11 && sessionIdCheck.substr(sessionIdCheck.length() - 11) == "&sessionId=")
				{
					// Found the pattern, now read the sessionId value
					std::string sessionId;
					do
					{
						c = mod.externalRead<char>(sessionCheckPos);
						sessionCheckPos = sessionCheckPos.add(1);
					} while ((string::isNumberChar(c) || (c >= 'a' && c <= 'f') || (c >= 'A' && c <= 'F')) && (sessionId.push_back(c), true));
					if (!sessionId.empty())
					{
						authz += "&sessionId=" + sessionId;
					}
					break;
				}
			}
			
			// Verify minimum length: "?accountId=...&nonce=" + at least one digit
			if (authz.length() > 19)
			{
				matches.push_back(authz);
				if (!allMatches)
				{
					std::cout << " The crumbs have been gruzzled." << std::endl;
					return authz;
				}
			}
			std::cout << ".";
		}
	}
	
	if (!matches.empty())
	{
		std::cout << " Found " << matches.size() << " gruzzled crumbs." << std::endl;
		return matches[0]; // Return first match
	}
	
	std::cout << " Failed to gruzzle the crumbs." << std::endl;
	return {};
}

// Structure to hold command-line arguments
struct Args {
	bool skip_scan = false;
	bool download = true;  // Default to true
	bool all_matches = false;
	std::string output_file;
};

[[nodiscard]] static Args parseArgs(int argc, char* argv[])
{
	Args args;
	
	for (int arg_index = 1; arg_index < argc; ++arg_index)
	{
		std::string arg = argv[arg_index];
		if (arg == "--skip-scan" || arg == "-s" || arg == "--skip-process")
		{
			args.skip_scan = true;
		}
		else if (arg == "--no-download")
		{
			args.download = false;
		}
		else if (arg == "--all-matches")
		{
			args.all_matches = true;
		}
		else if (arg.find("--output=") == 0)
		{
			args.output_file = arg.substr(9);
		}
	}
	
	return args;
}

int main(int argc, char* argv[])
{
	Args args = parseArgs(argc, argv);
	
	// Check for nonce from command line or environment
	std::string providedNonce = getNonceFromArgs(argc, argv);
	
	std::string authz;
	
	// If nonce is provided, build auth string from accountId + nonce
	if (!providedNonce.empty())
	{
		// Try to get accountId from CLI/env or lastData.dat
		std::string accountId = getAccountIdFromArgs(argc, argv);
		if (accountId.empty() || accountId.length() != 24)
		{
			accountId = getAccountIdFromLastData();
		}
		
		if (accountId.empty() || accountId.length() != 24)
		{
			std::cout << "Error: Could not find account ID. Please:" << std::endl;
			std::cout << "  - Provide it via --account-id flag or ACCOUNT_ID environment variable" << std::endl;
			std::cout << "  - Or run the tool once without --nonce to create a lastData.dat file" << std::endl;
#if SOUP_WINDOWS
			system("pause > nul");
#endif
			return 7;
		}
		authz = "?accountId=" + accountId + "&nonce=" + providedNonce;
		std::cout << "Using provided nonce (--nonce or NONCE environment variable)." << std::endl;
		std::cout << authz << std::endl;
	}
	
	// If not using provided nonce, scan memory (unless --skip-scan)
	if (authz.empty() && !args.skip_scan)
	{
		auto proc = Process::get("Warframe.x64.exe");
#if !SOUP_WINDOWS
		// On non-Windows systems (Linux, macOS, etc.), the process name is truncated
		// due to Linux's 16-character limit in /proc/[pid]/comm (TASK_COMM_LEN)
		// "Warframe.x64.exe" (17 chars) gets truncated to "Warframe.x64.ex" (16 chars)
		if (!proc)
		{
			proc = Process::get("Warframe.x64.ex");
		}
#endif
		if (!proc)
		{
			std::cout << "Process not found." << std::endl;
#if SOUP_WINDOWS
			system("pause > nul");
#endif
			return 1;
		}
		auto mod = proc->open();
		SOUP_IF_UNLIKELY (!mod)
		{
			std::cout << "Failed to open process." << std::endl;
#if SOUP_WINDOWS
			system("pause > nul");
#endif
			return 2;
		}
		authz = gruzzleAuthz(*mod, args.all_matches);
		SOUP_IF_UNLIKELY (authz.empty())
		{
#if SOUP_WINDOWS
			system("pause > nul");
#endif
			return 3;
		}
	}
	
	// If skip-scan and no nonce provided, try to get accountId from lastData.dat
	if (authz.empty() && args.skip_scan)
	{
		std::string accountId = getAccountIdFromArgs(argc, argv);
		if (accountId.empty() || accountId.length() != 24)
		{
			accountId = getAccountIdFromLastData();
		}
		
		if (accountId.empty() || accountId.length() != 24)
		{
			std::cout << "Error: Could not find account ID. Please:" << std::endl;
			std::cout << "  - Provide it via --account-id flag or ACCOUNT_ID environment variable" << std::endl;
			std::cout << "  - Or ensure lastData.dat exists in the working directory" << std::endl;
#if SOUP_WINDOWS
			system("pause > nul");
#endif
			return 7;
		}
		
		// Without nonce, we can't build a valid auth string
		std::cout << "Error: --skip-scan requires either --nonce flag or memory scanning." << std::endl;
#if SOUP_WINDOWS
		system("pause > nul");
#endif
		return 8;
	}
	
	// Print auth string if we scanned memory (already printed for providedNonce case)
	if (!authz.empty() && providedNonce.empty())
	{
		std::cout << authz << std::endl;
	}
	
	// Download inventory if requested (default is true)
	if (!args.download)
	{
		return 0;
	}
	
	std::cout << "Downloading inventory... ";
	// Note: Could also use api.warframe.com
	HttpRequest hr("mobile.warframe.com", "/api/inventory.php" + authz);
	auto res = hr.execute();
	SOUP_IF_UNLIKELY (!res)
	{
		std::cout << "Request failed." << std::endl;
#if SOUP_WINDOWS
		system("pause > nul");
#endif
		return 5;
	}
	auto inventory = std::move(res->body);
	auto jr = json::decode(inventory);
	SOUP_IF_UNLIKELY (!jr)
	{
		std::cout << "Received an invalid response." << std::endl;
#if SOUP_WINDOWS
		system("pause > nul");
#endif
		return 6;
	}
	
	// Extract account ID and account name from JSON
	std::string accountId;
	std::string accountName = "unknown";
	
	if (jr->isObj())
	{
		auto& json_object = jr->asObj();
		
		// Extract account ID (always keep in JSON - original behavior)
		if (auto accountIdNode = json_object.find("accountId"))
		{
			if (accountIdNode->isStr())
			{
				accountId = accountIdNode->asStr().value;
			}
		}
		
		// Extract account name (for filename suffix)
		// Try common field names for account name
		const char* nameFields[] = { "playerName", "PlayerName", "alias", "Alias", "name", "Name", "username", "Username" };
		for (const char* field_name : nameFields)
		{
			if (auto nameNode = json_object.find(field_name))
			{
				if (nameNode->isStr())
				{
					accountName = nameNode->asStr().value;
					break;
				}
			}
		}
	}
	
	// Create filename with account name suffix (when available)
	std::string jsonFilename = args.output_file.empty() ? "inventory.json" : args.output_file;
	if (args.output_file.empty() && accountName != "unknown" && !accountName.empty())
	{
		std::string sanitizedName = sanitizeFilename(accountName);
		jsonFilename = "inventory_" + sanitizedName + ".json";
	}
	
	// Save JSON (always with accountId - original behavior)
	string::toFile(jsonFilename, jr->encodePretty());
	std::cout << "Saved to " << jsonFilename << std::endl;
	
	// Create encrypted file with account ID included
	// Use the original inventory data (which still has accountId)
	std::string inventoryWithAccountId = inventory;
	
	// Encrypt and save
	aes::pkcs7Pad(inventoryWithAccountId);
	aes::cbcEncrypt(
		reinterpret_cast<uint8_t*>(inventoryWithAccountId.data()), inventoryWithAccountId.size(),
		key, 16,
		iv
	);
	
	std::string datFilename = "lastData.dat";
	if (accountName != "unknown" && !accountName.empty())
	{
		std::string sanitizedName = sanitizeFilename(accountName);
		datFilename = "lastData_" + sanitizedName + ".dat";
	}
	
	string::toFile(datFilename, inventoryWithAccountId);
	std::cout << "Saved to " << datFilename << " (encrypted, contains account ID)" << std::endl;
#if SOUP_WINDOWS
	system("pause > nul");
#endif
	return 0;
}
