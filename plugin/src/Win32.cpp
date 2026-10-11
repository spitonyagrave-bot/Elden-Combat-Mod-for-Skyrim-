// Built without the CommonLib PCH so it can include <Windows.h> on its own.
#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <Windows.h>
#include <ShlObj.h>

#include <string>

#include "Win32.h"

namespace
{
    // The folder this DLL (ERCombat.dll) is in, with a trailing separator.
    // Returns 0 on success, otherwise the Windows error code.
    DWORD OwnDirectory(std::wstring& dir)
    {
        HMODULE self = nullptr;
        if (!GetModuleHandleExW(
                GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
                reinterpret_cast<LPCWSTR>(&OwnDirectory), &self)) {
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
        dir = std::move(path);
        return 0;
    }
}

namespace Win32
{
    std::uint32_t LoadCore()
    {
        std::wstring path;
        if (auto error = OwnDirectory(path); error != 0) {
            return error;
        }
        path += L"er_combat_core.dll";
        if (!LoadLibraryExW(path.c_str(), nullptr, LOAD_WITH_ALTERED_SEARCH_PATH)) {
            return GetLastError();
        }
        return 0;
    }

    std::optional<std::filesystem::path> PluginDirectory()
    {
        std::wstring dir;
        if (OwnDirectory(dir) != 0) {
            return std::nullopt;
        }
        return std::filesystem::path(dir);
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

    void* RequestPluginApi(const wchar_t* dll, std::uint8_t version)
    {
        HMODULE module = GetModuleHandleW(dll);
        if (!module) {
            return nullptr;
        }
        using Request = void* (*)(std::uint8_t);
        auto request = reinterpret_cast<Request>(GetProcAddress(module, "RequestPluginAPI"));
        return request ? request(version) : nullptr;
    }
}
