// Built without the CommonLib PCH so it can include <Windows.h> on its own.
#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <Windows.h>
#include <ShlObj.h>

#include <string>

#include "Win32.h"

namespace Win32
{
    std::uint32_t LoadCore()
    {
        HMODULE self = nullptr;
        if (!GetModuleHandleExW(
                GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
                reinterpret_cast<LPCWSTR>(&LoadCore), &self)) {
            return GetLastError();
        }

        std::wstring path(MAX_PATH, L'\0');
        for (;;) {
            auto len = GetModuleFileNameW(self, path.data(), static_cast<DWORD>(path.size()));
            if (len == 0) {
                return GetLastError();
            }
            if (len < path.size()) {
                path.resize(len);
                break;
            }
            path.resize(path.size() * 2);
        }

        path.erase(path.find_last_of(L"\\/") + 1);
        path += L"er_combat_core.dll";
        if (!LoadLibraryExW(path.c_str(), nullptr, LOAD_WITH_ALTERED_SEARCH_PATH)) {
            return GetLastError();
        }
        return 0;
    }

    std::optional<std::filesystem::path> LogDirectory()
    {
        PWSTR documents = nullptr;
        if (SHGetKnownFolderPath(FOLDERID_Documents, KF_FLAG_DEFAULT, nullptr, &documents) != S_OK) {
            CoTaskMemFree(documents);
            return std::nullopt;
        }
        std::filesystem::path path = documents;
        CoTaskMemFree(documents);
        return path / "My Games" / "Skyrim Special Edition" / "SKSE";
    }
}
