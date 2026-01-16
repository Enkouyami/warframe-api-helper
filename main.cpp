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

[[nodiscard]] static std::string gruzzleAuthz(const ProcessHandle& mod)
{
	std::cout << "Gruzzling";
	std::unordered_map<std::string, int> candidates{};
	const auto pattern = Pattern("3F 61 63 63 6F 75 6E 74 49 64 3D"); // ?accountId=
	for (const auto& ai : mod.getAllocations())
	{
		if (auto res = mod.externalScan(ai.range, pattern))
		{
			res = res.add(11);

			char accountId[24];
			mod.externalRead(res, accountId, 24);
			res = res.add(24);

			res = res.add(7); // &nonce=

			std::string authz = "?accountId=" + std::string(accountId, 24) + "&nonce=";
			char c;
			do
			{
				c = mod.externalRead<char>(res);
				res = res.add(1);
			} while (string::isNumberChar(c) && (authz.push_back(c), true));
			std::cout << ".";
			if (auto e = candidates.find(authz); e != candidates.end())
			{
				if (++e->second == 3)
				{
					std::cout << " The crumbs have been gruzzled." << std::endl;
					return authz;
				}
			}
			else
			{
				candidates.emplace(authz, 1);
			}
		}
	}
	
	// If no candidate found with 3 occurrences, check for ones with 2 occurrences
	std::string bestCandidate{};
	int bestCount = 0;
	for (const auto& [authz, count] : candidates)
	{
		if (count >= 2 && count > bestCount)
		{
			bestCandidate = authz;
			bestCount = count;
		}
	}
	
	if (bestCount >= 2)
	{
		std::cout << " Warning: Found " << bestCount << " occurrences (expected 3). Using best candidate with disclaimer." << std::endl;
		std::cout << "DISCLAIMER: This result is based on " << bestCount << " occurrences instead of the expected 3. It may be less reliable." << std::endl;
		return bestCandidate;
	}
	
	std::cout << " Failed to gruzzle the crumbs." << std::endl;
	return {};
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
