#include <iostream>
#include <unordered_map>

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

int main()
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
	auto authz = gruzzleAuthz(*mod);
	SOUP_IF_UNLIKELY (authz.empty())
	{
#if SOUP_WINDOWS
		system("pause > nul");
#endif
		return 3;
	}
	std::cout << authz << std::endl;
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
	string::toFile("inventory.json", jr->encodePretty());
	aes::pkcs7Pad(inventory);
	aes::cbcEncrypt(
		reinterpret_cast<uint8_t*>(inventory.data()), inventory.size(),
		key, 16,
		iv
	);
	string::toFile("lastData.dat", inventory);
	std::cout << "Saved to inventory.json & lastData.dat in working directory." << std::endl;
#if SOUP_WINDOWS
	system("pause > nul");
#endif
	return 0;
}
