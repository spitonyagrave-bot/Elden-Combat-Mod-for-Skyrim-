#pragma once

#include <cstdint>

namespace CoreLoader
{
    // Loads er_combat_core.dll from the folder ERCombat.dll sits in.
    // ERCombat.dll delay-loads the core because Windows only looks for
    // dependencies next to SkyrimSE.exe, not in Data/SKSE/Plugins.
    // Returns 0 on success, otherwise the Windows error code.
    std::uint32_t Load();
}
