// Built without the CommonLib PCH so it can include <Windows.h> on its own.
#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <Windows.h>

#include <string>

#include "CoreLoader.h"

namespace CoreLoader
{
    std::uint32_t Load()
    {
        HMODULE self = nullptr;
        if (!GetModuleHandleExW(
                GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
                reinterpret_cast<LPCWSTR>(&Load), &self)) {
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
}
