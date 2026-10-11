#pragma once

#include "er_combat.h"

// Maps what Skyrim has in the player's hands to Elden Ring weapon classes.
//
// Modded weapon types are found by keyword first (Animated Armoury and the
// mods that share its keywords), then by Skyrim's own weapon type. Bows,
// crossbows, magic staffs and spells have no Elden Ring moveset in our data:
// with one of those in either hand the fight stays Skyrim's.
namespace Weapons
{
    struct Loadout
    {
        bool supported = true;      // false: a bow, crossbow, magic staff or spell is in hand
        std::string_view right;     // Elden Ring class names ("" = keep)
        std::string_view left;
        bool twoHanded = false;
    };

    Loadout Read(RE::PlayerCharacter* player);

    // Puts the loadout into the core when it changed. Call with the core lock held.
    void Apply(ErCombat* combat, const Loadout& loadout);

    // Forget the last loadout (a new core was created).
    void Reset();
}
