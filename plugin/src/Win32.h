#pragma once

#include <cstdint>
#include <filesystem>
#include <optional>

// The few Windows calls the plugin makes itself. They live in their own
// translation unit, built without the CommonLib PCH, so it can include
// <Windows.h>.
namespace Win32
{
    // Loads er_combat_core.dll from the folder ERCombat.dll sits in.
    // ERCombat.dll delay-loads the core because Windows only looks for
    // dependencies next to SkyrimSE.exe, not in Data/SKSE/Plugins.
    // Returns 0 on success, otherwise the Windows error code.
    std::uint32_t LoadCore();

    // The folder ERCombat.dll sits in (Data/SKSE/Plugins), where the core
    // and ERCombat.erdata live too.
    std::optional<std::filesystem::path> PluginDirectory();

    // Documents/My Games/Skyrim Special Edition/SKSE, where SKSE and most
    // plugins log. CommonLibSSE-NG 3.7.0's own log_directory() reads the
    // folder name from an address that is wrong on 1.6.1170 and ends up in
    // "My Games/Skyrim.INI/SKSE" instead.
    std::optional<std::filesystem::path> LogDirectory();
}
