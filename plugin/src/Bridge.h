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
    static void ApplyRootMotion(RE::PlayerCharacter* player, const ErWorld& world, const ErOutput& out, bool acting);
    static void ReleaseMovement();
    static void CoreHits(RE::PlayerCharacter* player, const ErOutput& out);
    // Elden Ring's lock-on: the enemy nearest the middle of the view.
    static RE::Actor* FindLockTarget(RE::PlayerCharacter* player, float heading);
    static RE::Actor* UpdateLock(RE::PlayerCharacter* player, bool pressed, float heading);
    static void PlayAction(RE::PlayerCharacter* player, const ErOutput& out);
    static float CameraHeading(RE::PlayerCharacter* player);

    static inline ErCombat* combat = nullptr;
    static inline bool combatMode = false;
    // Who this hit of the swing already struck.
    static inline std::unordered_set<std::uint32_t> hitThisSwing;
    static inline bool hitLive = false;
    static inline bool lastSupported = true;
    static inline std::uint32_t lastSerial = 0;
    static inline bool iframes = false;
    static inline bool blocking = false;
    static inline bool heldMovement = false;
    static inline RE::ActorHandle lockTarget;
    static inline ErState lastState = ER_STATE_GROUND;
};
