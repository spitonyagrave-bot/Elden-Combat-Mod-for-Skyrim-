#pragma once

#include "er_combat.h"

// Runs the combat core alongside Skyrim's own player update.
//
// Combat mode is on only while Elden Ring mode is switched on (toggle key),
// a weapon is drawn and no menu is up. While it
// is on, the core owns dodging, sprinting, jumping and attacks: Skyrim's own
// handlers for those are switched off, and each action the core starts is
// played through the animation event named in ERCombat.ini. Activate,
// movement, menus, shouts and everything else stay with Skyrim, so doors,
// loot and dialogue keep working mid-fight.
class Bridge
{
public:
    static void Install();

    // Held while the simulation is stepped, read or destroyed: the pose
    // driver reads it from the animation update.
    static std::mutex& CoreLock()
    {
        static std::mutex lock;
        return lock;
    }

private:
    static void Update(RE::PlayerCharacter* a_this, float a_delta);
    static inline REL::Relocation<decltype(Update)> _Update;

    static void Tick(RE::PlayerCharacter* player, float delta);
    static void SetCombatMode(bool on);
    static void SetBlock(RE::PlayerCharacter* player, bool on);
    static void PlayAction(RE::PlayerCharacter* player, const ErOutput& out);
    static float CameraHeading(RE::PlayerCharacter* player);

    static inline ErCombat* combat = nullptr;
    static inline bool combatMode = false;
    static inline std::uint32_t lastSerial = 0;
    static inline bool iframes = false;
    static inline bool blocking = false;
    static inline ErState lastState = ER_STATE_GROUND;
};
