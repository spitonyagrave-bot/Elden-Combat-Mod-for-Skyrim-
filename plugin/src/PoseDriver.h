#pragma once

#include "er_combat.h"

// Writes the Elden Ring pose the core samples onto the player's third-person
// skeleton every frame, after Skyrim's own animation update, while Elden Ring
// combat mode is on. Skyrim's behavior graph keeps running underneath (it
// idles), and the pose fades in and back out over ER_POSE_FADE_SECONDS.
// When Elden Ring mode is off and the fade is over, it touches nothing.
class PoseDriver
{
public:
    // Loads ERCombat.eranims and hooks the player's animation update.
    static void Install();

    // The bridge says each frame whether the pose should be shown and which
    // simulation drives it (null: fade out).
    static void SetSource(const ErCombat* combat);

    // The pose driver is loaded, bound to the player's skeleton and showing
    // (or about to show) the Elden Ring pose: Skyrim's own attack and dodge
    // animations must not be started then.
    static bool Active();

    // Called after the player's update when the animation-update hook is not
    // used (ERCombat.ini [Pose] Hook=0).
    static void AfterPlayerUpdate(RE::PlayerCharacter* player, float delta);

private:
    static void UpdateAnimation(RE::PlayerCharacter* a_this, float a_delta);
    static inline REL::Relocation<decltype(UpdateAnimation)> _UpdateAnimation;

    static void Apply(RE::PlayerCharacter* player, float delta);
    static bool Bind(RE::PlayerCharacter* player, RE::NiAVObject* root);

    static inline bool loaded = false;
    static inline ErRig* rig = nullptr;
    static inline const ErCombat* source = nullptr;
    static inline bool wanted = false;

    // The skeleton the rig is bound to.
    static inline RE::NiAVObject* boundRoot = nullptr;
    static inline std::array<RE::NiAVObject*, ER_BONE_COUNT> bones{};
    static inline std::uint32_t driven = 0;
    static inline bool bindFailed = false;

    static inline bool writing = false;
    static inline bool waitLogged = false;
    static inline bool sawHook = false;
    static inline std::string lastClip;
};
