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
    // Elden Ring's running: Skyrim's movement input from the core's step.
    static void Walk(RE::PlayerCharacter* player, const ErWorld& world, const ErOutput& out, float delta);
    // The part of a root-motion step Skyrim's collision lets through.
    static RE::NiPoint3 Collide(RE::PlayerCharacter* player, RE::NiPoint3 move);
    static void CoreHits(RE::PlayerCharacter* player, const ErOutput& out);
    // Elden Ring's lock-on: the enemy nearest the middle of the view.
    // The next target to the left (-1) or right (+1) of the current one.
    static RE::Actor* SwitchTarget(RE::PlayerCharacter* player, RE::Actor* current, int side);
    // A whoosh when a swing goes live (Skyrim's swing sounds come from its
    // own attack animations, which don't play in Elden Ring mode).
    static void PlaySwing(RE::PlayerCharacter* player, const ErOutput& out);
    static RE::Actor* FindLockTarget(RE::PlayerCharacter* player, float heading);
    static RE::Actor* UpdateLock(RE::PlayerCharacter* player, bool pressed, float heading);
    static void PlayAction(RE::PlayerCharacter* player, const ErOutput& out);

    static inline ErCombat* combat = nullptr;
    static inline bool combatMode = false;
    // Who this hit of the swing already struck.
    static inline std::unordered_set<std::uint32_t> hitThisSwing;
    static inline bool hitLive = false;
    static inline bool skyrimJump = true;
    static inline bool lastSupported = true;
    static inline std::uint32_t lastSerial = 0;
    static inline bool iframes = false;
    static inline bool blocking = false;
    static inline bool heldMovement = false;
    static inline bool walkValid = false;
    static inline RE::NiPoint3 walkEnd;
    static inline bool walkDirValid = false;
    static inline float walkDirX = 0.0f, walkDirY = 0.0f;
    static inline float shownYaw = 0.0f;
    static inline bool yawValid = false;
    static inline bool swingLogged = false;
    static inline RE::ActorHandle lockTarget;
    static inline ErState lastState = ER_STATE_GROUND;
    static inline ErAction lastKind = ER_ACTION_NONE;
};
